import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:reel/src/rust/transaction.dart';
import 'package:reel/src/rust/api/review_api.dart' as review_api;
import 'package:reel/src/rust/api/history_api.dart' as history_api;
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/library_provider.dart';
import 'package:reel/components/loading_skeleton.dart';
import 'package:reel/theme/app_theme.dart';

class ReviewPageWidget extends ConsumerStatefulWidget {
  final List<String> batchIds;
  const ReviewPageWidget({super.key, required this.batchIds});

  @override
  ConsumerState<ReviewPageWidget> createState() => _ReviewPageWidgetState();
}

class _ReviewPageWidgetState extends ConsumerState<ReviewPageWidget> {
  List<Transaction> _items = [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final data =
          await review_api.getReviewItems(batchIds: widget.batchIds);
      if (mounted) setState(() { _items = data; _loading = false; });
    } catch (e) {
      debugPrint('[review] Load failed: $e');
      if (mounted) {
        setState(() { _items = []; _loading = false; });
        ref.read(toastProvider.notifier).show('Failed to load review items: $e', type: ToastType.error);
      }
    }
  }

  Future<void> _approveAll() async {
    final toast = ref.read(toastProvider.notifier);
    try {
      final ids = _items.map((t) => t.id).toList();
      await review_api.lockTransactions(ids: ids);
      setState(() => _items = []);
      toast.show('All items approved');
    } catch (e) {
      toast.show('Failed to approve all: $e', type: ToastType.error);
    }
  }

  Future<void> _clearAll() async {
    final toast = ref.read(toastProvider.notifier);
    try {
      final result = await review_api.clearAllPending();
      setState(() => _items = []);
      ref.read(libraryProvider.notifier).refresh();
      final msg = result.failed > 0
          ? 'Cleared ${result.succeeded} items (${result.failed} failed to undo)'
          : 'Cleared ${result.succeeded} items -- files moved back';
      toast.show(msg,
          type: result.failed > 0 ? ToastType.error : ToastType.success);
    } catch (e) {
      toast.show('Clear failed: $e', type: ToastType.error);
    }
  }

  Future<void> _approveGroup(List<String> ids) async {
    final toast = ref.read(toastProvider.notifier);
    try {
      await review_api.lockTransactions(ids: ids);
      setState(() => _items.removeWhere((t) => ids.contains(t.id)));
    } catch (e) {
      toast.show('Failed to approve: $e', type: ToastType.error);
    }
  }

  Future<void> _undoGroup(List<String> ids) async {
    final toast = ref.read(toastProvider.notifier);
    try {
      int succeeded = 0;
      int failed = 0;
      for (final id in ids) {
        final result = await history_api.undoTransaction(id: id);
        if (result.success) {
          succeeded++;
        } else {
          failed++;
        }
      }
      setState(() => _items.removeWhere((t) => ids.contains(t.id)));
      ref.read(libraryProvider.notifier).refresh();
      if (ids.length == 1) {
        toast.show('File moved back to original location');
      } else {
        final msg = failed > 0
            ? 'Undone $succeeded of ${ids.length} ($failed failed)'
            : 'Undone $succeeded items';
        toast.show(msg, type: failed > 0 ? ToastType.error : ToastType.success);
      }
    } catch (e) {
      toast.show('Undo failed: $e', type: ToastType.error);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return _ReviewLoadingSkeleton();
    }

    if (_items.isEmpty) {
      return const Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.check_circle_outline, size: 40, color: AppColors.success),
            SizedBox(height: 12),
            Text(
              'All items reviewed',
              style: TextStyle(fontSize: 14, color: AppColors.textSecondary),
            ),
          ],
        ),
      );
    }

    // Group by title
    final groups = _groupByTitle(_items);

    return Column(
      children: [
        // Top bar
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                '${groups.length} title${groups.length != 1 ? 's' : ''} (${_items.length} file${_items.length != 1 ? 's' : ''})',
                style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
              ),
              Row(
                children: [
                  _ActionButton(
                    icon: Icons.delete_outline,
                    label: 'Undo All',
                    color: AppColors.error,
                    onTap: _clearAll,
                  ),
                  const SizedBox(width: 8),
                  _ActionButton(
                    icon: Icons.check_circle_outline,
                    label: 'Approve All',
                    color: AppColors.success,
                    onTap: _approveAll,
                  ),
                ],
              ),
            ],
          ),
        ),

        // Review cards
        Expanded(
          child: ListView.separated(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            itemCount: groups.length,
            separatorBuilder: (_, __) => const SizedBox(height: 8),
            itemBuilder: (context, index) {
              final group = groups[index];
              return _ReviewGroupCard(
                group: group,
                onApprove: () =>
                    _approveGroup(group.transactions.map((t) => t.id).toList()),
                onUndo: () =>
                    _undoGroup(group.transactions.map((t) => t.id).toList()),
              );
            },
          ),
        ),
      ],
    );
  }

  List<_TitleGroup> _groupByTitle(List<Transaction> items) {
    final map = <String, _TitleGroup>{};
    for (final item in items) {
      final key = item.tmdbId != null
          ? 'tmdb-${item.tmdbId}'
          : '${item.title}-${item.year ?? ""}';
      map.putIfAbsent(
        key,
        () => _TitleGroup(
          title: item.title,
          year: item.year,
          format: item.format,
          genre: item.genre,
          posterUrl: item.posterUrl,
          confidence: item.confidence,
          transactions: [],
        ),
      );
      map[key]!.transactions.add(item);
      if (item.confidence < map[key]!.confidence) {
        map[key]!.confidence = item.confidence;
      }
      if (map[key]!.posterUrl == null && item.posterUrl != null) {
        map[key]!.posterUrl = item.posterUrl;
      }
    }
    return map.values.toList();
  }
}

