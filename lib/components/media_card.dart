import 'package:flutter/material.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/components/poster_fallback.dart';
import 'package:reel/theme/app_theme.dart';

/// Card that displays a media item with poster, title, and year.
/// Used in genre rows, media grids, and recently added.
class MediaCard extends StatefulWidget {
  final MediaInfo media;
  final double width;
  final double height;
  final VoidCallback? onTap;
  final VoidCallback? onPlay;

  const MediaCard({
    super.key,
    required this.media,
    this.width = 120,
    this.height = 180,
    this.onTap,
    this.onPlay,
  });

  /// Small size for horizontal scrolling rows.
  const MediaCard.small({
    super.key,
    required this.media,
    this.onTap,
    this.onPlay,
  })  : width = 120,
        height = 180;

  /// Medium size for grid view.
  const MediaCard.medium({
    super.key,
    required this.media,
    this.onTap,
    this.onPlay,
  })  : width = 160,
        height = 240;

  @override
  State<MediaCard> createState() => _MediaCardState();
}

class _MediaCardState extends State<MediaCard> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final media = widget.media;
    final gradient = AppColors.formatGradients[media.format] ??
        AppColors.formatGradientDefault;

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: SizedBox(
          width: widget.width,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              // Poster
              AnimatedContainer(
                duration: const Duration(milliseconds: 150),
                transform: _hovered ? (Matrix4.identity()..scaleByDouble(1.03, 1.03, 1.03, 1.0)) : Matrix4.identity(),
                transformAlignment: Alignment.center,
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(8),
                  child: SizedBox(
                    width: widget.width,
                    height: widget.height,
                    child: Stack(
                      fit: StackFit.expand,
                      children: [
                        // Poster image or gradient fallback
                        if (media.posterUrl != null)
                          CachedNetworkImage(
                            imageUrl: media.posterUrl!,
                            fit: BoxFit.cover,
                            placeholder: (_, __) => Container(
                              decoration: BoxDecoration(
                                gradient: LinearGradient(
                                  begin: Alignment.topLeft,
                                  end: Alignment.bottomRight,
                                  colors: gradient,
                                ),
                              ),
                            ),
                            errorWidget: (_, __, ___) => PosterFallback(compact: true,
                              title: media.title,
                              gradient: gradient,
                            ),
                          )
                        else
                          PosterFallback(compact: true,
                            title: media.title,
                            gradient: gradient,
                          ),

                        // Hover overlay with play button
                        if (_hovered) ...[
                          Container(color: Colors.black.withValues(alpha: 0.4)),
                          Center(
                            child: GestureDetector(
                              behavior: HitTestBehavior.opaque,
                              onTap: () {
                                // Only trigger play, not the parent onTap
                                if (widget.onPlay != null) {
                                  widget.onPlay!.call();
                                } else {
                                  widget.onTap?.call();
                                }
                              },
                              child: Container(
                                width: 44,
                                height: 44,
                                decoration: BoxDecoration(
                                  color: Colors.white.withValues(alpha: 0.2),
                                  shape: BoxShape.circle,
                                ),
                                child: const Icon(
                                  Icons.play_arrow_rounded,
                                  size: 28,
                                  color: Colors.white,
                                ),
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
              ),

              // Title and year
              Padding(
                padding: const EdgeInsets.only(top: 6, left: 2, right: 2),
                child: Text(
                  media.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                    color: AppColors.textPrimary,
                  ),
                ),
              ),
              if (media.year != null)
                Padding(
                  padding: const EdgeInsets.only(left: 2),
                  child: Text(
                    '${media.year}',
                    style: const TextStyle(
                      fontSize: 10,
                      color: AppColors.textTertiary,
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

