import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/db/transactions.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/api/review_api.dart' as review_api;
import 'package:reel/src/rust/api/history_api.dart' as history_api;
import 'package:reel/pages/edit_media_dialog.dart';
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/library_provider.dart';
import 'package:reel/utils/clear_pending.dart';
import 'package:reel/components/loading_skeleton.dart';
import 'package:reel/theme/app_theme.dart';
import 'package:reel/components/action_button.dart';

class ReviewPageWidget extends ConsumerStatefulWidget {
  final List<String> batchIds;
  const ReviewPageWidget({super.key, required this.batchIds});

  @override
  ConsumerState<ReviewPageWidget> createState() => _ReviewPageWidgetState();
}

class _ReviewPageWidgetState extends ConsumerState<ReviewPageWidget> {
  List<Transaction> _items = [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final data =
          await review_api.getReviewItems(batchIds: widget.batchIds);
      if (mounted) setState(() { _items = data; _loading = false; });
    } catch (e) {
      debugPrint('[review] Load failed: $e');
      if (mounted) {
        setState(() { _items = []; _loading = false; });
        ref.read(toastProvider.notifier).show('Failed to load review items: $e', type: ToastType.error);
      }
    }
  }

  Future<void> _approveAll() async {
    // Don't bulk-approve "Needs Review" items — they need individual confirmation
    final autoItems = _items.where((t) => t.format != 'Needs Review').toList();
    final reviewItems = _items.where((t) => t.format == 'Needs Review').toList();
    if (autoItems.isEmpty && reviewItems.isNotEmpty) {
      ref.read(toastProvider.notifier).show(
        'Review items need individual confirmation — use Fix ID',
        type: ToastType.info,
      );
      return;
    }
    final toast = ref.read(toastProvider.notifier);
    try {
      final ids = autoItems.map((t) => t.id).toList();
      await review_api.lockTransactions(ids: ids);
      setState(() => _items = reviewItems);
      toast.show('Approved ${autoItems.length} item(s)');
    } catch (e) {
      toast.show('Failed to approve: $e', type: ToastType.error);
    }
  }

  Future<void> _clearAll() async {
    if (await clearAllPendingAndReport(ref)) {
      setState(() => _items = []);
    }
  }

  Future<void> _approveGroup(List<String> ids) async {
    final toast = ref.read(toastProvider.notifier);
    try {
      await review_api.lockTransactions(ids: ids);
      setState(() => _items.removeWhere((t) => ids.contains(t.id)));
    } catch (e) {
      toast.show('Failed to approve: $e', type: ToastType.error);
    }
  }

  Future<void> _undoGroup(List<String> ids) async {
    final toast = ref.read(toastProvider.notifier);
    try {
      final results = await Future.wait(
        ids.map((id) => history_api.undoTransaction(id: id)),
      );
      final succeededIds = <String>[];
      for (var i = 0; i < results.length; i++) {
        if (results[i].success) succeededIds.add(ids[i]);
      }
      final succeeded = succeededIds.length;
      final failed = results.length - succeeded;
      setState(() => _items.removeWhere((t) => succeededIds.contains(t.id)));
      ref.read(libraryProvider.notifier).refresh();
      if (ids.length == 1) {
        toast.show('File moved back to original location');
      } else {
        final msg = failed > 0
            ? 'Undone $succeeded of ${ids.length} ($failed failed)'
            : 'Undone $succeeded items';
        toast.show(msg, type: failed > 0 ? ToastType.error : ToastType.success);
      }
    } catch (e) {
      toast.show('Undo failed: $e', type: ToastType.error);
    }
  }

  /// Open the Fix ID dialog for a "Needs Review" group.
  /// Builds a synthetic MediaDetail from the transaction data so the dialog can work.
  Future<void> _fixGroup(_TitleGroup group) async {
    final txns = group.transactions;
    final first = txns.first;

    // Build a synthetic MediaDetail for the edit dialog
    final detail = MediaDetail(
      title: first.title,
      year: first.year,
      path: first.destPath,
      posterUrl: first.posterUrl,
      tmdbId: first.tmdbId,
      format: first.format,
      genre: first.genre,
      mediaType: first.mediaType,
      seasonCount: 0,
      episodeCount: txns.length,
      files: txns.map((t) => MediaFile(
        path: t.destPath,
        filename: t.destPath.split(RegExp(r'[\\/]')).last,
        season: t.season,
        episode: t.episode,
        episodeTitle: t.episodeTitle,
        sizeBytes: BigInt.zero,
        hasSubtitles: false,
      )).toList(),
    );

    final result = await showDialog<bool>(
      context: context,
      builder: (ctx) => EditMediaDialog(detail: detail),
    );
    if (result == true && mounted) {
      ref.read(libraryProvider.notifier).refresh();
      await _load(); // Reload — fixed items will have new transactions
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const _ReviewLoadingSkeleton();
    }

    if (_items.isEmpty) {
      return const Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.check_circle_outline, size: 40, color: AppColors.success),
            SizedBox(height: 12),
            Text(
              'All items reviewed',
              style: TextStyle(fontSize: 14, color: AppColors.textSecondary),
            ),
          ],
        ),
      );
    }

    // Group by title, sort "Needs Review" items first
    final groups = _groupByTitle(_items);
    groups.sort((a, b) {
      final aReview = a.format == 'Needs Review' ? 0 : 1;
      final bReview = b.format == 'Needs Review' ? 0 : 1;
      if (aReview != bReview) return aReview.compareTo(bReview);
      return a.confidence.compareTo(b.confidence);
    });

    final reviewCount = groups.where((g) => g.format == 'Needs Review').length;
    final autoCount = groups.length - reviewCount;

    return Column(
      children: [
        // Top bar
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '${groups.length} title${groups.length != 1 ? 's' : ''} (${_items.length} file${_items.length != 1 ? 's' : ''})',
                    style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
                  ),
                  if (reviewCount > 0)
                    Text(
                      '$reviewCount need${reviewCount == 1 ? 's' : ''} review',
                      style: const TextStyle(fontSize: 11, color: AppColors.warning),
                    ),
                ],
              ),
              Row(
                children: [
                  ActionButton(
                    icon: Icons.delete_outline,
                    label: 'Undo All',
                    color: AppColors.error,
                    onTap: _clearAll,
                  ),
                  if (autoCount > 0) ...[
                    const SizedBox(width: 8),
                    ActionButton(
                      icon: Icons.check_circle_outline,
                      label: 'Approve All',
                      color: AppColors.success,
                      onTap: _approveAll,
                    ),
                  ],
                ],
              ),
            ],
          ),
        ),

        // Review cards
        Expanded(
          child: ListView.separated(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            itemCount: groups.length,
            separatorBuilder: (_, __) => const SizedBox(height: 8),
            itemBuilder: (context, index) {
              final group = groups[index];
              final isReview = group.format == 'Needs Review';
              return _ReviewGroupCard(
                group: group,
                onApprove: isReview
                    ? null // No blind approve for triage items
                    : () => _approveGroup(group.transactions.map((t) => t.id).toList()),
                onUndo: () =>
                    _undoGroup(group.transactions.map((t) => t.id).toList()),
                onFix: isReview ? () => _fixGroup(group) : null,
              );
            },
          ),
        ),
      ],
    );
  }

  List<_TitleGroup> _groupByTitle(List<Transaction> items) {
    final map = <String, _TitleGroup>{};
    for (final item in items) {
      final key = item.tmdbId != null
          ? 'tmdb-${item.tmdbId}-${item.format}'
          : '${item.title}-${item.year ?? ""}-${item.format}';
      map.putIfAbsent(
        key,
        () => _TitleGroup(
          title: item.title,
          year: item.year,
          format: item.format,
          genre: item.genre,
          mediaType: item.mediaType,
          posterUrl: item.posterUrl,
          confidence: item.confidence,
          transactions: [],
        ),
      );
      map[key]!.transactions.add(item);
      if (item.confidence < map[key]!.confidence) {
        map[key]!.confidence = item.confidence;
      }
      if (map[key]!.posterUrl == null && item.posterUrl != null) {
        map[key]!.posterUrl = item.posterUrl;
      }
    }
    return map.values.toList();
  }
}

