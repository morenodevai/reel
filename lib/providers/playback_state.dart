import 'package:flutter/foundation.dart';
import 'package:reel/src/rust/library.dart';
import 'package:reel/src/rust/db/watch_progress.dart';
import 'package:reel/src/rust/api/playback_api.dart' as playback_api;
import 'package:media_kit/media_kit.dart';

/// Immutable state for the video player.
class PlaybackState {
  final bool playing;
  final Duration position;
  final Duration duration;
  final Duration buffer;
  final double volume;
  final bool muted;
  final bool fullscreen;
  final bool controlsVisible;
  final List<SubtitleTrack> subtitleTracks;
  final List<AudioTrack> audioTracks;
  final SubtitleTrack? activeSubtitle;
  final AudioTrack? activeAudio;
  final MediaDetail? mediaDetail;
  final MediaFile? currentFile;
  final List<MediaFile> playlist;
  final int currentIndex;
  final bool loading;
  final String? error;
  final int autoPlayCountdown; // 0 = not counting, >0 = seconds remaining
  final List<ExternalSubInfo> externalSubs;

  const PlaybackState({
    this.playing = false,
    this.position = Duration.zero,
    this.duration = Duration.zero,
    this.buffer = Duration.zero,
    this.volume = 100.0,
    this.muted = false,
    this.fullscreen = false,
    this.controlsVisible = true,
    this.subtitleTracks = const [],
    this.audioTracks = const [],
    this.activeSubtitle,
    this.activeAudio,
    this.mediaDetail,
    this.currentFile,
    this.playlist = const [],
    this.currentIndex = 0,
    this.loading = true,
    this.error,
    this.autoPlayCountdown = 0,
    this.externalSubs = const [],
  });

  PlaybackState copyWith({
    bool? playing,
    Duration? position,
    Duration? duration,
    Duration? buffer,
    double? volume,
    bool? muted,
    bool? fullscreen,
    bool? controlsVisible,
    List<SubtitleTrack>? subtitleTracks,
    List<AudioTrack>? audioTracks,
    SubtitleTrack? activeSubtitle,
    AudioTrack? activeAudio,
    MediaDetail? mediaDetail,
    MediaFile? currentFile,
    List<MediaFile>? playlist,
    int? currentIndex,
    bool? loading,
    String? error,
    int? autoPlayCountdown,
    List<ExternalSubInfo>? externalSubs,
    bool clearError = false,
    bool clearActiveSubtitle = false,
  }) {
    return PlaybackState(
      playing: playing ?? this.playing,
      position: position ?? this.position,
      duration: duration ?? this.duration,
      buffer: buffer ?? this.buffer,
      volume: volume ?? this.volume,
      muted: muted ?? this.muted,
      fullscreen: fullscreen ?? this.fullscreen,
      controlsVisible: controlsVisible ?? this.controlsVisible,
      subtitleTracks: subtitleTracks ?? this.subtitleTracks,
      audioTracks: audioTracks ?? this.audioTracks,
      activeSubtitle: clearActiveSubtitle ? null : (activeSubtitle ?? this.activeSubtitle),
      activeAudio: activeAudio ?? this.activeAudio,
      mediaDetail: mediaDetail ?? this.mediaDetail,
      currentFile: currentFile ?? this.currentFile,
      playlist: playlist ?? this.playlist,
      currentIndex: currentIndex ?? this.currentIndex,
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
      autoPlayCountdown: autoPlayCountdown ?? this.autoPlayCountdown,
      externalSubs: externalSubs ?? this.externalSubs,
    );
  }

  bool get isTV => mediaDetail?.mediaType == 'tv';
  bool get hasNext => currentIndex < playlist.length - 1;
  bool get hasPrevious => currentIndex > 0;

  /// Progress fraction 0.0-1.0.
  double get progress =>
      duration.inMilliseconds > 0 ? position.inMilliseconds / duration.inMilliseconds : 0.0;
}

/// Info about an external subtitle file (from Rust discovery).
class ExternalSubInfo {
  final String path;
  final String language;
  final String format;
  const ExternalSubInfo({required this.path, required this.language, required this.format});
}

/// What to play: a file, the season-scoped playlist, and the index within it.
class PlayTarget {
  final MediaFile file;
  final List<MediaFile> playlist;
  final int index;
  const PlayTarget({required this.file, required this.playlist, required this.index});
}

/// Determine the right file to play for a given media.
///
/// Movies: plays the first (usually only) file.
/// Series: finds the most recently watched episode by timestamp.
/// If in-progress, resumes it. If completed, plays the next episode.
/// Falls back to S01E01 if no progress exists or the series is finished.
/// Playlist is scoped to the target episode's season.
Future<PlayTarget?> resolvePlayTarget(MediaDetail detail) async {
  if (detail.files.isEmpty) return null;

  // Sort all files by season, then episode
  final sorted = List<MediaFile>.from(detail.files)
    ..sort((a, b) {
      final sc = (a.season ?? 0).compareTo(b.season ?? 0);
      return sc != 0 ? sc : (a.episode ?? 0).compareTo(b.episode ?? 0);
    });

  if (detail.mediaType != 'tv') {
    return PlayTarget(file: sorted.first, playlist: sorted, index: 0);
  }

  // Series — load watch progress for all episodes
  List<WatchProgress> allProgress = [];
  try {
    allProgress = await playback_api.loadAllProgress(mediaPath: detail.path);
  } catch (e) {
    debugPrint('[playback] Failed to load watch progress for play target: $e');
  }

  // Sort progress by lastWatched descending to find the most recently touched episode.
  // Try each entry in case the most recent one references a file that no longer exists.
  final sortedProgress = List<WatchProgress>.from(allProgress)
    ..sort((a, b) => b.lastWatched.compareTo(a.lastWatched));

  MediaFile? target;
  for (final progress in sortedProgress) {
    final idx = sorted.indexWhere((f) => f.path == progress.filePath);
    if (idx < 0) continue; // file no longer in library, skip

    if (!progress.completed) {
      target = sorted[idx];
    } else if (idx + 1 < sorted.length) {
      target = sorted[idx + 1];
    }
    break;
  }

  // No valid progress or series fully completed — start from S01E01
  final resolved = target ?? sorted.first;

  // Scope playlist to the target's season
  final seasonFiles = sorted.where((f) => f.season == resolved.season).toList();
  final seasonIdx = seasonFiles.indexWhere((f) => f.path == resolved.path);

  return PlayTarget(
    file: resolved,
    playlist: seasonFiles,
    index: seasonIdx >= 0 ? seasonIdx : 0,
  );
}
