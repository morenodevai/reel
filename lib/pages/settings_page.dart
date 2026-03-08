import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:reel/src/rust/config.dart';
import 'package:reel/src/rust/api/ai_api.dart' as ai_api;
import 'package:reel/src/rust/api/qbit_api.dart' as qbit_api;
import 'package:reel/src/rust/api/pipeline_api.dart' as pipeline_api;
import 'package:reel/providers/config_provider.dart';
import 'package:reel/providers/qbit_provider.dart';
import 'package:reel/pages/settings_widgets.dart';
import 'package:reel/theme/app_theme.dart';

/// Language codes for subtitle selection.
const _languages = [
  ('eng', 'English'),
  ('spa', 'Spanish'),
  ('fre', 'French'),
  ('ger', 'German'),
  ('jpn', 'Japanese'),
  ('por', 'Portuguese'),
  ('ita', 'Italian'),
  ('kor', 'Korean'),
  ('chi', 'Chinese'),
  ('ara', 'Arabic'),
  ('hin', 'Hindi'),
];

class SettingsPageWidget extends ConsumerStatefulWidget {
  const SettingsPageWidget({super.key});

  @override
  ConsumerState<SettingsPageWidget> createState() => _SettingsPageWidgetState();
}

class _SettingsPageWidgetState extends ConsumerState<SettingsPageWidget> {
  String? _qbitStatusOverride;
  Timer? _overrideClearTimer;
  bool _aiReady = false;
  String? _aiError;
  bool _showQbitManual = false;
  bool _showApiKeys = false;
  final _revealKeys = <String, bool>{};
  bool _rescanning = false;
  String _rescanStatus = '';
  Timer? _rescanClearTimer;
  StreamSubscription<String>? _rescanSub;
  Timer? _aiPollTimer;
  String _appVersion = '';

