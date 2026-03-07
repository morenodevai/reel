import 'package:flutter/material.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/pipeline.dart';
import 'package:reel/theme/app_theme.dart';

/// Card for a format (Movies, Shows, Anime, etc.) with icon, name, counts, and poster samples.
class FormatCard extends StatefulWidget {
  final FormatInfo format;
  final VoidCallback onTap;

  const FormatCard({super.key, required this.format, required this.onTap});

  @override
  State<FormatCard> createState() => _FormatCardState();
}

class _FormatCardState extends State<FormatCard> {
  bool _hovered = false;

  static const _formatIcons = <String, IconData>{
    'Movies': Icons.movie_outlined,
    'Shows': Icons.tv_outlined,
    'Anime': Icons.auto_awesome_outlined,
    'Anime Movies': Icons.auto_awesome_outlined,
    'Animated Movies': Icons.animation_outlined,
    'Animated Shows': Icons.animation_outlined,
    'Documentary': Icons.movie_outlined,
  };

  static const _formatAccent = <String, Color>{
    'Movies': AppColors.movie,
    'Shows': AppColors.primary,
    'Anime': AppColors.anime,
    'Anime Movies': AppColors.anime,
    'Animated Movies': AppColors.warning,
    'Animated Shows': AppColors.info,
    'Documentary': AppColors.success,
  };

  @override
  Widget build(BuildContext context) {
    final format = widget.format;
    final icon = _formatIcons[format.name] ?? Icons.movie_outlined;
    final accent = _formatAccent[format.name] ?? AppColors.primary;

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          transform: Matrix4.identity()..scale(_hovered ? 1.02 : 1.0),
          transformAlignment: Alignment.center,
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: _hovered
                ? AppColors.surfaceHover.withValues(alpha: 0.5)
                : AppColors.surfaceElevated.withValues(alpha: 0.5),
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: AppColors.border),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Icon + count row
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: accent.withValues(alpha: 0.2),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Icon(icon, size: 20, color: accent),
                  ),
                  Text(
                    '${format.mediaCount} ${format.mediaCount == 1 ? 'item' : 'items'}',
                    style: const TextStyle(
                      fontSize: 12,
                      color: AppColors.textTertiary,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),

              // Name
              Text(
                format.name,
                style: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                  color: AppColors.textPrimary,
                ),
              ),
              const SizedBox(height: 2),
              Text(
                '${format.genreCount} ${format.genreCount == 1 ? 'genre' : 'genres'}',
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textTertiary,
                ),
              ),

              // Poster samples
              if (format.posterSamples.isNotEmpty) ...[
                const SizedBox(height: 12),
                Row(
                  children: format.posterSamples.take(4).map((url) {
                    return Padding(
                      padding: const EdgeInsets.only(right: 4),
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(4),
                        child: SizedBox(
                          width: 40,
                          height: 60,
                          child: CachedNetworkImage(
                            imageUrl: url,
                            fit: BoxFit.cover,
                            placeholder: (_, __) => Container(
                              color: AppColors.surfaceElevated,
                            ),
                            errorWidget: (_, __, ___) => Container(
                              color: AppColors.surfaceElevated,
                            ),
                          ),
                        ),
                      ),
                    );
                  }).toList(),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
