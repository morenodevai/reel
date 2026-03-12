import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/api/library_api.dart' as library_api;
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/components/media_card.dart';
import 'package:reel/components/loading_skeleton.dart';
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/utils/play_media.dart';

/// Grid page showing all media items in a genre, with infinite scroll.
class MediaPageWidget extends ConsumerStatefulWidget {
  final String genrePath;

  const MediaPageWidget({super.key, required this.genrePath});

  @override
  ConsumerState<MediaPageWidget> createState() => _MediaPageWidgetState();
}

class _MediaPageWidgetState extends ConsumerState<MediaPageWidget> {
  final List<MediaInfo> _items = [];
  bool _hasMore = true;
  bool _loading = true;
  bool _loadingMore = false;

  @override
  void initState() {
    super.initState();
    _loadPage(0);
  }

  bool _onScroll(ScrollNotification notification) {
    if (notification.metrics.pixels >=
            notification.metrics.maxScrollExtent - 300 &&
        _hasMore &&
        !_loadingMore) {
      _loadPage(_items.length);
    }
    return false;
  }

  Future<void> _loadPage(int offset) async {
    if (_loadingMore) return;
    setState(() {
      if (offset == 0) _loading = true;
      _loadingMore = true;
    });

    try {
      final page = await library_api.getGenreContents(
        genrePath: widget.genrePath,
        offset: offset,
        limit: 30,
      );
      if (mounted) {
        setState(() {
          if (offset == 0) {
            _items.clear();
          }
          _items.addAll(page.items);
          _hasMore = page.hasMore;
          _loading = false;
          _loadingMore = false;
        });
      }
    } catch (e) {
      debugPrint('[media] Load failed: $e');
      if (mounted) {
        setState(() {
          _loading = false;
          _loadingMore = false;
          _hasMore = false;
        });
        if (offset == 0) {
          ref.read(toastProvider.notifier).show('Failed to load media: $e', type: ToastType.error);
        }
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading && _items.isEmpty) {
      return _MediaLoadingSkeleton();
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final crossAxisCount = constraints.maxWidth > 900
            ? 5
            : constraints.maxWidth > 700
                ? 4
                : constraints.maxWidth > 450
                    ? 3
                    : 3;
        final spacing = 12.0;
        final cardWidth =
            (constraints.maxWidth - 32 - spacing * (crossAxisCount - 1)) /
                crossAxisCount;
        final cardHeight = cardWidth * 1.5; // 2:3 aspect ratio

        return NotificationListener<ScrollNotification>(
          onNotification: _onScroll,
          child: CustomScrollView(
            key: PageStorageKey('media:${widget.genrePath}'),
            slivers: [
              SliverPadding(
                padding: const EdgeInsets.all(16),
                sliver: SliverGrid(
                  delegate: SliverChildBuilderDelegate(
                    (context, index) {
                      final media = _items[index];
                      return MediaCard(
                        media: media,
                        width: cardWidth,
                        height: cardHeight,
                        onTap: () {
                          ref.read(navigationProvider.notifier).goToMediaDetail(media);
                        },
                        onPlay: () => playMedia(media, ref, isMounted: () => mounted),
                      );
                    },
                    childCount: _items.length,
                  ),
                  gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: crossAxisCount,
                    mainAxisSpacing: spacing,
                    crossAxisSpacing: spacing,
                    childAspectRatio: cardWidth / (cardHeight + 40), // account for title text
                  ),
                ),
              ),

              // Loading spinner at bottom
              if (_hasMore)
                const SliverToBoxAdapter(
                  child: Padding(
                    padding: EdgeInsets.all(24),
                    child: Center(
                      child: SizedBox(
                        width: 24,
                        height: 24,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                    ),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

class _MediaLoadingSkeleton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Wrap(
        spacing: 12,
        runSpacing: 16,
        children: List.generate(15, (i) {
          return Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              SkeletonBox(width: 120, height: 180, borderRadius: 8),
              const SizedBox(height: 6),
              SkeletonBox(width: 90, height: 12, borderRadius: 4),
            ],
          );
        }),
      ),
    );
  }
}
