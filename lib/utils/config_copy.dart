import 'package:reel/src/rust/config.dart';

/// Copy a Config with optional field overrides.
/// Config is FRB-generated and lacks copyWith, so this is the shared helper.
Config copyConfig(Config c, {
  String? libraryPath,
  String? watchPath,
  String? tmdbApiKey,
  String? opensubsApiKey,
  String? tvdbApiKey,
  List<String>? subtitleLanguages,
  bool? autoDownloadSubs,
  QbitConfig? qbittorrent,
  bool? qbitEnabled,
  bool? watcherEnabled,
  String? theme,
  bool clearLibraryPath = false,
  bool clearWatchPath = false,
}) {
  return Config(
    libraryPath: clearLibraryPath ? null : (libraryPath ?? c.libraryPath),
    watchPath: clearWatchPath ? null : (watchPath ?? c.watchPath),
    tmdbApiKey: tmdbApiKey ?? c.tmdbApiKey,
    opensubsApiKey: opensubsApiKey ?? c.opensubsApiKey,
    tvdbApiKey: tvdbApiKey ?? c.tvdbApiKey,
    subtitleLanguages: subtitleLanguages ?? c.subtitleLanguages,
    autoDownloadSubs: autoDownloadSubs ?? c.autoDownloadSubs,
    qbittorrent: qbittorrent ?? c.qbittorrent,
    qbitEnabled: qbitEnabled ?? c.qbitEnabled,
    watcherEnabled: watcherEnabled ?? c.watcherEnabled,
    theme: theme ?? c.theme,
  );
}
