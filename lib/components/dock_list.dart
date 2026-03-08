import 'dart:async';
import 'dart:math';
import 'package:flutter/material.dart';

/// Dock-style magnification list.
///
/// Cards under the cursor swell (maxScale), adjacent cards scale proportionally
/// less via cosine bell curve falloff. Includes a dwell timer to prevent the
/// "scanner" effect during vertical scrolling.
class DockList extends StatefulWidget {
  final int itemCount;
  final IndexedWidgetBuilder itemBuilder;
  final double itemWidth;
  final double containerHeight;
  final double spacing;
  final EdgeInsets padding;
  final double maxScale;
  final double effectRadius;
  final Duration dwellDuration;
  final Duration fadeInDuration;
  final Duration fadeOutDuration;
  final ValueChanged<bool>? onActiveChanged;

  const DockList({
    super.key,
    required this.itemCount,
    required this.itemBuilder,
    this.itemWidth = 120,
    this.containerHeight = 230,
    this.spacing = 12,
    this.padding = const EdgeInsets.symmetric(horizontal: 16),
    this.maxScale = 1.3,
    this.effectRadius = 200,
    this.dwellDuration = const Duration(milliseconds: 150),
    this.fadeInDuration = const Duration(milliseconds: 300),
    this.fadeOutDuration = const Duration(milliseconds: 300),
    this.onActiveChanged,
  });

  @override
  State<DockList> createState() => _DockListState();
}

class _DockListState extends State<DockList>
    with SingleTickerProviderStateMixin {
  late AnimationController _effectController;
  final ScrollController _scrollController = ScrollController();
  double? _cursorX;
  Timer? _dwellTimer;
  bool _active = false;

  @override
  void initState() {
    super.initState();
    _effectController = AnimationController(
      vsync: this,
      duration: widget.fadeInDuration,
      reverseDuration: widget.fadeOutDuration,
    );
    _scrollController.addListener(_onScroll);
  }

  @override
  void dispose() {
    _dwellTimer?.cancel();
    _effectController.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  /// Empty setState forces a rebuild so _scaleFor recalculates each item's
  /// magnification against the updated scroll offset.
  void _onScroll() {
    if (_effectController.value > 0 && _cursorX != null) {
      setState(() {});
    }
  }

  double _scaleFor(int index) {
    if (_cursorX == null || _effectController.value == 0) return 1.0;
    final scrollOffset =
        _scrollController.hasClients ? _scrollController.offset : 0.0;
    final center = widget.padding.left +
        index * (widget.itemWidth + widget.spacing) +
        widget.itemWidth / 2 -
        scrollOffset;
    final dist = (_cursorX! - center).abs();
    if (dist >= widget.effectRadius) return 1.0;
    final t = dist / widget.effectRadius;
    final raw =
        1.0 + (widget.maxScale - 1.0) * (cos(t * pi) + 1) / 2;
    return 1.0 + (raw - 1.0) * _effectController.value;
  }

  void _onEnter(PointerEvent event) {
    _cursorX = event.localPosition.dx;
    _dwellTimer?.cancel();
    _dwellTimer = Timer(widget.dwellDuration, () {
      if (!mounted || _cursorX == null) return;
      _effectController.forward();
      if (!_active) {
        _active = true;
        widget.onActiveChanged?.call(true);
      }
    });
  }

  void _onHover(PointerEvent event) {
    _cursorX = event.localPosition.dx;
    setState(() {});
  }

  void _onExit(PointerEvent event) {
    _dwellTimer?.cancel();
    _cursorX = null;
    _effectController.reverse();
    if (_active) {
      _active = false;
      widget.onActiveChanged?.call(false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: _onEnter,
      onHover: _onHover,
      onExit: _onExit,
      child: SizedBox(
        height: widget.containerHeight,
        child: AnimatedBuilder(
          animation: _effectController,
          builder: (context, _) {
            return SingleChildScrollView(
              controller: _scrollController,
              scrollDirection: Axis.horizontal,
              clipBehavior: Clip.none,
              padding: widget.padding,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: List.generate(widget.itemCount, (index) {
                  final scale = _scaleFor(index);
                  return Padding(
                    padding: EdgeInsets.only(
                      right:
                          index < widget.itemCount - 1 ? widget.spacing : 0,
                    ),
                    child: SizedBox(
                      width: widget.itemWidth * scale,
                      child: FittedBox(
                        fit: BoxFit.fitWidth,
                        alignment: Alignment.bottomLeft,
                        child: SizedBox(
                          width: widget.itemWidth,
                          child: widget.itemBuilder(context, index),
                        ),
                      ),
                    ),
                  );
                }),
              ),
            );
          },
        ),
      ),
    );
  }
}
