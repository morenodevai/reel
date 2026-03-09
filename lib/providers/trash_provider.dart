import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/api/trash_api.dart' as trash_api;

/// Trash count provider — fetches the count from the DB.
/// Invalidate after trash operations to refresh the badge.
final trashCountProvider = FutureProvider<int>((ref) async {
  return trash_api.getTrashCount();
});
