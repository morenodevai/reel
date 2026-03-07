import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:reel/theme/app_theme.dart';

/// Animated shimmer placeholder for loading states.
class SkeletonBox extends StatefulWidget {
  final double width;
  final double height;
  final double borderRadius;

  const SkeletonBox({
    super.key,
    this.width = double.infinity,
    required this.height,
    this.borderRadius = 8,
  });

  @override
  State<SkeletonBox> createState() => _SkeletonBoxState();
}

class _SkeletonBoxState extends State<SkeletonBox>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1500),
    )..repeat();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _ShimmerBuilder(
      animation: _controller,
      builder: (context, child) {
        final opacity =
            0.3 + 0.2 * math.sin(_controller.value * 2 * math.pi);
        return Container(
          width: widget.width,
          height: widget.height,
          decoration: BoxDecoration(
            color: AppColors.surface.withValues(alpha: opacity),
            borderRadius: BorderRadius.circular(widget.borderRadius),
          ),
        );
      },
    );
  }
}

/// Simple AnimatedWidget wrapper (private to avoid shadowing Flutter SDK's AnimatedBuilder).
class _ShimmerBuilder extends AnimatedWidget {
  final Widget Function(BuildContext, Widget?) builder;
  const _ShimmerBuilder({
    required Animation<double> animation,
    required this.builder,
  }) : super(listenable: animation);

  @override
  Widget build(BuildContext context) {
    return builder(context, null);
  }
}