class _TitleGroup {
  final String title;
  final int? year;
  final String format;
  final String genre;
  String? posterUrl;
  double confidence;
  final List<Transaction> transactions;

  _TitleGroup({
    required this.title,
    this.year,
    required this.format,
    required this.genre,
    this.posterUrl,
    required this.confidence,
    required this.transactions,
  });
}

class _ReviewGroupCard extends StatelessWidget {
  final _TitleGroup group;
  final VoidCallback onApprove;
  final VoidCallback onUndo;

  const _ReviewGroupCard({
    required this.group,
    required this.onApprove,
    required this.onUndo,
  });

  @override
  Widget build(BuildContext context) {
    final confidenceColor = group.confidence >= 0.8
        ? AppColors.success
        : group.confidence >= 0.5
            ? AppColors.warning
            : AppColors.error;

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppColors.surface.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: AppColors.border),
      ),
      child: Row(
        children: [
          // Poster
          ClipRRect(
            borderRadius: BorderRadius.circular(6),
            child: SizedBox(
              width: 48,
              height: 72,
              child: group.posterUrl != null
                  ? CachedNetworkImage(
                      imageUrl: group.posterUrl!,
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
                  '${group.title}${group.year != null ? ' (${group.year})' : ''}',
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 2),
                Text(
                  '${group.format} / ${group.genre}',
                  style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
                ),
                const SizedBox(height: 2),
                Row(
                  children: [
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                      decoration: BoxDecoration(
                        color: confidenceColor.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        '${(group.confidence * 100).toInt()}%',
                        style: TextStyle(
                          fontSize: 10,
                          fontWeight: FontWeight.w600,
                          color: confidenceColor,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Text(
                      '${group.transactions.length} file${group.transactions.length != 1 ? 's' : ''}',
                      style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                    ),
                  ],
                ),
              ],
            ),
          ),

          // Action buttons
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _SmallButton(
                icon: Icons.undo,
                onTap: onUndo,
                color: AppColors.textSecondary,
              ),
              const SizedBox(width: 4),
              _SmallButton(
                icon: Icons.check,
                onTap: onApprove,
                color: AppColors.success,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _SmallButton extends StatelessWidget {
  final IconData icon;
  final VoidCallback onTap;
  final Color color;
  const _SmallButton({required this.icon, required this.onTap, required this.color});

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.all(6),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
          ),
          child: Icon(icon, size: 18, color: color),
        ),
      ),
    );
  }
}

class _ActionButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final Color color;
  final VoidCallback onTap;
  const _ActionButton({
    required this.icon,
    required this.label,
    required this.color,
    required this.onTap,
  });

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
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                  color: color,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ReviewLoadingSkeleton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: List.generate(
        3,
        (i) => Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: SkeletonBox(height: 85, borderRadius: 12),
        ),
      ),
    );
  }
}
