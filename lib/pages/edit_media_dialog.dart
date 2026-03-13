import 'dart:async';
import 'package:flutter/material.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/pipeline.dart';
import 'package:reel/src/rust/formats.dart';
import 'package:reel/src/rust/identify/tmdb.dart';
import 'package:reel/src/rust/api/pipeline_api.dart' as pipeline_api;
import 'package:reel/theme/app_theme.dart';

/// Dialog for correcting a misidentified media item.
/// Searches TMDb, lets user pick the correct match, and applies the edit.
class EditMediaDialog extends StatefulWidget {
  final MediaDetail detail;

  const EditMediaDialog({super.key, required this.detail});

  @override
  State<EditMediaDialog> createState() => _EditMediaDialogState();
}

class _EditMediaDialogState extends State<EditMediaDialog> {
  final _searchController = TextEditingController();
  Timer? _debounce;
  List<TmdbMatch> _results = [];
  List<FormatOption> _formatOptions = [];
  bool _searching = false;
  bool _saving = false;
  String? _error;

  // Editable field controllers
  final _titleController = TextEditingController();
  final _yearController = TextEditingController();

  // Editable fields — initialized from current metadata
  late String _format;
  late String _genre;
  late String _mediaType;
  late BigInt? _tmdbId;
  late String? _posterUrl;

  // Available genres for the selected format
  List<String> _availableGenres = [];
  bool _hasSearched = false; // true after at least one search completed

  @override
  void initState() {
    super.initState();
    final d = widget.detail;
    _format = d.format;
    _genre = d.genre;
    _mediaType = d.mediaType;
    _tmdbId = d.tmdbId;
    _posterUrl = d.posterUrl;
    _searchController.text = d.title;
    _titleController.text = d.title;
    _yearController.text = d.year?.toString() ?? '';
    _loadFormatOptions();
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _searchController.dispose();
    _titleController.dispose();
    _yearController.dispose();
    super.dispose();
  }

