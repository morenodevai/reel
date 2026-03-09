import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:media_kit/media_kit.dart';
import 'package:window_manager/window_manager.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/api/playback_api.dart' as playback_api;

// Re-export state types so existing imports keep working.
export 'package:reel/providers/playback_state.dart';
import 'package:reel/providers/playback_state.dart';

class PlaybackNotifier extends Notifier<PlaybackState> {
  late final Player _player;
  final List<StreamSubscription> _subs = [];
  Timer? _saveTimer;
  Timer? _controlsTimer;
  Timer? _autoPlayTimer;
  StreamSubscription<Duration>? _resumeSub;
  Timer? _resumeTimeout;
  bool _disposed = false;
  double _preMuteVolume = 100.0;
  bool _autoPlayTriggered = false;

  Player get player => _player;

  @override
  PlaybackState build() {
    _player = Player();
    _setupStreams();
    // Start periodic save
    _saveTimer = Timer.periodic(const Duration(seconds: 10), (_) => _saveProgress());
    ref.onDispose(_cleanup);
    return const PlaybackState();
  }

  void _setupStreams() {
    _subs.add(_player.stream.playing.listen((playing) {
      if (!_disposed) state = state.copyWith(playing: playing);
    }));
    _subs.add(_player.stream.position.listen((pos) {
      if (!_disposed) {
        state = state.copyWith(position: pos);
        _checkCompletion();
      }
    }));
    _subs.add(_player.stream.duration.listen((dur) {
      if (!_disposed) state = state.copyWith(duration: dur);
    }));
    _subs.add(_player.stream.buffer.listen((buf) {
      if (!_disposed) state = state.copyWith(buffer: buf);
    }));
    _subs.add(_player.stream.buffering.listen((buffering) {
      if (!_disposed) state = state.copyWith(loading: buffering);
    }));
    _subs.add(_player.stream.completed.listen((completed) {
      if (!_disposed && completed) _onCompleted();
    }));
    _subs.add(_player.stream.tracks.listen((tracks) {
      if (_disposed) return;
      state = state.copyWith(
        subtitleTracks: tracks.subtitle,
        audioTracks: tracks.audio,
      );
    }));
    _subs.add(_player.stream.track.listen((track) {
      if (_disposed) return;
      state = state.copyWith(
        activeSubtitle: track.subtitle,
        activeAudio: track.audio,
      );
    }));
    _subs.add(_player.stream.error.listen((error) {
      if (!_disposed) state = state.copyWith(error: error);
    }));
  }

  /// Open a file for playback. Seeks to saved position if resuming.
  Future<void> open(
    MediaDetail detail,
    MediaFile file,
    List<MediaFile> playlist,
    int index,
  ) async {
    _cancelAutoPlay();
    state = state.copyWith(
      mediaDetail: detail,
      currentFile: file,
      playlist: playlist,
      currentIndex: index,
      loading: true,
      clearError: true,
      autoPlayCountdown: 0,
    );

    _autoPlayTriggered = false;

    // Load saved position BEFORE opening the file
    Duration? resumePos;
    try {
      final progress = await playback_api.loadWatchProgress(filePath: file.path);
      if (progress != null && !progress.completed && progress.positionSeconds > 5.0) {
        resumePos = Duration(milliseconds: (progress.positionSeconds * 1000).toInt());
      }
    } catch (e) {
      debugPrint('Failed to load watch progress: $e');
    }

    // Open the file in media_kit
    await _player.open(Media('file://${file.path}'));

    // Discover external subtitles
    _discoverExternalSubs(file.path);

    // Wait for player to be ready (duration available), then seek to resume position
    if (resumePos != null) {
      // Cancel any previous resume subscription
      _resumeSub?.cancel();
      _resumeTimeout?.cancel();
      _resumeSub = _player.stream.duration.listen((dur) {
        if (dur.inSeconds > 0 && resumePos != null) {
          _player.seek(resumePos!);
          resumePos = null; // Only seek once
          _resumeSub?.cancel();
          _resumeSub = null;
          _resumeTimeout?.cancel();
          _resumeTimeout = null;
        }
      });
      // Cancel after 5s to avoid leaking
      _resumeTimeout = Timer(const Duration(seconds: 5), () {
        _resumeSub?.cancel();
        _resumeSub = null;
        _resumeTimeout = null;
      });
    }
  }

