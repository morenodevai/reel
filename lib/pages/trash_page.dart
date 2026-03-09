import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/companion.dart';
import 'package:reel/src/rust/api/trash_api.dart' as trash_api;
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/trash_provider.dart';
import 'package:reel/components/empty_state.dart';
import 'package:reel/components/loading_skeleton.dart';
import 'package:reel/theme/app_theme.dart';
import 'package:reel/utils/time_format.dart';

class TrashPageWidget extends ConsumerStatefulWidget {
  const TrashPageWidget({super.key});

  @override
  ConsumerState<TrashPageWidget> createState() => _TrashPageWidgetState();
}

class _TrashPageWidgetState extends ConsumerState<TrashPageWidget> {
  List<TrashItem> _items = [];
  bool _loading = true;
  String? _error;
  String? _busyId;
  bool _emptyingAll = false;

  @override
  void initState() {
    super.initState();
    _loadItems();
  }

  Future<void> _loadItems() async {
    setState(() { _loading = true; _error = null; });
    try {
      final items = await trash_api.getTrashItems();
      if (mounted) setState(() { _items = items; _loading = false; });
    } catch (e) {
      if (mounted) setState(() { _loading = false; _error = '$e'; });
    }
  }

  Future<void> _handleRestore(String id) async {
    setState(() => _busyId = id);
    final toast = ref.read(toastProvider.notifier);
    try {
      final path = await trash_api.restoreTrashItem(id: id);
      if (mounted) {
        setState(() => _items.removeWhere((t) => t.id == id));
        ref.invalidate(trashCountProvider);
      }
      toast.show('Restored to $path');
    } catch (e) {
      toast.show('Restore failed: $e', type: ToastType.error);
    } finally {
      if (mounted) setState(() => _busyId = null);
    }
  }

  Future<void> _handleDelete(String id) async {
    setState(() => _busyId = id);
    final toast = ref.read(toastProvider.notifier);
    try {
      await trash_api.deleteTrashItem(id: id);
      if (mounted) {
        setState(() => _items.removeWhere((t) => t.id == id));
        ref.invalidate(trashCountProvider);
      }
      toast.show('Permanently deleted');
    } catch (e) {
      toast.show('Delete failed: $e', type: ToastType.error);
    } finally {
      if (mounted) setState(() => _busyId = null);
    }
  }

  Future<void> _handleEmptyAll() async {
    if (_emptyingAll) return;
    setState(() => _emptyingAll = true);
    final toast = ref.read(toastProvider.notifier);
    try {
      final count = await trash_api.emptyTrash();
      if (mounted) {
        setState(() => _items.clear());
        ref.invalidate(trashCountProvider);
      }
      toast.show('Permanently deleted $count item(s)');
    } catch (e) {
      toast.show('Empty trash failed: $e', type: ToastType.error);
    } finally {
      if (mounted) setState(() => _emptyingAll = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading && _items.isEmpty) {
      return _TrashLoadingSkeleton();
    }

    if (_error != null && _items.isEmpty) {
      return EmptyState(
        icon: Icons.error_outline,
        title: 'Failed to load trash',
        subtitle: _error,
      );
    }

    if (_items.isEmpty) {
      return const EmptyState(
        icon: Icons.delete_outline,
        title: 'Trash is empty',
        subtitle: 'Companion files from organized folders will appear here',
      );
    }

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                '${_items.length} item${_items.length != 1 ? 's' : ''}',
                style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
              ),
              _ActionButton(
                icon: Icons.delete_forever,
                label: _emptyingAll ? 'Emptying...' : 'Empty All',
                color: AppColors.error,
                onTap: _emptyingAll ? null : _handleEmptyAll,
              ),
            ],
          ),
        ),
        Expanded(
          child: ListView.builder(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            itemCount: _items.length,
            itemBuilder: (context, index) {
              final item = _items[index];
              return _TrashItemRow(
                item: item,
                busy: _busyId == item.id,
                onRestore: () => _handleRestore(item.id),
                onDelete: () => _handleDelete(item.id),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _TrashItemRow extends StatelessWidget {
  final TrashItem item;
  final bool busy;
  final VoidCallback onRestore;
  final VoidCallback onDelete;

  const _TrashItemRow({
    required this.item,
    required this.busy,
    required this.onRestore,
    required this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    return AnimatedOpacity(
      duration: const Duration(milliseconds: 150),
      opacity: busy ? 0.5 : 1.0,
      child: Container(
        margin: const EdgeInsets.only(bottom: 8),
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: AppColors.surface.withValues(alpha: 0.5),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          children: [
            const Icon(Icons.image_outlined, size: 32, color: AppColors.textQuaternary),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    item.filename,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w500,
                      color: AppColors.textPrimary,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    item.originalPath,
                    style: const TextStyle(fontSize: 11, color: AppColors.textQuaternary),
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    timeAgo(item.timestamp),
                    style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                  ),
                ],
              ),
            ),
            _RowButton(
              icon: Icons.restore,
              label: 'Restore',
              onTap: busy ? null : onRestore,
            ),
            const SizedBox(width: 4),
            _RowButton(
              icon: Icons.close,
              label: 'Delete',
              color: AppColors.error,
              onTap: busy ? null : onDelete,
            ),
          ],
        ),
      ),
    );
  }
}

class _RowButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final Color color;
  final VoidCallback? onTap;
  const _RowButton({required this.icon, required this.label, this.color = AppColors.textSecondary, this.onTap});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: onTap != null ? SystemMouseCursors.click : SystemMouseCursors.basic,
      child: GestureDetector(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 14, color: color),
              const SizedBox(width: 4),
              Text(label, style: TextStyle(fontSize: 12, color: color)),
            ],
          ),
        ),
      ),
    );
  }
}

class _ActionButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final Color color;
  final VoidCallback? onTap;
  const _ActionButton({required this.icon, required this.label, required this.color, this.onTap});

  @override
  Widget build(BuildContext context) {
    final effectiveColor = onTap != null ? color : color.withValues(alpha: 0.4);
    return MouseRegion(
      cursor: onTap != null ? SystemMouseCursors.click : SystemMouseCursors.basic,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
          decoration: BoxDecoration(
            color: effectiveColor.withValues(alpha: 0.1),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 14, color: effectiveColor),
              const SizedBox(width: 6),
              Text(label, style: TextStyle(fontSize: 12, fontWeight: FontWeight.w500, color: effectiveColor)),
            ],
          ),
        ),
      ),
    );
  }
}

class _TrashLoadingSkeleton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: List.generate(
        4,
        (i) => Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: SkeletonBox(height: 72, borderRadius: 8),
        ),
      ),
    );
  }
}
