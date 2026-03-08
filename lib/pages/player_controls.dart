import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/providers/playback_provider.dart';
import 'package:reel/theme/app_theme.dart';

/// Format a Duration as HH:MM:SS or MM:SS.
String formatDuration(Duration d) {
  final h = d.inHours;
  final m = d.inMinutes.remainder(60);
  final s = d.inSeconds.remainder(60);
  if (h > 0) return '${h.toString().padLeft(2, '0')}:${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
  return '${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
}

// ---------------------------------------------------------------------------
// Icon Button — minimal, no tooltip noise
// ---------------------------------------------------------------------------

class PlayerIconBtn extends StatefulWidget {
  final IconData icon;
  final double size;
  final VoidCallback? onTap;

  const PlayerIconBtn({super.key, required this.icon, this.size = 24, this.onTap});

  @override
  State<PlayerIconBtn> createState() => _PlayerIconBtnState();
}

class _PlayerIconBtnState extends State<PlayerIconBtn> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 100),
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: _hovered ? Colors.white.withValues(alpha: 0.15) : Colors.transparent,
          ),
          child: Icon(
            widget.icon,
            size: widget.size,
            color: Colors.white,
            shadows: const [Shadow(blurRadius: 8, color: Colors.black)],
          ),
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Controls Overlay — shows/hides with animation
// ---------------------------------------------------------------------------

class ControlsOverlay extends ConsumerWidget {
  final PlaybackState state;
  final VoidCallback onExit;

  const ControlsOverlay({super.key, required this.state, required this.onExit});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return IgnorePointer(
      ignoring: !state.controlsVisible,
      child: AnimatedOpacity(
        opacity: state.controlsVisible ? 1.0 : 0.0,
        duration: const Duration(milliseconds: 200),
        child: Stack(
          children: [
            // Top gradient
            Positioned(
              top: 0, left: 0, right: 0, height: 120,
              child: Container(
                decoration: const BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [Color(0xCC000000), Colors.transparent],
                  ),
                ),
              ),
            ),

            // Bottom gradient
            Positioned(
              bottom: 0, left: 0, right: 0, height: 180,
              child: Container(
                decoration: const BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.bottomCenter,
                    end: Alignment.topCenter,
                    colors: [Color(0xCC000000), Colors.transparent],
                  ),
                ),
              ),
            ),

            // Top bar
            Positioned(
              top: 16, left: 20, right: 20,
              child: PlayerTopBar(state: state, onExit: onExit),
            ),

            // Center play/pause (only shown when paused)
            if (!state.playing)
              const Center(child: _CenterPlayIcon()),

            // Bottom bar: progress + buttons
            Positioned(
              bottom: 0, left: 0, right: 0,
              child: PlayerBottomBar(state: state),
            ),
          ],
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Top Bar
// ---------------------------------------------------------------------------

class PlayerTopBar extends StatelessWidget {
  final PlaybackState state;
  final VoidCallback onExit;

  const PlayerTopBar({super.key, required this.state, required this.onExit});

