import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:media_kit_video/media_kit_video.dart';
import 'package:reel/src/rust/pipeline.dart';
import 'package:reel/providers/playback_provider.dart';
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/pages/player_controls.dart';
import 'package:reel/theme/app_theme.dart';

class PlayerPageWidget extends ConsumerStatefulWidget {
  final MediaDetail detail;
  final MediaFile file;
  final List<MediaFile> playlist;
  final int startIndex;

  const PlayerPageWidget({
    super.key,
    required this.detail,
    required this.file,
    required this.playlist,
    required this.startIndex,
  });

  @override
  ConsumerState<PlayerPageWidget> createState() => _PlayerPageWidgetState();
}

class _PlayerPageWidgetState extends ConsumerState<PlayerPageWidget> {
  late final VideoController _videoController;
  final FocusNode _focusNode = FocusNode();
  bool _initialized = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final notifier = ref.read(playbackProvider.notifier);
      _videoController = VideoController(notifier.player);
      notifier.open(widget.detail, widget.file, widget.playlist, widget.startIndex);
      setState(() => _initialized = true);
      _focusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  bool _handleKey(KeyEvent event) {
    if (event is! KeyDownEvent) return false;
    final notifier = ref.read(playbackProvider.notifier);
    final s = ref.read(playbackProvider);
    final key = event.logicalKey;
    final shift = HardwareKeyboard.instance.isShiftPressed;

    if (key == LogicalKeyboardKey.space || key == LogicalKeyboardKey.keyK) {
      notifier.togglePlay();
    } else if (key == LogicalKeyboardKey.arrowLeft) {
      notifier.seekRelative(shift ? -30 : -10);
    } else if (key == LogicalKeyboardKey.arrowRight) {
      notifier.seekRelative(shift ? 30 : 10);
    } else if (key == LogicalKeyboardKey.arrowUp) {
      notifier.adjustVolume(5);
    } else if (key == LogicalKeyboardKey.arrowDown) {
      notifier.adjustVolume(-5);
    } else if (key == LogicalKeyboardKey.keyF) {
      notifier.toggleFullscreen();
    } else if (key == LogicalKeyboardKey.keyM) {
      notifier.toggleMute();
    } else if (key == LogicalKeyboardKey.escape) {
      if (s.fullscreen) {
        notifier.toggleFullscreen();
      } else {
        _exitPlayer();
      }
    } else if (key == LogicalKeyboardKey.keyN && s.isTV) {
      notifier.playNext();
    } else if (key == LogicalKeyboardKey.keyP && s.isTV) {
      notifier.playPrevious();
    } else {
      return false;
    }
    notifier.onMouseActivity();
    return true;
  }

  void _exitPlayer() {
    ref.read(playbackProvider.notifier).onLeavePlayer();
    ref.read(navigationProvider.notifier).pop();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(playbackProvider);

    return Focus(
      focusNode: _focusNode,
      autofocus: true,
      onKeyEvent: (node, event) {
        return _handleKey(event)
            ? KeyEventResult.handled
            : KeyEventResult.ignored;
      },
      child: Container(
        color: Colors.black,
        child: Stack(
          children: [
            // Video layer
            if (_initialized)
              Positioned.fill(
                child: Video(
                  controller: _videoController,
                  controls: NoVideoControls,
                ),
              )
            else
              const Center(
                child: CircularProgressIndicator(color: AppColors.primary),
              ),

            // Tap area — click = play/pause, double-click = fullscreen
            Positioned.fill(
              child: MouseRegion(
                cursor: state.controlsVisible
                    ? SystemMouseCursors.basic
                    : SystemMouseCursors.none,
                onHover: (_) => ref.read(playbackProvider.notifier).onMouseActivity(),
                child: GestureDetector(
                  behavior: HitTestBehavior.translucent,
                  onTap: () => ref.read(playbackProvider.notifier).togglePlay(),
                  onDoubleTap: () => ref.read(playbackProvider.notifier).toggleFullscreen(),
                ),
              ),
            ),

            // Loading spinner
            if (state.loading && _initialized)
              const Center(
                child: CircularProgressIndicator(color: Colors.white54, strokeWidth: 2),
              ),

            // Error
            if (state.error != null)
              Center(
                child: Container(
                  padding: const EdgeInsets.all(24),
                  decoration: BoxDecoration(
                    color: Colors.black87,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Text(
                    state.error!,
                    style: const TextStyle(color: AppColors.error, fontSize: 14),
                  ),
                ),
              ),

            // Controls overlay (fades in/out)
            ControlsOverlay(state: state, onExit: _exitPlayer),

            // Auto-play next episode
            if (state.autoPlayCountdown > 0)
              AutoPlayOverlay(state: state),
          ],
        ),
      ),
    );
  }
}
