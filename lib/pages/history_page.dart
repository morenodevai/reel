import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/db/transactions.dart';
import 'package:reel/src/rust/api/history_api.dart' as history_api;
import 'package:reel/src/rust/api/review_api.dart' as review_api;
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/library_provider.dart';
import 'package:reel/components/empty_state.dart';
import 'package:reel/components/loading_skeleton.dart';
import 'package:reel/theme/app_theme.dart';
import 'package:reel/utils/time_format.dart';

class HistoryPageWidget extends ConsumerStatefulWidget {
  const HistoryPageWidget({super.key});

  @override
  ConsumerState<HistoryPageWidget> createState() => _HistoryPageWidgetState();
}

class _HistoryPageWidgetState extends ConsumerState<HistoryPageWidget> {
  final List<Transaction> _items = [];
  bool _hasMore = true;
  bool _loading = true;
  bool _loadingMore = false;
  String? _undoingId;
  final ScrollController _scrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    _loadPage(0);
    _scrollController.addListener(_onScroll);
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (_scrollController.position.pixels >=
            _scrollController.position.maxScrollExtent - 300 &&
        _hasMore &&
        !_loadingMore) {
      _loadPage(_items.length);
    }
  }

  Future<void> _loadPage(int offset) async {
    if (_loadingMore) return;
    setState(() {
      if (offset == 0) _loading = true;
      _loadingMore = true;
    });

    try {
      final page = await history_api.getHistory(offset: offset, limit: 30);
      if (mounted) {
        setState(() {
          if (offset == 0) _items.clear();
          _items.addAll(page.items);
          _hasMore = page.hasMore;
          _loading = false;
          _loadingMore = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _loadingMore = false;
          _hasMore = false;
        });
      }
    }
  }

  Future<void> _handleApproveAll() async {
    final toast = ref.read(toastProvider.notifier);
    try {
      final ids = _items.map((t) => t.id).toList();
      await review_api.lockTransactions(ids: ids);
      setState(() => _items.clear());
      toast.show('All items approved');
    } catch (e) {
      toast.show('Approve failed: $e', type: ToastType.error);
    }
  }

  Future<void> _handleClearAll() async {
    final toast = ref.read(toastProvider.notifier);
    try {
      final result = await review_api.clearAllPending();
      setState(() => _items.clear());
      ref.read(libraryProvider.notifier).refresh();
      final msg = result.failed > 0
          ? 'Cleared ${result.succeeded} items (${result.failed} failed to undo)'
          : 'Cleared ${result.succeeded} items -- files moved back';
      toast.show(msg, type: result.failed > 0 ? ToastType.error : ToastType.success);
    } catch (e) {
      toast.show('Clear failed: $e', type: ToastType.error);
    }
  }

  Future<void> _handleUndo(String id) async {
    setState(() => _undoingId = id);
    final toast = ref.read(toastProvider.notifier);
    try {
      final result = await history_api.undoTransaction(id: id);
      if (result.success) {
        setState(() => _items.removeWhere((t) => t.id == id));
        toast.show('File moved back to original location');
        ref.read(libraryProvider.notifier).refresh();
      } else {
        toast.show(result.message, type: ToastType.error);
      }
    } catch (e) {
      toast.show('Undo failed: $e', type: ToastType.error);
    } finally {
      if (mounted) setState(() => _undoingId = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading && _items.isEmpty) {
      return _HistoryLoadingSkeleton();
    }

    if (_items.isEmpty) {
      return const EmptyState(
        icon: Icons.history,
        title: 'No history yet',
        subtitle: 'Organize some files to see them here',
      );
    }

    return Column(
      children: [
        // Top bar with bulk actions
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                '${_items.length} item${_items.length != 1 ? 's' : ''}',
                style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
              ),
              Row(
                children: [
                  _BulkButton(
                    icon: Icons.delete_outline,
                    label: 'Clear All',
                    color: AppColors.error,
                    onTap: _handleClearAll,
                  ),
                  const SizedBox(width: 8),
                  _BulkButton(
                    icon: Icons.check_circle_outline,
                    label: 'Approve All',
                    color: AppColors.success,
                    onTap: _handleApproveAll,
                  ),
                ],
              ),
            ],
          ),
        ),

        // List
        Expanded(
          child: ListView.builder(
            controller: _scrollController,
            padding: const EdgeInsets.symmetric(horizontal: 16),
            itemCount: _items.length + (_hasMore ? 1 : 0),
            itemBuilder: (context, index) {
              if (index == _items.length) {
                return const Padding(
                  padding: EdgeInsets.all(24),
                  child: Center(
                    child: SizedBox(
                      width: 24,
                      height: 24,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ),
                );
              }

              final txn = _items[index];
              return _HistoryItemRow(
                transaction: txn,
                undoing: _undoingId == txn.id,
                onUndo: () => _handleUndo(txn.id),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _HistoryItemRow extends StatelessWidget {
  final Transaction transaction;
  final bool undoing;
  final VoidCallback onUndo;

  const _HistoryItemRow({
    required this.transaction,
    required this.undoing,
    required this.onUndo,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppColors.surface.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          // Poster thumbnail
          ClipRRect(
            borderRadius: BorderRadius.circular(4),
            child: SizedBox(
              width: 48,
              height: 72,
              child: transaction.posterUrl != null
                  ? CachedNetworkImage(
                      imageUrl: transaction.posterUrl!,
                      fit: BoxFit.cover,
                      placeholder: (_, __) =>
                          Container(color: AppColors.surfaceElevated),
                      errorWidget: (_, __, ___) =>
                          Container(color: AppColors.surfaceElevated),
                    )
                  : Container(color: AppColors.surfaceElevated),
            ),
          ),
          const SizedBox(width: 12),

          // Info
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${transaction.title}${transaction.year != null ? ' (${transaction.year})' : ''}',
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 2),
                Text(
                  '${transaction.format} / ${transaction.genre}',
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textTertiary,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  timeAgo(transaction.timestamp),
                  style: const TextStyle(
                    fontSize: 10,
                    color: AppColors.textQuaternary,
                  ),
                ),
              ],
            ),
          ),

          // Undo button
          MouseRegion(
            cursor: SystemMouseCursors.click,
            child: GestureDetector(
              onTap: undoing ? null : onUndo,
              child: AnimatedOpacity(
                duration: const Duration(milliseconds: 150),
                opacity: undoing ? 0.5 : 1.0,
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: const Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(Icons.undo, size: 14, color: AppColors.textSecondary),
                      SizedBox(width: 6),
                      Text(
                        'Undo',
                        style: TextStyle(
                          fontSize: 12,
                          color: AppColors.textSecondary,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

}

class _BulkButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final Color color;
  final VoidCallback onTap;
  const _BulkButton({required this.icon, required this.label, required this.color, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
          decoration: BoxDecoration(
            color: color.withValues(alpha: 0.1),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 14, color: color),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w500, color: color),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _HistoryLoadingSkeleton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: List.generate(
        5,
        (i) => Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: SkeletonBox(height: 80, borderRadius: 8),
        ),
      ),
    );
  }
}