  @override
  void initState() {
    super.initState();
    _checkAi();
    _loadVersion();
    _aiPollTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (!_aiReady && _aiError == null) _checkAi();
    });
  }

  Future<void> _loadVersion() async {
    final info = await PackageInfo.fromPlatform();
    if (mounted) setState(() => _appVersion = info.version);
  }

  @override
  void dispose() {
    _aiPollTimer?.cancel();
    _rescanClearTimer?.cancel();
    _overrideClearTimer?.cancel();
    _rescanSub?.cancel();
    super.dispose();
  }

  Future<void> _checkAi() async {
    try {
      final ready = await ai_api.isAiReady();
      if (mounted) setState(() => _aiReady = ready);
    } catch (e) {
      if (mounted) setState(() => _aiError = 'Check failed: $e');
    }
  }

  void _updateConfig(Config Function(Config c) updater) {
    ref.read(configProvider.notifier).updateConfig(updater);
  }

  Config _copyConfig(Config c, {
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

  @override
  Widget build(BuildContext context) {
    final config = ref.watch(configProvider).value;
    if (config == null) {
      return const Center(child: CircularProgressIndicator());
    }

    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // --- Library section ---
              const SectionHeader('Library'),
              SettingsCard(children: [
                SettingRow(
                  label: 'Library Folder',
                  trailing: SmallButton(
                    icon: Icons.folder_open_outlined,
                    label: config.libraryPath != null ? 'Change' : 'Choose Folder',
                    onTap: _pickLibraryFolder,
                  ),
                ),
                if (config.libraryPath != null)
                  PathText(config.libraryPath!),

                const Divider(height: 24),

                SettingRow(
                  label: 'Watch folder for new files',
                  trailing: ToggleSwitch(
                    value: config.watcherEnabled,
                    onChanged: (v) => _updateConfig((c) => _copyConfig(c, watcherEnabled: v)),
                  ),
                ),
                if (config.watcherEnabled) ...[
                  const SizedBox(height: 8),
                  SettingRow(
                    label: 'Drop folder',
                    isSubsetting: true,
                    trailing: SmallButton(
                      icon: Icons.folder_open_outlined,
                      label: config.watchPath != null ? 'Change' : 'Choose Folder',
                      onTap: _pickWatchFolder,
                    ),
                  ),
                  if (config.watchPath != null)
                    PathText(config.watchPath!)
                  else
                    const Padding(
                      padding: EdgeInsets.only(top: 4),
                      child: Text(
                        'Files dropped here get auto-organized into your library',
                        style: TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                      ),
                    ),
                ],

                if (config.libraryPath != null) ...[
                  const Divider(height: 24),
                  SettingRow(
                    label: 'Rescan library',
                    subtitle: 'Fix misidentified files, download missing subtitles',
                    trailing: SmallButton(
                      icon: Icons.refresh,
                      label: _rescanning ? 'Scanning...' : 'Rescan',
                      onTap: _rescanning ? null : _handleRescan,
                    ),
                  ),
                  if (_rescanStatus.isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: 6),
                      child: Text(
                        _rescanStatus,
                        style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                ],
              ]),

              const SizedBox(height: 24),

              // --- Subtitles section ---
              const SectionHeader('Subtitles'),
              SettingsCard(children: [
                SettingRow(
                  label: 'Auto-download subtitles',
                  trailing: ToggleSwitch(
                    value: config.autoDownloadSubs,
                    onChanged: (v) => _updateConfig((c) => _copyConfig(c, autoDownloadSubs: v)),
                  ),
                ),
                const Divider(height: 24),
                const Text(
                  'Languages',
                  style: TextStyle(fontSize: 12, color: AppColors.textSecondary),
                ),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: _languages.map((lang) {
                    final selected = config.subtitleLanguages.contains(lang.$1);
                    return MouseRegion(
                      cursor: SystemMouseCursors.click,
                      child: GestureDetector(
                        onTap: () => _toggleLanguage(lang.$1, config),
                        child: Container(
                          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                          decoration: BoxDecoration(
                            color: selected
                                ? AppColors.primary.withValues(alpha: 0.2)
                                : AppColors.surfaceElevated,
                            borderRadius: BorderRadius.circular(6),
                          ),
                          child: Text(
                            '${lang.$2} (${lang.$1})',
                            style: TextStyle(
                              fontSize: 12,
                              color: selected ? AppColors.primary : AppColors.textTertiary,
                            ),
                          ),
                        ),
                      ),
                    );
                  }).toList(),
                ),
              ]),

              const SizedBox(height: 24),

              // --- qBittorrent section ---
              const SectionHeader('qBittorrent'),
              SettingsCard(children: [
                SettingRow(
                  label: 'Auto-import from qBittorrent',
                  trailing: ToggleSwitch(
                    value: config.qbitEnabled,
                    onChanged: (v) => ref.read(configProvider.notifier).updateConfig(
                      (c) => _copyConfig(c, qbitEnabled: v),
                      immediate: true,
                    ),
                  ),
                ),
                if (config.qbitEnabled) ...[
                  const Divider(height: 24),
                  Builder(builder: (context) {
                    final qbit = ref.watch(qbitProvider);
                    final status = _qbitStatusOverride ?? qbit.status;
                    final connected = qbit.running || status.startsWith('Connected');
                    return Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Row(
                        children: [
                          Icon(
                            connected ? Icons.wifi : Icons.wifi_off,
                            size: 14,
                            color: connected
                                ? AppColors.success
                                : AppColors.textQuaternary,
                          ),
                          const SizedBox(width: 8),
                          Text(
                            status.isEmpty ? 'Not configured' : status,
                            style: const TextStyle(fontSize: 12, color: AppColors.textTertiary),
                          ),
                        ],
                      ),
                      SmallButton(
                        label: 'Auto-detect',
                        onTap: _autoDetectQbit,
                      ),
                    ],
                  );
                  }),
                  const SizedBox(height: 8),
                  MouseRegion(
                    cursor: SystemMouseCursors.click,
                    child: GestureDetector(
                      onTap: () => setState(() => _showQbitManual = !_showQbitManual),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(
                            _showQbitManual ? Icons.expand_less : Icons.expand_more,
                            size: 12,
                            color: AppColors.textTertiary,
                          ),
                          const SizedBox(width: 4),
                          const Text(
                            'Configure manually',
                            style: TextStyle(fontSize: 12, color: AppColors.textTertiary),
                          ),
                        ],
                      ),
                    ),
                  ),
                  if (_showQbitManual) ...[
                    const Divider(height: 16),
                    QbitManualConfig(
                      config: config.qbittorrent,
                      onUpdate: (field, value) => _updateQbit(field, value, config),
                      onTest: () => _testQbit(config),
                    ),
                  ],
                  const Divider(height: 24),
                  SettingRow(
                    label: 'Auto-remove completed',
                    isSubsetting: true,
                    trailing: ToggleSwitch(
                      value: config.qbittorrent.autoRemove,
                      onChanged: (v) {
                        final newQbit = QbitConfig(
                          host: config.qbittorrent.host,
                          port: config.qbittorrent.port,
                          username: config.qbittorrent.username,
                          password: config.qbittorrent.password,
                          autoRemove: v,
                        );
                        _updateConfig((c) => _copyConfig(c, qbittorrent: newQbit));
                      },
                    ),
                  ),
                ],
              ]),

              const SizedBox(height: 24),

              // --- API Keys section ---
              MouseRegion(
                cursor: SystemMouseCursors.click,
                child: GestureDetector(
                  onTap: () => setState(() => _showApiKeys = !_showApiKeys),
                  child: Row(
                    children: [
                      const Icon(Icons.key, size: 12, color: AppColors.textTertiary),
                      const SizedBox(width: 6),
                      Icon(
                        _showApiKeys ? Icons.expand_less : Icons.expand_more,
                        size: 12,
                        color: AppColors.textTertiary,
                      ),
                      const SizedBox(width: 4),
                      const Text(
                        'API KEYS',
                        style: TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                          color: AppColors.textTertiary,
                          letterSpacing: 1,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              if (_showApiKeys) ...[
                const SizedBox(height: 12),
                SettingsCard(children: [
                  ApiKeyField(
                    label: 'TMDb API Key',
                    value: config.tmdbApiKey,
                    hint: 'Enter TMDb API key',
                    helpText: 'Used for movie/TV metadata and posters. Get one free at themoviedb.org',
                    reveal: _revealKeys['tmdb'] ?? false,
                    onToggleReveal: () => setState(() =>
                        _revealKeys['tmdb'] = !(_revealKeys['tmdb'] ?? false)),
                    onChanged: (v) => _updateConfig((c) => _copyConfig(c, tmdbApiKey: v)),
                  ),
                  const Divider(height: 24),
                  ApiKeyField(
                    label: 'OpenSubtitles API Key',
                    value: config.opensubsApiKey,
                    hint: 'Enter OpenSubtitles API key',
                    helpText: 'Used for auto-downloading subtitles. Get one free at opensubtitles.com',
                    reveal: _revealKeys['opensubs'] ?? false,
                    onToggleReveal: () => setState(() =>
                        _revealKeys['opensubs'] = !(_revealKeys['opensubs'] ?? false)),
                    onChanged: (v) => _updateConfig((c) => _copyConfig(c, opensubsApiKey: v)),
                  ),
                ]),
              ],

              const SizedBox(height: 24),

              // --- About section ---
              const SectionHeader('About'),
              SettingsCard(children: [
                SettingRow(
                  label: 'Version',
                  trailing: Text(
                    _appVersion.isEmpty ? '...' : _appVersion,
                    style: const TextStyle(fontSize: 12, color: AppColors.textQuaternary),
                  ),
                ),
                const SizedBox(height: 8),
                SettingRow(
                  label: 'AI Classifier',
                  trailing: Text(
                    _aiError ?? (_aiReady ? 'Ready' : 'Initializing...'),
                    style: TextStyle(
                      fontSize: 12,
                      color: _aiError != null
                          ? AppColors.error
                          : _aiReady
                              ? AppColors.success
                              : AppColors.textQuaternary,
                    ),
                  ),
                ),
              ]),

              const SizedBox(height: 64),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _pickLibraryFolder() async {
    final result = await FilePicker.platform.getDirectoryPath(
      dialogTitle: 'Choose library folder',
    );
    if (result != null) {
      _updateConfig((c) => _copyConfig(c, libraryPath: result));
    }
  }

  Future<void> _pickWatchFolder() async {
    final result = await FilePicker.platform.getDirectoryPath(
      dialogTitle: 'Choose watch folder',
    );
    if (result != null) {
      _updateConfig((c) => _copyConfig(c, watchPath: result));
    }
  }

  void _toggleLanguage(String code, Config config) {
    final langs = List<String>.from(config.subtitleLanguages);
    if (langs.contains(code)) {
      langs.remove(code);
    } else {
      langs.add(code);
    }
    _updateConfig((c) => _copyConfig(c, subtitleLanguages: langs));
  }

  void _updateQbit(String field, dynamic value, Config config) {
    final q = config.qbittorrent;
    final newQbit = switch (field) {
      'host' => QbitConfig(host: value as String, port: q.port, username: q.username, password: q.password, autoRemove: q.autoRemove),
      'port' => QbitConfig(host: q.host, port: value as int, username: q.username, password: q.password, autoRemove: q.autoRemove),
      'username' => QbitConfig(host: q.host, port: q.port, username: value as String, password: q.password, autoRemove: q.autoRemove),
      'password' => QbitConfig(host: q.host, port: q.port, username: q.username, password: value as String, autoRemove: q.autoRemove),
      _ => q,
    };
    _updateConfig((c) => _copyConfig(c, qbittorrent: newQbit));
  }

  void _clearOverrideLater() {
    _overrideClearTimer?.cancel();
    _overrideClearTimer = Timer(const Duration(seconds: 5), () {
      if (mounted) setState(() => _qbitStatusOverride = null);
    });
  }

  Future<void> _autoDetectQbit() async {
    setState(() => _qbitStatusOverride = 'Searching...');
    try {
      final detected = await qbit_api.autoDetectQbittorrent();
      await ref.read(configProvider.notifier).updateConfig(
        (c) => _copyConfig(c, qbittorrent: detected),
        immediate: true,
      );
      final result = await qbit_api.testQbittorrent(qbitConfig: detected);
      if (mounted) {
        setState(() => _qbitStatusOverride =
            result.connected ? 'Connected (v${result.version})' : result.message);
        _clearOverrideLater();
      }
    } catch (e) {
      if (mounted) {
        setState(() => _qbitStatusOverride = 'Not found');
        _clearOverrideLater();
      }
    }
  }

  Future<void> _testQbit(Config config) async {
    setState(() => _qbitStatusOverride = 'Testing...');
    try {
      final result =
          await qbit_api.testQbittorrent(qbitConfig: config.qbittorrent);
      if (mounted) {
        setState(() => _qbitStatusOverride =
            result.connected ? 'Connected (v${result.version})' : result.message);
        _clearOverrideLater();
      }
    } catch (e) {
      if (mounted) {
        setState(() => _qbitStatusOverride = 'Error: $e');
        _clearOverrideLater();
      }
    }
  }

  Future<void> _handleRescan() async {
    _rescanClearTimer?.cancel();
    setState(() { _rescanning = true; _rescanStatus = 'Starting...'; });
    try {
      _rescanSub?.cancel();
      _rescanSub = pipeline_api.rescanLibrary().listen(
        (event) {
          if (event.startsWith('DONE:')) {
            if (mounted) {
              setState(() { _rescanning = false; _rescanStatus = 'Done'; });
              _rescanClearTimer?.cancel();
              _rescanClearTimer = Timer(const Duration(seconds: 5), () {
                if (mounted) setState(() => _rescanStatus = '');
              });
            }
          } else if (mounted) {
            setState(() => _rescanStatus = event);
          }
        },
        onError: (e) {
          if (mounted) {
            setState(() { _rescanning = false; _rescanStatus = 'Error: $e'; });
          }
        },
      );
    } catch (e) {
      if (mounted) {
        setState(() { _rescanning = false; _rescanStatus = 'Error: $e'; });
      }
    }
  }
}