class _TitleGroup {
  final String title;
  final int? year;
  final String format;
  final String genre;
  final String mediaType;
  String? posterUrl;
  double confidence;
  final List<Transaction> transactions;

  _TitleGroup({
    required this.title,
    this.year,
    required this.format,
    required this.genre,
    required this.mediaType,
    this.posterUrl,
    required this.confidence,
    required this.transactions,
  });
}

class _ReviewGroupCard extends StatefulWidget {
  final _TitleGroup group;
  final VoidCallback? onApprove;
  final VoidCallback onUndo;
  final VoidCallback? onFix;

  const _ReviewGroupCard({
    required this.group,
    this.onApprove,
    required this.onUndo,
    this.onFix,
  });

  @override
  State<_ReviewGroupCard> createState() => _ReviewGroupCardState();
}

class _ReviewGroupCardState extends State<_ReviewGroupCard> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final group = widget.group;
    final isReview = group.format == 'Needs Review';
    final confidenceColor = group.confidence >= 0.8
        ? AppColors.success
        : group.confidence >= 0.5
            ? AppColors.warning
            : AppColors.error;

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppColors.surface.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(
          color: isReview ? AppColors.warning.withValues(alpha: 0.4) : AppColors.border,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              // Poster
              ClipRRect(
                borderRadius: BorderRadius.circular(6),
                child: SizedBox(
                  width: 48,
                  height: 72,
                  child: group.posterUrl != null
                      ? CachedNetworkImage(
                          imageUrl: group.posterUrl!,
                          fit: BoxFit.cover,
                          placeholder: (_, __) =>
                              Container(color: AppColors.surfaceElevated),
                          errorWidget: (_, __, ___) =>
                              Container(color: AppColors.surfaceElevated),
                        )
                      : Container(
                          color: AppColors.surfaceElevated,
                          child: const Icon(Icons.help_outline, size: 20, color: AppColors.textQuaternary),
                        ),
                ),
              ),
              const SizedBox(width: 12),

              // Info
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (isReview)
                      const Text(
                        'Is this correct?',
                        style: TextStyle(fontSize: 10, fontWeight: FontWeight.w600, color: AppColors.warning),
                      ),
                    Text(
                      '${group.title}${group.year != null ? ' (${group.year})' : ''}',
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                        color: AppColors.textPrimary,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      isReview
                          ? '${group.mediaType == "tv" ? "TV Show" : "Movie"} — low confidence'
                          : '${group.format} / ${group.genre}',
                      style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
                    ),
                    const SizedBox(height: 2),
                    Row(
                      children: [
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                          decoration: BoxDecoration(
                            color: confidenceColor.withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(
                            '${(group.confidence * 100).toInt()}%',
                            style: TextStyle(
                              fontSize: 10,
                              fontWeight: FontWeight.w600,
                              color: confidenceColor,
                            ),
                          ),
                        ),
                        const SizedBox(width: 8),
                        Text(
                          '${group.transactions.length} file${group.transactions.length != 1 ? 's' : ''}',
                          style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                        ),
                        if (group.transactions.length > 1) ...[
                          const SizedBox(width: 6),
                          MouseRegion(
                            cursor: SystemMouseCursors.click,
                            child: GestureDetector(
                              onTap: () => setState(() => _expanded = !_expanded),
                              child: Icon(
                                _expanded ? Icons.expand_less : Icons.expand_more,
                                size: 16,
                                color: AppColors.textQuaternary,
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
              ),

              // Action buttons
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  _SmallButton(
                    icon: Icons.undo,
                    onTap: widget.onUndo,
                    color: AppColors.textSecondary,
                  ),
                  if (widget.onFix != null) ...[
                    const SizedBox(width: 4),
                    _SmallButton(
                      icon: Icons.edit_outlined,
                      onTap: widget.onFix!,
                      color: AppColors.primary,
                      label: 'Fix ID',
                    ),
                  ],
                  if (widget.onApprove != null) ...[
                    const SizedBox(width: 4),
                    _SmallButton(
                      icon: Icons.check,
                      onTap: widget.onApprove!,
                      color: AppColors.success,
                    ),
                  ],
                ],
              ),
            ],
          ),

          // Expanded file list — shows source filename → proposed name
          if (_expanded || (isReview && group.transactions.length == 1))
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Column(
                children: group.transactions.map((t) {
                  final sourceFile = t.sourcePath.split(RegExp(r'[\\/]')).last;
                  final destFile = t.destPath.split(RegExp(r'[\\/]')).last;
                  return Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: Row(
                      children: [
                        const SizedBox(width: 60), // align with content
                        Expanded(
                          child: RichText(
                            overflow: TextOverflow.ellipsis,
                            text: TextSpan(
                              style: const TextStyle(fontSize: 11, fontFamily: 'monospace'),
                              children: [
                                TextSpan(
                                  text: sourceFile,
                                  style: const TextStyle(color: AppColors.textTertiary),
                                ),
                                const TextSpan(
                                  text: '  →  ',
                                  style: TextStyle(color: AppColors.textQuaternary),
                                ),
                                TextSpan(
                                  text: destFile,
                                  style: const TextStyle(color: AppColors.textSecondary),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ],
                    ),
                  );
                }).toList(),
              ),
            ),
        ],
      ),
    );
  }
}

class _SmallButton extends StatelessWidget {
  final IconData icon;
  final VoidCallback onTap;
  final Color color;
  final String? label;
  const _SmallButton({required this.icon, required this.onTap, required this.color, this.label});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
          ),
          child: label != null
              ? Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(icon, size: 14, color: color),
                    const SizedBox(width: 4),
                    Text(label!, style: TextStyle(fontSize: 11, fontWeight: FontWeight.w500, color: color)),
                  ],
                )
              : Icon(icon, size: 18, color: color),
        ),
      ),
    );
  }
}

class _ReviewLoadingSkeleton extends StatelessWidget {
  const _ReviewLoadingSkeleton();

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: List.generate(
        3,
        (i) => Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: SkeletonBox(height: 85, borderRadius: 12),
        ),
      ),
    );
  }
}
