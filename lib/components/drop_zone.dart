import 'package:flutter/material.dart';
import 'package:desktop_drop/desktop_drop.dart';
import 'package:reel/theme/app_theme.dart';

/// Drop zone for dragging media files/folders from Finder.
/// Uses desktop_drop for native macOS drag-and-drop support.
class DropZone extends StatefulWidget {
  final void Function(List<String> paths) onDrop;
  final VoidCallback onBrowse;
  final bool compact;

  const DropZone({
    super.key,
    required this.onDrop,
    required this.onBrowse,
    this.compact = false,
  });

  @override
  State<DropZone> createState() => _DropZoneState();
}

class _DropZoneState extends State<DropZone> {
  bool _dragOver = false;

  @override
  Widget build(BuildContext context) {
    return DropTarget(
      onDragDone: (details) {
        setState(() => _dragOver = false);
        final paths = details.files.map((f) => f.path).toList();
        if (paths.isNotEmpty) {
          widget.onDrop(paths);
        }
      },
      onDragEntered: (_) => setState(() => _dragOver = true),
      onDragExited: (_) => setState(() => _dragOver = false),
      child: widget.compact
          ? _CompactDropZone(dragOver: _dragOver, onBrowse: widget.onBrowse)
          : _FullDropZone(dragOver: _dragOver, onBrowse: widget.onBrowse),
    );
  }
}

class _CompactDropZone extends StatelessWidget {
  final bool dragOver;
  final VoidCallback onBrowse;
  const _CompactDropZone({required this.dragOver, required this.onBrowse});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onBrowse,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          width: double.infinity,
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
              color: dragOver ? AppColors.primary : AppColors.border,
              width: 2,
              strokeAlign: BorderSide.strokeAlignInside,
            ),
            color: dragOver ? AppColors.primary.withValues(alpha: 0.1) : Colors.transparent,
          ),
          child: Text(
            dragOver ? 'Release to organize' : 'Drop media here or click to browse',
            textAlign: TextAlign.center,
            style: const TextStyle(
              fontSize: 12,
              color: AppColors.textTertiary,
            ),
          ),
        ),
      ),
    );
  }
}

class _FullDropZone extends StatelessWidget {
  final bool dragOver;
  final VoidCallback onBrowse;
  const _FullDropZone({required this.dragOver, required this.onBrowse});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onBrowse,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          width: double.infinity,
          padding: const EdgeInsets.symmetric(vertical: 32),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(16),
            border: Border.all(
              color: dragOver ? AppColors.primary : AppColors.border,
              width: 2,
              strokeAlign: BorderSide.strokeAlignInside,
            ),
            color: dragOver ? AppColors.primary.withValues(alpha: 0.1) : Colors.transparent,
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.folder_open_outlined,
                size: 40,
                color: dragOver ? AppColors.primary : AppColors.textQuaternary,
              ),
              const SizedBox(height: 12),
              Text(
                dragOver ? 'Release to organize' : 'Drop media files here',
                style: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                  color: AppColors.textSecondary,
                ),
              ),
              const SizedBox(height: 4),
              const Text(
                'or click to browse',
                style: TextStyle(
                  fontSize: 12,
                  color: AppColors.textTertiary,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