  @override
  Widget build(BuildContext context) {
    final detail = state.mediaDetail;
    final file = state.currentFile;
    String title = detail?.title ?? '';
    if (state.isTV && file != null && file.season != null && file.episode != null) {
      final ep = 'S${file.season.toString().padLeft(2, '0')}E${file.episode.toString().padLeft(2, '0')}';
      final epTitle = file.episodeTitle != null ? ' - ${file.episodeTitle}' : '';
      title = '$title  $ep$epTitle';
    }

    return Row(
      children: [
        PlayerIconBtn(icon: Icons.arrow_back_rounded, onTap: onExit),
        const SizedBox(width: 12),
        Expanded(
          child: Text(
            title,
            style: const TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w600,
              color: Colors.white,
              shadows: [Shadow(blurRadius: 12, color: Colors.black)],
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );
  }
}

// ---------------------------------------------------------------------------
// Center Play Icon — only visible when paused
// ---------------------------------------------------------------------------

class _CenterPlayIcon extends StatelessWidget {
  const _CenterPlayIcon();

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        shape: BoxShape.circle,
        color: Colors.black38,
      ),
      padding: const EdgeInsets.all(16),
      child: Icon(
        Icons.play_arrow_rounded,
        size: 56,
        color: Colors.white.withValues(alpha: 0.9),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Bottom Bar
// ---------------------------------------------------------------------------

class PlayerBottomBar extends ConsumerWidget {
  final PlaybackState state;
  const PlayerBottomBar({super.key, required this.state});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 0, 20, 16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Progress bar
          PlayerProgressBar(state: state),
          const SizedBox(height: 8),
          // Controls row
          Row(
            children: [
              // Prev / Play / Next
              if (state.hasPrevious)
                PlayerIconBtn(
                  icon: Icons.skip_previous_rounded,
                  size: 22,
                  onTap: () => ref.read(playbackProvider.notifier).playPrevious(),
                ),
              PlayerIconBtn(
                icon: state.playing ? Icons.pause_rounded : Icons.play_arrow_rounded,
                size: 28,
                onTap: () => ref.read(playbackProvider.notifier).togglePlay(),
              ),
              if (state.hasNext)
                PlayerIconBtn(
                  icon: Icons.skip_next_rounded,
                  size: 22,
                  onTap: () => ref.read(playbackProvider.notifier).playNext(),
                ),
              const SizedBox(width: 8),
              // Volume
              _VolumeControl(state: state),
              const SizedBox(width: 12),
              // Time display
              Text(
                '${formatDuration(state.position)} / ${formatDuration(state.duration)}',
                style: const TextStyle(fontSize: 12, color: Colors.white70),
              ),
              const Spacer(),
              // Subtitle picker
              SubtitlePicker(state: state),
              const SizedBox(width: 2),
              // Audio picker
              AudioPicker(state: state),
              const SizedBox(width: 2),
              // Fullscreen
              PlayerIconBtn(
                icon: state.fullscreen ? Icons.fullscreen_exit_rounded : Icons.fullscreen_rounded,
                size: 22,
                onTap: () => ref.read(playbackProvider.notifier).toggleFullscreen(),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Progress Bar — big hit area, smooth drag
// ---------------------------------------------------------------------------

class PlayerProgressBar extends ConsumerStatefulWidget {
  final PlaybackState state;
  const PlayerProgressBar({super.key, required this.state});

  @override
  ConsumerState<PlayerProgressBar> createState() => _PlayerProgressBarState();
}

class _PlayerProgressBarState extends ConsumerState<PlayerProgressBar> {
  bool _dragging = false;
  double _dragProgress = 0.0;
  bool _hovering = false;
  double _hoverProgress = 0.0;

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final progress = _dragging ? _dragProgress : state.progress;
    final bufferProgress = state.duration.inMilliseconds > 0
        ? state.buffer.inMilliseconds / state.duration.inMilliseconds
        : 0.0;
    final expanded = _hovering || _dragging;

    return LayoutBuilder(builder: (context, constraints) {
      final width = constraints.maxWidth;

      return MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        onHover: (event) {
          setState(() {
            _hoverProgress = (event.localPosition.dx / width).clamp(0.0, 1.0);
          });
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (details) {
            final frac = (details.localPosition.dx / width).clamp(0.0, 1.0);
            ref.read(playbackProvider.notifier).seek(
              Duration(milliseconds: (frac * state.duration.inMilliseconds).toInt()),
            );
          },
          onHorizontalDragStart: (details) {
            setState(() {
              _dragging = true;
              _dragProgress = (details.localPosition.dx / width).clamp(0.0, 1.0);
            });
          },
          onHorizontalDragUpdate: (details) {
            setState(() {
              _dragProgress = (details.localPosition.dx / width).clamp(0.0, 1.0);
            });
          },
          onHorizontalDragEnd: (_) {
            ref.read(playbackProvider.notifier).seek(
              Duration(milliseconds: (_dragProgress * state.duration.inMilliseconds).toInt()),
            );
            setState(() => _dragging = false);
          },
          // Tall hit area for easy clicking
          child: SizedBox(
            height: 28,
            child: Stack(
              alignment: Alignment.center,
              clipBehavior: Clip.none,
              children: [
                // Track background
                AnimatedContainer(
                  duration: const Duration(milliseconds: 100),
                  height: expanded ? 6 : 4,
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(3),
                    color: Colors.white.withValues(alpha: 0.15),
                  ),
                ),
                // Buffered
                Align(
                  alignment: Alignment.centerLeft,
                  child: AnimatedContainer(
                    duration: const Duration(milliseconds: 100),
                    height: expanded ? 6 : 4,
                    width: (bufferProgress.clamp(0.0, 1.0) * width),
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(3),
                      color: Colors.white.withValues(alpha: 0.25),
                    ),
                  ),
                ),
                // Progress fill
                Align(
                  alignment: Alignment.centerLeft,
                  child: AnimatedContainer(
                    duration: _dragging ? Duration.zero : const Duration(milliseconds: 100),
                    height: expanded ? 6 : 4,
                    width: (progress.clamp(0.0, 1.0) * width),
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(3),
                      color: AppColors.primary,
                    ),
                  ),
                ),
                // Drag handle
                if (expanded)
                  Positioned(
                    left: (progress.clamp(0.0, 1.0) * width) - 7,
                    child: Container(
                      width: 14,
                      height: 14,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: AppColors.primary,
                        boxShadow: [
                          BoxShadow(
                            color: AppColors.primary.withValues(alpha: 0.4),
                            blurRadius: 6,
                          ),
                        ],
                      ),
                    ),
                  ),
                // Hover time tooltip
                if (_hovering && !_dragging)
                  Positioned(
                    left: (_hoverProgress * width).clamp(24.0, width - 24.0) - 24,
                    top: -28,
                    child: Container(
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                      decoration: BoxDecoration(
                        color: Colors.black87,
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        formatDuration(Duration(
                          milliseconds: (_hoverProgress * state.duration.inMilliseconds).toInt(),
                        )),
                        style: const TextStyle(fontSize: 11, color: Colors.white),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      );
    });
  }
}

// ---------------------------------------------------------------------------
// Volume Control
// ---------------------------------------------------------------------------

class _VolumeControl extends ConsumerStatefulWidget {
  final PlaybackState state;
  const _VolumeControl({required this.state});

  @override
  ConsumerState<_VolumeControl> createState() => _VolumeControlState();
}

class _VolumeControlState extends ConsumerState<_VolumeControl> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final notifier = ref.read(playbackProvider.notifier);

    IconData volumeIcon;
    if (state.muted || state.volume == 0) {
      volumeIcon = Icons.volume_off_rounded;
    } else if (state.volume < 50) {
      volumeIcon = Icons.volume_down_rounded;
    } else {
      volumeIcon = Icons.volume_up_rounded;
    }

    return MouseRegion(
      onEnter: (_) => setState(() => _expanded = true),
      onExit: (_) => setState(() => _expanded = false),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          PlayerIconBtn(
            icon: volumeIcon,
            size: 22,
            onTap: notifier.toggleMute,
          ),
          AnimatedContainer(
            duration: const Duration(milliseconds: 150),
            width: _expanded ? 80 : 0,
            child: _expanded
                ? SliderTheme(
                    data: SliderThemeData(
                      trackHeight: 3,
                      thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 5),
                      activeTrackColor: AppColors.primary,
                      inactiveTrackColor: Colors.white24,
                      thumbColor: AppColors.primary,
                      overlayShape: SliderComponentShape.noOverlay,
                    ),
                    child: Slider(
                      value: state.muted ? 0 : state.volume,
                      min: 0,
                      max: 100,
                      onChanged: (v) => notifier.setVolume(v),
                    ),
                  )
                : const SizedBox.shrink(),
          ),
        ],
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Subtitle Picker
// ---------------------------------------------------------------------------

class SubtitlePicker extends ConsumerWidget {
  final PlaybackState state;
  const SubtitlePicker({super.key, required this.state});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return PlayerIconBtn(
      icon: Icons.subtitles_outlined,
      size: 20,
      onTap: () => _showPicker(context, ref),
    );
  }

  void _showPicker(BuildContext context, WidgetRef ref) {
    final RenderBox button = context.findRenderObject() as RenderBox;
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    final position = RelativeRect.fromRect(
      button.localToGlobal(Offset.zero, ancestor: overlay) & button.size,
      Offset.zero & overlay.size,
    );

    final items = <PopupMenuEntry<String>>[
      const PopupMenuItem(value: 'none', child: Text('Off')),
    ];

    for (final track in state.subtitleTracks) {
      final label = track.title ?? track.language ?? track.id;
      items.add(PopupMenuItem(
        value: 'embedded:${track.id}',
        child: Row(
          children: [
            if (state.activeSubtitle?.id == track.id)
              const Icon(Icons.check, size: 16, color: AppColors.primary)
            else
              const SizedBox(width: 16),
            const SizedBox(width: 8),
            Expanded(child: Text(label, overflow: TextOverflow.ellipsis)),
          ],
        ),
      ));
    }

    if (state.externalSubs.isNotEmpty) {
      items.add(const PopupMenuDivider());
      for (final sub in state.externalSubs) {
        items.add(PopupMenuItem(
          value: 'ext:${sub.path}',
          child: Row(
            children: [
              const SizedBox(width: 24),
              Expanded(child: Text('${sub.language} (.${sub.format})', overflow: TextOverflow.ellipsis)),
            ],
          ),
        ));
      }
    }

    showMenu<String>(
      context: context,
      position: position,
      items: items,
      color: AppColors.surfaceElevated,
    ).then((value) {
      if (value == null) return;
      final s = ref.read(playbackProvider);
      final n = ref.read(playbackProvider.notifier);
      if (value == 'none') {
        n.disableSubtitles();
      } else if (value.startsWith('embedded:')) {
        final id = value.substring(9);
        try {
          n.setSubtitleTrack(s.subtitleTracks.firstWhere((t) => t.id == id));
        } catch (e) {
          debugPrint('[player] Subtitle track lookup failed: $e');
        }
      } else if (value.startsWith('ext:')) {
        try {
          n.loadExternalSubtitle(s.externalSubs.firstWhere((e) => e.path == value.substring(4)));
        } catch (e) {
          debugPrint('[player] External subtitle lookup failed: $e');
        }
      }
    });
  }
}

// ---------------------------------------------------------------------------
// Audio Picker
// ---------------------------------------------------------------------------

class AudioPicker extends ConsumerWidget {
  final PlaybackState state;
  const AudioPicker({super.key, required this.state});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return PlayerIconBtn(
      icon: Icons.audiotrack_outlined,
      size: 20,
      onTap: () => _showPicker(context, ref),
    );
  }

  void _showPicker(BuildContext context, WidgetRef ref) {
    final RenderBox button = context.findRenderObject() as RenderBox;
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    final position = RelativeRect.fromRect(
      button.localToGlobal(Offset.zero, ancestor: overlay) & button.size,
      Offset.zero & overlay.size,
    );

    final items = <PopupMenuEntry<String>>[];
    for (final track in state.audioTracks) {
      final label = track.title ?? track.language ?? track.id;
      items.add(PopupMenuItem(
        value: track.id,
        child: Row(
          children: [
            if (state.activeAudio?.id == track.id)
              const Icon(Icons.check, size: 16, color: AppColors.primary)
            else
              const SizedBox(width: 16),
            const SizedBox(width: 8),
            Expanded(child: Text(label, overflow: TextOverflow.ellipsis)),
          ],
        ),
      ));
    }

    if (items.isEmpty) return;

    showMenu<String>(
      context: context,
      position: position,
      items: items,
      color: AppColors.surfaceElevated,
    ).then((value) {
      if (value == null) return;
      final s = ref.read(playbackProvider);
      try {
        ref.read(playbackProvider.notifier).setAudioTrack(
          s.audioTracks.firstWhere((t) => t.id == value),
        );
      } catch (e) {
        debugPrint('[player] Audio track lookup failed: $e');
      }
    });
  }
}

// ---------------------------------------------------------------------------
// Auto-Play Overlay
// ---------------------------------------------------------------------------

class AutoPlayOverlay extends ConsumerWidget {
  final PlaybackState state;
  const AutoPlayOverlay({super.key, required this.state});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final nextIndex = state.currentIndex + 1;
    if (nextIndex >= state.playlist.length) return const SizedBox.shrink();
    final nextFile = state.playlist[nextIndex];
    final notifier = ref.read(playbackProvider.notifier);

    String nextLabel;
    if (nextFile.season != null && nextFile.episode != null) {
      nextLabel = 'S${nextFile.season.toString().padLeft(2, '0')}E${nextFile.episode.toString().padLeft(2, '0')}';
      if (nextFile.episodeTitle != null) nextLabel += ' - ${nextFile.episodeTitle}';
    } else {
      nextLabel = nextFile.filename;
    }

    return Positioned(
      bottom: 80,
      right: 24,
      child: Container(
        width: 280,
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: AppColors.surfaceElevated.withValues(alpha: 0.95),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: AppColors.border),
          boxShadow: [
            BoxShadow(color: Colors.black.withValues(alpha: 0.5), blurRadius: 20, offset: const Offset(0, 8)),
          ],
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                const Expanded(
                  child: Text('Next Episode',
                    style: TextStyle(fontSize: 13, fontWeight: FontWeight.w600, color: AppColors.textPrimary)),
                ),
                MouseRegion(
                  cursor: SystemMouseCursors.click,
                  child: GestureDetector(
                    onTap: notifier.cancelAutoPlay,
                    child: const Icon(Icons.close_rounded, size: 18, color: AppColors.textTertiary),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 6),
            Text(nextLabel,
              style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
              maxLines: 1, overflow: TextOverflow.ellipsis),
            const SizedBox(height: 12),
            MouseRegion(
              cursor: SystemMouseCursors.click,
              child: GestureDetector(
                onTap: notifier.playNext,
                child: Container(
                  width: double.infinity,
                  padding: const EdgeInsets.symmetric(vertical: 10),
                  decoration: BoxDecoration(color: AppColors.primary, borderRadius: BorderRadius.circular(8)),
                  alignment: Alignment.center,
                  child: Text('Play Now (${state.autoPlayCountdown}s)',
                    style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600, color: Colors.white)),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
