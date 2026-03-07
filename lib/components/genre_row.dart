import 'package:flutter/material.dart';
import 'package:reel/src/rust/pipeline.dart';
import 'package:reel/components/media_card.dart';
import 'package:reel/components/dock_row.dart';
import 'package:reel/theme/app_theme.dart';

/// A horizontal row showing a genre name, count, "See All" button, and media
/// card samples with macOS Dock-style magnification on hover.
class GenreRow extends StatelessWidget {
  final GenreInfo genre;
  final VoidCallback onSeeAll;
  final void Function(MediaInfo media)? onMediaTap;
  final void Function(MediaInfo media)? onMediaPlay;

  const GenreRow({
    super.key,
    required this.genre,
    required this.onSeeAll,
    this.onMediaTap,
    this.onMediaPlay,
  });

  @override
  Widget build(BuildContext context) {
    return DockRow(
      header: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Row(
              children: [
                Text(
                  genre.name,
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textPrimary,
                  ),
                ),
                const SizedBox(width: 6),
                Text(
                  '(${genre.mediaCount})',
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w400,
                    color: AppColors.textTertiary,
                  ),
                ),
              ],
            ),
            MouseRegion(
              cursor: SystemMouseCursors.click,
              child: GestureDetector(
                onTap: onSeeAll,
                child: const Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      'See All',
                      style: TextStyle(
                        fontSize: 12,
                        color: AppColors.primary,
                      ),
                    ),
                    SizedBox(width: 2),
                    Icon(Icons.chevron_right,
                        size: 14, color: AppColors.primary),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
      itemCount: genre.mediaSamples.length,
      itemBuilder: (context, index) {
        final media = genre.mediaSamples[index];
        return MediaCard.small(
          media: media,
          onTap: () => onMediaTap?.call(media),
          onPlay: () => onMediaPlay?.call(media),
        );
      },
    );
  }
}
