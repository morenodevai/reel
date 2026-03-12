import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/api/review_api.dart' as review_api;
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/library_provider.dart';

/// Undo all pending (unlocked) transactions, refresh the library,
/// and report the result via toast. Returns true on success.
Future<bool> clearAllPendingAndReport(WidgetRef ref) async {
  final toast = ref.read(toastProvider.notifier);
  try {
    final result = await review_api.clearAllPending();
    ref.read(libraryProvider.notifier).refresh();
    final msg = result.failed > 0
        ? 'Cleared ${result.succeeded} items (${result.failed} failed to undo)'
        : 'Cleared ${result.succeeded} items -- files moved back';
    toast.show(msg, type: result.failed > 0 ? ToastType.error : ToastType.success);
    return true;
  } catch (e) {
    toast.show('Clear failed: $e', type: ToastType.error);
    return false;
  }
}
