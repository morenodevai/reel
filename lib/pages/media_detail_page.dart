import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/api/library_api.dart' as library_api;
import 'package:reel/src/rust/api/pipeline_api.dart' as pipeline_api;
import 'package:reel/src/rust/api/subtitle_api.dart' as subtitle_api;
import 'package:reel/src/rust/api/playback_api.dart' as playback_api;
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/providers/playback_provider.dart';
import 'package:reel/src/rust/db/watch_progress.dart';
import 'package:reel/pages/media_detail_widgets.dart';
import 'package:reel/theme/app_theme.dart';

/// Media detail page showing full metadata, files, and episode list.
class MediaDetailPageWidget extends ConsumerStatefulWidget {
  final MediaInfo media;
  const MediaDetailPageWidget({super.key, required this.media});

  @override
  ConsumerState<MediaDetailPageWidget> createState() =>
      _MediaDetailPageWidgetState();
}

class _MediaDetailPageWidgetState
    extends ConsumerState<MediaDetailPageWidget> {
  MediaDetail? _detail;
  bool _loading = true;
  int _activeSeason = 1;
  bool _seasonDropdownOpen = false;
  bool _downloadingSubs = false;
  Map<String, WatchProgress> _watchProgress = {};

  @override
  void initState() {
    super.initState();
    _loadDetails();
  }

  Future<void> _loadDetails() async {
    setState(() => _loading = true);
    try {
      final detail =
          await library_api.getMediaDetails(mediaPath: widget.media.path);
      if (!mounted) return;

      // Load watch progress for all episodes
      Map<String, WatchProgress> progressMap = {};
      try {
        final allProgress = await playback_api.loadAllProgress(mediaPath: detail.path);
        progressMap = {for (final p in allProgress) p.filePath: p};
      } catch (e) {
        debugPrint('[detail] Failed to load watch progress: $e');
      }

      // Determine first available season, preferring one with in-progress episodes
      final seasons = _getSeasons(detail);
      int activeSeason = seasons.isNotEmpty ? seasons.first : 1;
      if (detail.mediaType == 'tv') {
        int? inProgressSeason;
        int? firstUnwatchedSeason;
        for (final s in seasons) {
          final seasonFiles = detail.files.where((f) => f.season == s);
          if (inProgressSeason == null) {
            final hasInProgress = seasonFiles.any((f) {
              final wp = progressMap[f.path];
              return wp != null && !wp.completed && wp.positionSeconds > 5.0;
            });
            if (hasInProgress) inProgressSeason = s;
          }
          if (firstUnwatchedSeason == null) {
            final hasUnwatched = seasonFiles.any((f) {
              final wp = progressMap[f.path];
              return wp == null || !wp.completed;
            });
            if (hasUnwatched) firstUnwatchedSeason = s;
          }
        }
        activeSeason = inProgressSeason ?? firstUnwatchedSeason ?? activeSeason;
      }

      setState(() {
        _detail = detail;
        _watchProgress = progressMap;
        _loading = false;
        _activeSeason = activeSeason;
      });
    } catch (e) {
      if (mounted) {
        setState(() => _loading = false);
        ref.read(toastProvider.notifier).show(
              'Failed to load details: $e',
              type: ToastType.error,
            );
      }
    }
  }

  List<int> _getSeasons(MediaDetail detail) {
    final set = <int>{};
    for (final f in detail.files) {
      if (f.season != null) set.add(f.season!);
    }
    final list = set.toList()..sort();
    return list;
  }

  List<MediaFile> _getCurrentEpisodes() {
    if (_detail == null) return [];
    if (_detail!.mediaType == 'movie') return _detail!.files;
    return _detail!.files
        .where((f) => f.season == _activeSeason)
        .toList()
      ..sort((a, b) => (a.episode ?? 0).compareTo(b.episode ?? 0));
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) return const DetailLoadingSkeleton();

    if (_detail == null) {
      return const Center(
        child: Text(
          'Failed to load details',
          style: TextStyle(fontSize: 14, color: AppColors.textTertiary),
        ),
      );
    }

    final detail = _detail!;
    final isTV = detail.mediaType == 'tv';
    final seasons = _getSeasons(detail);
    final currentEpisodes = _getCurrentEpisodes();

    return SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Hero section: poster + metadata
          HeroSection(
            detail: detail,
            isTV: isTV,
            onPlay: () => _handlePlay(detail),
            onReveal: () => _revealInExplorer(detail.path),
            onDownloadSubs: () => _handleDownloadSubs(detail.path),
            downloadingSubs: _downloadingSubs,
            onRestartSeason: isTV ? () => _handleRestartSeason(detail, currentEpisodes) : null,
            onRestartShow: isTV ? () => _handleRestartShow(detail) : null,
          ),

          const SizedBox(height: 32),

          // Season selector for TV shows
          if (isTV && seasons.length > 1) ...[
            SeasonSelector(
              seasons: seasons,
              activeSeason: _activeSeason,
              detail: detail,
              isOpen: _seasonDropdownOpen,
              onToggle: () =>
                  setState(() => _seasonDropdownOpen = !_seasonDropdownOpen),
              onSelect: (s) => setState(() {
                _activeSeason = s;
                _seasonDropdownOpen = false;
              }),
            ),
            const SizedBox(height: 16),
          ],

          // Single season label
          if (isTV && seasons.length == 1) ...[
            Row(
              children: [
                Text(
                  'Season $_activeSeason',
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textPrimary,
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  '${currentEpisodes.length} episode${currentEpisodes.length != 1 ? 's' : ''}',
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textQuaternary,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
          ],

          // Episode / file list
          if (currentEpisodes.isEmpty)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 16),
              child: Center(
                child: Text(
                  'No files found',
                  style: TextStyle(fontSize: 14, color: AppColors.textQuaternary),
                ),
              ),
            )
          else
            ...currentEpisodes.asMap().entries.map((entry) {
              final idx = entry.key;
              final file = entry.value;
              return EpisodeRow(
                file: file,
                isTV: isTV,
                showTitle: detail.title,
                progress: _watchProgress[file.path],
                onPlay: () => _handlePlayFile(file, detail, idx),
              );
            }),

          const SizedBox(height: 48),
        ],
      ),
    );
  }

  Future<void> _handlePlay(MediaDetail detail) async {
    if (detail.files.isEmpty) return;
    final target = await resolvePlayTarget(detail);
    if (target == null || !mounted) return;
    ref.read(navigationProvider.notifier).goToPlayer(detail, target.file, target.playlist, target.index);
  }

  void _handlePlayFile(MediaFile file, MediaDetail detail, int index) {
    final playlist = _getCurrentEpisodes();
    ref.read(navigationProvider.notifier).goToPlayer(detail, file, playlist, index);
  }

  Future<void> _handleRestartSeason(MediaDetail detail, List<MediaFile> currentEpisodes) async {
    for (final f in currentEpisodes) {
      try {
        await playback_api.setFileUnwatched(filePath: f.path);
      } catch (e) {
        debugPrint('[detail] Failed to unwatch ${f.path}: $e');
      }
    }
    if (!mounted) return;
    if (currentEpisodes.isNotEmpty) {
      ref.read(navigationProvider.notifier).goToPlayer(
        detail, currentEpisodes.first, currentEpisodes, 0,
      );
    }
  }

  Future<void> _handleRestartShow(MediaDetail detail) async {
    for (final f in detail.files) {
      try {
        await playback_api.setFileUnwatched(filePath: f.path);
      } catch (e) {
        debugPrint('[detail] Failed to unwatch ${f.path}: $e');
      }
    }
    if (!mounted) return;
    if (detail.files.isNotEmpty) {
      final sorted = List<MediaFile>.from(detail.files)
        ..sort((a, b) {
          final sc = (a.season ?? 0).compareTo(b.season ?? 0);
          return sc != 0 ? sc : (a.episode ?? 0).compareTo(b.episode ?? 0);
        });
      ref.read(navigationProvider.notifier).goToPlayer(
        detail, sorted.first, sorted, 0,
      );
    }
  }

  void _revealInExplorer(String path) {
    pipeline_api.revealInFinder(path: path).catchError((e) {
      if (mounted) {
        ref
            .read(toastProvider.notifier)
            .show('Failed to reveal file: $e', type: ToastType.error);
      }
    });
  }

  Future<void> _handleDownloadSubs(String path) async {
    setState(() => _downloadingSubs = true);
    final toast = ref.read(toastProvider.notifier);
    try {
      final result = await subtitle_api.searchSubtitles(path: path);
      toast.show(result);
    } catch (e) {
      toast.show('Subtitle download failed: $e', type: ToastType.error);
    } finally {
      if (mounted) {
        setState(() => _downloadingSubs = false);
        await _loadDetails();
      }
    }
  }
}
