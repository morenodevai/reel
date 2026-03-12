import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/api/library_api.dart' as library_api;
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/playback_state.dart';

/// Load media details, resolve the play target, and navigate to the player.
///
/// Pass [isMounted] from StatefulWidget callers to guard async gaps.
/// Omit for StatelessWidget callers.
Future<void> playMedia(
  MediaInfo media,
  WidgetRef ref, {
  bool Function()? isMounted,
}) async {
  try {
    final detail = await library_api.getMediaDetails(mediaPath: media.path);
    if (isMounted != null && !isMounted()) return;
    final target = await resolvePlayTarget(detail);
    if (target == null) return;
    if (isMounted != null && !isMounted()) return;
    ref
        .read(navigationProvider.notifier)
        .goToPlayer(detail, target.file, target.playlist, target.index);
  } catch (e) {
    if (isMounted == null || isMounted()) {
      ref
          .read(toastProvider.notifier)
          .show('Could not load media: $e', type: ToastType.error);
    }
  }
}
