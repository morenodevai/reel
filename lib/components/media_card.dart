import 'package:flutter/material.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/pipeline.dart';
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

  static const _formatGradients = <String, List<Color>>{
    'Movies': [Color(0x99783F04), Color(0x4D78350F)],
    'Shows': [Color(0x991E3A5F), Color(0x4D1E3A5F)],
    'Anime': [Color(0x994A1D96), Color(0x4D4A1D96)],
    'Anime Movies': [Color(0x994A1D96), Color(0x4D4A1D96)],
    'Animated Movies': [Color(0x997C2D12), Color(0x4D7C2D12)],
    'Animated Shows': [Color(0x99164E63), Color(0x4D164E63)],
    'Documentary': [Color(0x99166534), Color(0x4D166534)],
  };

  @override
  Widget build(BuildContext context) {
    final media = widget.media;
    final gradient = _formatGradients[media.format] ??
        const [Color(0x991A1A2E), Color(0x4D1A1A2E)];

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
                transform: Matrix4.identity()..scale(_hovered ? 1.03 : 1.0),
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
                            errorWidget: (_, __, ___) => _FallbackPoster(
                              title: media.title,
                              gradient: gradient,
                            ),
                          )
                        else
                          _FallbackPoster(
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

class _FallbackPoster extends StatelessWidget {
  final String title;
  final List<Color> gradient;
  const _FallbackPoster({required this.title, required this.gradient});

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: gradient,
        ),
      ),
      alignment: Alignment.bottomLeft,
      padding: const EdgeInsets.all(12),
      child: Text(
        title,
        style: const TextStyle(
          fontSize: 12,
          fontWeight: FontWeight.w500,
          color: AppColors.textSecondary,
          height: 1.3,
        ),
      ),
    );
  }
}