  Future<void> _loadFormatOptions() async {
    try {
      final options = await pipeline_api.getFormatOptions();
      if (!mounted) return;
      setState(() {
        _formatOptions = options;
        _updateAvailableGenres();
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _error = 'Failed to load format options');
    }
  }

  void _updateAvailableGenres() {
    // Validate format exists in available options
    final hasFormat = _formatOptions.any((f) => f.name == _format);
    if (!hasFormat && _formatOptions.isNotEmpty) {
      _format = _formatOptions.first.name;
    }
    final match = _formatOptions.where((f) => f.name == _format);
    _availableGenres = match.isNotEmpty ? match.first.genres : [];
    if (!_availableGenres.contains(_genre) && _availableGenres.isNotEmpty) {
      _genre = _availableGenres.first;
    }
  }

  void _onSearchChanged(String query) {
    _debounce?.cancel();
    if (query.trim().isEmpty) {
      setState(() => _results = []);
      return;
    }
    _debounce = Timer(const Duration(milliseconds: 400), () {
      _search(query.trim());
    });
  }

  Future<void> _search(String query) async {
    if (query.isEmpty) return;
    setState(() {
      _searching = true;
      _error = null;
    });
    try {
      // Don't pass year to search — it over-constrains results for corrections
      final results = await pipeline_api.searchTmdbMulti(query: query);
      if (!mounted) return;
      setState(() {
        _results = results;
        _searching = false;
        _hasSearched = true;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = 'Search failed: $e';
        _searching = false;
      });
    }
  }

  void _selectMatch(TmdbMatch match) {
    setState(() {
      _error = null;
      _tmdbId = match.tmdbId;
      _posterUrl = match.posterUrl;
      _mediaType = match.mediaType;
      _results = [];
      _hasSearched = false;
      _searchController.text = match.title;
      _titleController.text = match.title;
      _yearController.text = match.year?.toString() ?? '';
    });
  }

  Future<void> _save() async {
    if (_saving) return;

    // Read from controllers — single source of truth
    final title = _titleController.text.trim();
    final year = int.tryParse(_yearController.text);
    if (title.isEmpty) {
      setState(() => _error = 'Title cannot be empty');
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });

    final files = widget.detail.files;
    int saved = 0;
    try {
      for (final file in files) {
        await pipeline_api.editAndRelocate(
          edit: EditRequest(
            transactionId: file.path, // Rust resolves by dest_path fallback
            title: title,
            year: year,
            format: _format,
            genre: _genre,
            mediaType: _mediaType,
            tmdbId: _tmdbId,
            posterUrl: _posterUrl,
            season: file.season,
            episode: file.episode,
            episodeTitle: file.episodeTitle,
          ),
        );
        saved++;
      }
      if (mounted) Navigator.of(context).pop(true);
    } catch (e) {
      if (!mounted) return;
      if (saved > 0) {
        // Partial success — pop so library refreshes with changes made so far
        Navigator.of(context).pop(true);
        return;
      }
      setState(() {
        _error = 'Save failed: $e';
        _saving = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: AppColors.surface,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520, maxHeight: 600),
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Header
              Row(
                children: [
                  const Icon(Icons.edit_outlined, size: 18, color: AppColors.textSecondary),
                  const SizedBox(width: 8),
                  const Text(
                    'Fix Identification',
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                      color: AppColors.textPrimary,
                    ),
                  ),
                  const Spacer(),
                  MouseRegion(
                    cursor: SystemMouseCursors.click,
                    child: GestureDetector(
                      onTap: () => Navigator.of(context).pop(false),
                      child: const Icon(Icons.close, size: 18, color: AppColors.textQuaternary),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),

              // TMDb search
              _buildSearchField(),
              const SizedBox(height: 8),

              // Search results or selected preview
              Flexible(child: _buildBody()),

              // Error
              if (_error != null) ...[
                const SizedBox(height: 8),
                Text(_error!, style: const TextStyle(fontSize: 12, color: AppColors.error)),
              ],

              const SizedBox(height: 16),

              // Save button
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  MouseRegion(
                    cursor: SystemMouseCursors.click,
                    child: GestureDetector(
                      onTap: () => Navigator.of(context).pop(false),
                      child: Container(
                        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
                        child: const Text('Cancel', style: TextStyle(fontSize: 13, color: AppColors.textTertiary)),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  MouseRegion(
                    cursor: _saving ? SystemMouseCursors.basic : SystemMouseCursors.click,
                    child: GestureDetector(
                      onTap: _saving ? null : _save,
                      child: Container(
                        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
                        decoration: BoxDecoration(
                          color: _saving ? AppColors.primary.withValues(alpha: 0.5) : AppColors.primary,
                          borderRadius: BorderRadius.circular(8),
                        ),
                        child: Text(
                          _saving ? 'Saving...' : 'Save',
                          style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500, color: Colors.white),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildSearchField() {
    return Container(
      decoration: BoxDecoration(
        color: AppColors.surfaceElevated,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppColors.border),
      ),
      child: Row(
        children: [
          const Padding(
            padding: EdgeInsets.only(left: 12),
            child: Icon(Icons.search, size: 16, color: AppColors.textQuaternary),
          ),
          Expanded(
            child: TextField(
              controller: _searchController,
              onChanged: _onSearchChanged,
              onSubmitted: (q) => _search(q.trim()),
              style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
              decoration: const InputDecoration(
                hintText: 'Search TMDb...',
                hintStyle: TextStyle(fontSize: 13, color: AppColors.textQuaternary),
                border: InputBorder.none,
                contentPadding: EdgeInsets.symmetric(horizontal: 8, vertical: 10),
              ),
            ),
          ),
          if (_searching)
            const Padding(
              padding: EdgeInsets.only(right: 12),
              child: SizedBox(width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2)),
            ),
        ],
      ),
    );
  }

  Widget _buildBody() {
    // If we have search results, show them
    if (_results.isNotEmpty) {
      return _buildSearchResults();
    }

    // No results after searching — show hints
    if (_hasSearched && _results.isEmpty && !_searching) {
      return SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: AppColors.surfaceElevated,
                borderRadius: BorderRadius.circular(8),
              ),
              child: const Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'No results found',
                    style: TextStyle(fontSize: 13, fontWeight: FontWeight.w500, color: AppColors.textSecondary),
                  ),
                  SizedBox(height: 6),
                  Text(
                    '\u2022 Try the original language title (e.g. "Sen to Chihiro" instead of "Spirited Away")\n'
                    '\u2022 Try without the year\n'
                    '\u2022 Use the full title, not an abbreviation\n'
                    '\u2022 For sequels, include the number (e.g. "Kung Fu Panda 2")',
                    style: TextStyle(fontSize: 12, color: AppColors.textTertiary, height: 1.5),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
            _buildMetadataForm(),
          ],
        ),
      );
    }

    // Default: show the selected/current metadata for editing
    return SingleChildScrollView(child: _buildMetadataForm());
  }

  Widget _buildSearchResults() {
    return ListView.builder(
      shrinkWrap: true,
      itemCount: _results.length,
      itemBuilder: (context, index) {
        final match = _results[index];
        return MouseRegion(
          cursor: SystemMouseCursors.click,
          child: GestureDetector(
            onTap: () => _selectMatch(match),
            child: Container(
              margin: const EdgeInsets.only(bottom: 4),
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: AppColors.surfaceElevated,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: [
                  // Poster thumbnail
                  ClipRRect(
                    borderRadius: BorderRadius.circular(4),
                    child: SizedBox(
                      width: 36,
                      height: 54,
                      child: match.posterUrl != null
                          ? CachedNetworkImage(
                              imageUrl: match.posterUrl!,
                              fit: BoxFit.cover,
                              errorWidget: (_, __, ___) => Container(color: AppColors.surface),
                            )
                          : Container(color: AppColors.surface),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          '${match.title}${match.year != null ? ' (${match.year})' : ''}',
                          style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500, color: AppColors.textPrimary),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        const SizedBox(height: 2),
                        Text(
                          match.mediaType == 'tv' ? 'TV Show' : 'Movie',
                          style: const TextStyle(fontSize: 11, color: AppColors.textQuaternary),
                        ),
                        if (match.overview != null && match.overview!.isNotEmpty) ...[
                          const SizedBox(height: 2),
                          Text(
                            match.overview!,
                            style: const TextStyle(fontSize: 11, color: AppColors.textTertiary),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _buildMetadataForm() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Poster preview + editable fields
        Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: AppColors.surfaceElevated,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              ClipRRect(
                borderRadius: BorderRadius.circular(6),
                child: SizedBox(
                  width: 60,
                  height: 90,
                  child: _posterUrl != null
                      ? CachedNetworkImage(
                          imageUrl: _posterUrl!,
                          fit: BoxFit.cover,
                          errorWidget: (_, __, ___) => Container(color: AppColors.surface),
                        )
                      : Container(color: AppColors.surface),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    // Editable title
                    _buildTextInput(
                      controller: _titleController,
                      hint: 'Title',
                      style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: AppColors.textPrimary),
                    ),
                    const SizedBox(height: 6),
                    SizedBox(
                      width: 80,
                      child: _buildTextInput(
                        controller: _yearController,
                        hint: 'Year',
                        style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
                      ),
                    ),
                    const SizedBox(height: 6),
                    // Media type toggle
                    Row(
                      children: [
                        _buildTypeChip('Movie', 'movie'),
                        const SizedBox(width: 6),
                        _buildTypeChip('TV Show', 'tv'),
                      ],
                    ),
                    if (_tmdbId != null)
                      Padding(
                        padding: const EdgeInsets.only(top: 4),
                        child: Text(
                          'TMDb: $_tmdbId',
                          style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                        ),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),

        if (_formatOptions.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 8),
            child: Center(child: SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))),
          )
        else ...[
          // Format dropdown
          _buildDropdown(
            label: 'Format',
            value: _format,
            items: _formatOptions.map((f) => f.name).toList(),
            onChanged: (val) {
              setState(() {
                _format = val;
                _updateAvailableGenres();
              });
            },
          ),
          const SizedBox(height: 8),

          // Genre dropdown
          _buildDropdown(
            label: 'Genre',
            value: _genre,
            items: _availableGenres,
            onChanged: (val) => setState(() => _genre = val),
          ),
        ],
      ],
    );
  }

  Widget _buildTextInput({
    required TextEditingController controller,
    required String hint,
    TextStyle? style,
  }) {
    return TextField(
      controller: controller,
      style: style ?? const TextStyle(fontSize: 13, color: AppColors.textPrimary),
      decoration: InputDecoration(
        hintText: hint,
        hintStyle: const TextStyle(fontSize: 13, color: AppColors.textQuaternary),
        isDense: true,
        contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(4),
          borderSide: const BorderSide(color: AppColors.border),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(4),
          borderSide: const BorderSide(color: AppColors.border),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(4),
          borderSide: const BorderSide(color: AppColors.primary),
        ),
      ),
    );
  }

  Widget _buildTypeChip(String label, String type) {
    final isActive = _mediaType == type;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: () => setState(() => _mediaType = type),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
          decoration: BoxDecoration(
            color: isActive ? AppColors.primary.withValues(alpha: 0.15) : Colors.transparent,
            borderRadius: BorderRadius.circular(4),
            border: Border.all(
              color: isActive ? AppColors.primary : AppColors.border,
            ),
          ),
          child: Text(
            label,
            style: TextStyle(
              fontSize: 11,
              color: isActive ? AppColors.primary : AppColors.textQuaternary,
              fontWeight: isActive ? FontWeight.w500 : FontWeight.normal,
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildDropdown({
    required String label,
    required String value,
    required List<String> items,
    required void Function(String) onChanged,
  }) {
    return Row(
      children: [
        SizedBox(
          width: 60,
          child: Text(label, style: const TextStyle(fontSize: 12, color: AppColors.textTertiary)),
        ),
        Expanded(
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            decoration: BoxDecoration(
              color: AppColors.surfaceElevated,
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: AppColors.border),
            ),
            child: DropdownButtonHideUnderline(
              child: DropdownButton<String>(
                value: items.contains(value) ? value : (items.isNotEmpty ? items.first : null),
                isExpanded: true,
                dropdownColor: AppColors.surfaceElevated,
                style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
                icon: const Icon(Icons.expand_more, size: 16, color: AppColors.textQuaternary),
                items: items.map((item) {
                  return DropdownMenuItem(value: item, child: Text(item));
                }).toList(),
                onChanged: (val) {
                  if (val != null) onChanged(val);
                },
              ),
            ),
          ),
        ),
      ],
    );
  }
}
