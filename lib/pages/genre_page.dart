import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/pipeline.dart';
import 'package:reel/src/rust/api/library_api.dart' as library_api;
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/components/genre_row.dart';
import 'package:reel/components/empty_state.dart';
import 'package:reel/components/loading_skeleton.dart';

class GenrePageWidget extends ConsumerStatefulWidget {
  final String formatName;
  final String formatPath;

  const GenrePageWidget({
    super.key,
    required this.formatName,
    required this.formatPath,
  });

  @override
  ConsumerState<GenrePageWidget> createState() => _GenrePageWidgetState();
}

class _GenrePageWidgetState extends ConsumerState<GenrePageWidget> {
  List<GenreInfo>? _genres;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final genres =
          await library_api.getFormatContents(formatPath: widget.formatPath);
      if (mounted) setState(() { _genres = genres; _loading = false; });
    } catch (e) {
      if (mounted) setState(() { _genres = []; _loading = false; });
    }
  }

  @override
  Widget build(BuildContext context) {
    final nav = ref.read(navigationProvider.notifier);

    if (_loading) {
      return _GenreLoadingSkeleton();
    }

    if (_genres == null || _genres!.isEmpty) {
      return EmptyState(
        icon: Icons.movie_outlined,
        title: 'No ${widget.formatName} yet',
        subtitle: 'Drop some media on the home screen to get started',
      );
    }

    return ListView.separated(
      clipBehavior: Clip.none,
      padding: const EdgeInsets.symmetric(vertical: 16),
      itemCount: _genres!.length,
      separatorBuilder: (_, __) => const SizedBox(height: 24),
      itemBuilder: (context, index) {
        final genre = _genres![index];
        return GenreRow(
          genre: genre,
          onSeeAll: () => nav.goToMedia(
            formatName: widget.formatName,
            genreName: genre.name,
            genrePath: genre.path,
          ),
          onMediaTap: (media) => nav.goToMediaDetail(media),
        );
      },
    );
  }
}

class _GenreLoadingSkeleton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: List.generate(3, (i) {
        return Padding(
          padding: const EdgeInsets.only(bottom: 32),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const SkeletonBox(width: 130, height: 16),
              const SizedBox(height: 12),
              Row(
                children: List.generate(
                  5,
                  (j) => Padding(
                    padding: const EdgeInsets.only(right: 12),
                    child: SkeletonBox(width: 120, height: 180, borderRadius: 8),
                  ),
                ),
              ),
            ],
          ),
        );
      }),
    );
  }
}