  Future<void> _discoverExternalSubs(String videoPath) async {
    try {
      final subs = await playback_api.discoverExternalSubtitles(videoPath: videoPath);
      final externalSubs = subs
          .map((s) => ExternalSubInfo(path: s.path, language: s.language, format: s.format))
          .toList();
      if (!_disposed) {
        state = state.copyWith(externalSubs: externalSubs);
      }
    } catch (e) {
      debugPrint('[playback] Failed to discover external subtitles: $e');
    }
  }

  // -- Playback controls --

  void togglePlay() {
    _player.playOrPause();
    _saveProgress();
  }

  void pause() {
    _player.pause();
    _saveProgress();
  }

  Future<void> seek(Duration position) async {
    await _player.seek(position);
  }

  void seekRelative(int seconds) {
    final target = state.position + Duration(seconds: seconds);
    final clamped = target.isNegative
        ? Duration.zero
        : (target > state.duration ? state.duration : target);
    _player.seek(clamped);
  }

  void setVolume(double volume) {
    final clamped = volume.clamp(0.0, 100.0);
    _player.setVolume(clamped);
    if (clamped > 0) _preMuteVolume = clamped;
    state = state.copyWith(volume: clamped, muted: false);
  }

  void toggleMute() {
    if (state.muted) {
      final restored = _preMuteVolume > 0 ? _preMuteVolume : 100.0;
      _player.setVolume(restored);
      state = state.copyWith(volume: restored, muted: false);
    } else {
      _preMuteVolume = state.volume > 0 ? state.volume : _preMuteVolume;
      _player.setVolume(0);
      state = state.copyWith(muted: true);
    }
  }

  void adjustVolume(double delta) {
    setVolume((state.muted ? _preMuteVolume : state.volume) + delta);
  }

  Future<void> toggleFullscreen() async {
    final newFs = !state.fullscreen;
    await windowManager.setFullScreen(newFs);
    state = state.copyWith(fullscreen: newFs);
  }

  // -- Subtitle / audio track selection --

  Future<void> setSubtitleTrack(SubtitleTrack track) async {
    await _player.setSubtitleTrack(track);
    state = state.copyWith(activeSubtitle: track);
  }

  Future<void> disableSubtitles() async {
    await _player.setSubtitleTrack(SubtitleTrack.no());
    state = state.copyWith(clearActiveSubtitle: true);
  }

  Future<void> setAudioTrack(AudioTrack track) async {
    await _player.setAudioTrack(track);
    state = state.copyWith(activeAudio: track);
  }

  Future<void> loadExternalSubtitle(ExternalSubInfo sub) async {
    await _player.setSubtitleTrack(
      SubtitleTrack.uri('file://${sub.path}', title: sub.language, language: sub.language.toLowerCase()),
    );
  }

  // -- Controls visibility --

  void showControls() {
    state = state.copyWith(controlsVisible: true);
    _resetControlsTimer();
  }

  void hideControls() {
    state = state.copyWith(controlsVisible: false);
  }

  void _resetControlsTimer() {
    _controlsTimer?.cancel();
    if (state.playing) {
      _controlsTimer = Timer(const Duration(seconds: 3), () {
        if (!_disposed && state.playing) {
          state = state.copyWith(controlsVisible: false);
        }
      });
    }
  }

  void onMouseActivity() {
    if (!state.controlsVisible) {
      state = state.copyWith(controlsVisible: true);
    }
    _resetControlsTimer();
  }

  // -- Playlist navigation --

