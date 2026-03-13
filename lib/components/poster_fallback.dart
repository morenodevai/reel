import 'package:flutter/material.dart';
import 'package:reel/theme/app_theme.dart';

/// Gradient fallback shown when a poster image is unavailable.
/// Used by both media cards (compact) and detail pages (standard).
class PosterFallback extends StatelessWidget {
  final String title;
  final List<Color> gradient;
  final bool compact;

  const PosterFallback({
    super.key,
    required this.title,
    required this.gradient,
    this.compact = false,
  });

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
      padding: EdgeInsets.all(compact ? 12 : 16),
      child: Text(
        title,
        style: TextStyle(
          fontSize: compact ? 12 : 14,
          fontWeight: FontWeight.w500,
          color: AppColors.textSecondary,
          height: 1.3,
        ),
      ),
    );
  }
}
