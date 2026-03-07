import 'package:flutter/material.dart';
import 'package:reel/components/dock_list.dart';

/// A horizontal row with an animated header and a [DockList] beneath it.
///
/// Extracts the shared layout of header + Dock magnification list used by
/// genre rows, recently-added rows, and any future row that pairs a label
/// with a horizontally-scrolling card strip.
///
/// The [header] widget is wrapped in [AnimatedOpacity] automatically -- it
/// fades to 60 % when the dock is idle and snaps to full opacity when the
/// user hovers over the card strip.
class DockRow extends StatefulWidget {
  /// The header content rendered above the dock list.
  /// Rendered inside [AnimatedOpacity] -- do NOT wrap it yourself.
  final Widget header;

  /// Number of items forwarded to [DockList.itemCount].
  final int itemCount;

  /// Builder forwarded to [DockList.itemBuilder].
  final IndexedWidgetBuilder itemBuilder;

  /// Optional insets applied to the [DockList] via [Positioned].
  ///
  /// Use this to counteract parent padding (e.g.
  /// `EdgeInsets.symmetric(horizontal: -16)` to bleed the dock list into
  /// the parent's 16 px padding).
  final EdgeInsets dockListInsets;

  /// Height of the header region above the dock list.
  static const double headerHeight = 28;

  /// Default container height of [DockList] (must match its default).
  static const double dockListHeight = 230;

  const DockRow({
    super.key,
    required this.header,
    required this.itemCount,
    required this.itemBuilder,
    this.dockListInsets = EdgeInsets.zero,
  });

  @override
  State<DockRow> createState() => _DockRowState();
}

class _DockRowState extends State<DockRow> {
  bool _dockActive = false;

  @override
  Widget build(BuildContext context) {
    // Total height = header region + DockList's default container height.
    const totalHeight = DockRow.headerHeight + DockRow.dockListHeight;

    return SizedBox(
      height: totalHeight,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          // Header -- painted first so magnified cards can overlap it.
          Positioned(
            top: 0,
            left: 0,
            right: 0,
            child: AnimatedOpacity(
              opacity: _dockActive ? 1.0 : 0.6,
              duration: const Duration(milliseconds: 300),
              child: widget.header,
            ),
          ),

          // Dock list -- painted after header so magnified cards sit on top.
          Positioned(
            top: DockRow.headerHeight,
            left: widget.dockListInsets.left,
            right: widget.dockListInsets.right,
            child: DockList(
              itemCount: widget.itemCount,
              onActiveChanged: (active) =>
                  setState(() => _dockActive = active),
              itemBuilder: widget.itemBuilder,
            ),
          ),
        ],
      ),
    );
  }
}