  Future<void> playNext() async {
    if (!state.hasNext) return;
    _cancelAutoPlay();
    final nextIndex = state.currentIndex + 1;
    final nextFile = state.playlist[nextIndex];
    await open(state.mediaDetail!, nextFile, state.playlist, nextIndex);
  }

  Future<void> playPrevious() async {
    if (!state.hasPrevious) return;
    _cancelAutoPlay();
    final prevIndex = state.currentIndex - 1;
    final prevFile = state.playlist[prevIndex];
    await open(state.mediaDetail!, prevFile, state.playlist, prevIndex);
  }

  Future<void> playAtIndex(int index) async {
    if (index < 0 || index >= state.playlist.length) return;
    _cancelAutoPlay();
    final file = state.playlist[index];
    await open(state.mediaDetail!, file, state.playlist, index);
  }

  // -- Completion & auto-play --

  void _checkCompletion() {
    if (_autoPlayTriggered) return;
    if (state.autoPlayCountdown > 0) return;
    if (state.duration.inSeconds < 10) return;
    if (state.progress >= 0.9 && state.isTV && state.hasNext) {
      _startAutoPlayCountdown();
    }
  }

  void _onCompleted() {
    _markCompleted();
    if (state.isTV && state.hasNext) {
      _startAutoPlayCountdown();
    }
  }

  void _startAutoPlayCountdown() {
    if (state.autoPlayCountdown > 0) return;
    _autoPlayTriggered = true;
    state = state.copyWith(autoPlayCountdown: 5);
    _autoPlayTimer?.cancel();
    _autoPlayTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (_disposed) {
        timer.cancel();
        return;
      }
      final remaining = state.autoPlayCountdown - 1;
      if (remaining <= 0) {
        timer.cancel();
        state = state.copyWith(autoPlayCountdown: 0);
        playNext();
      } else {
        state = state.copyWith(autoPlayCountdown: remaining);
      }
    });
  }

  void cancelAutoPlay() {
    _cancelAutoPlay();
    state = state.copyWith(autoPlayCountdown: 0);
  }

  void _cancelAutoPlay() {
    _autoPlayTimer?.cancel();
    _autoPlayTimer = null;
  }

  // -- Progress persistence --

  Future<void> _saveProgress() async {
    final file = state.currentFile;
    final detail = state.mediaDetail;
    if (file == null || state.duration.inSeconds == 0) return;

    try {
      await playback_api.saveWatchProgress(
        filePath: file.path,
        position: state.position.inMilliseconds / 1000.0,
        duration: state.duration.inMilliseconds / 1000.0,
        title: detail?.title,
        posterUrl: detail?.posterUrl,
        mediaPath: detail?.path,
        season: file.season,
        episode: file.episode,
        episodeTitle: file.episodeTitle,
        mediaType: detail?.mediaType,
      );
    } catch (e) {
      debugPrint('[playback] Failed to save watch progress: $e');
    }
  }

  Future<void> _markCompleted() async {
    final file = state.currentFile;
    if (file == null) return;
    try {
      await playback_api.setFileCompleted(filePath: file.path);
    } catch (e) {
      debugPrint('[playback] Failed to mark file completed: $e');
    }
  }

  /// Called when navigating away from the player.
  Future<void> onLeavePlayer() async {
    await _player.pause();
    await _saveProgress();
    _cancelAutoPlay();
    if (state.fullscreen) {
      await windowManager.setFullScreen(false);
      state = state.copyWith(fullscreen: false);
    }
  }

  void _cleanup() {
    _disposed = true;
    _saveTimer?.cancel();
    _controlsTimer?.cancel();
    _autoPlayTimer?.cancel();
    _resumeSub?.cancel();
    _resumeTimeout?.cancel();
    for (final sub in _subs) {
      sub.cancel();
    }
    _subs.clear();
    _player.dispose();
  }
}

final playbackProvider =
    NotifierProvider<PlaybackNotifier, PlaybackState>(PlaybackNotifier.new);
