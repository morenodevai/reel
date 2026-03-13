import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path/path.dart' as p;
import 'package:reel/src/rust/config.dart';
import 'package:reel/src/rust/api/ai_api.dart' as ai_api;
import 'package:reel/src/rust/api/qbit_api.dart' as qbit_api;
import 'package:reel/src/rust/api/pipeline_api.dart' as pipeline_api;
import 'package:reel/src/rust/api/config_api.dart' as config_api;
import 'package:reel/providers/config_provider.dart';
import 'package:reel/providers/toast_provider.dart';
import 'package:reel/providers/qbit_provider.dart';
import 'package:reel/core/updater/update_provider.dart';
import 'package:reel/pages/settings_widgets.dart';
import 'package:reel/theme/app_theme.dart';
import 'package:reel/utils/config_copy.dart';

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
      if (mounted) {
        setState(() => _aiReady = ready);
        if (ready) {
          _aiPollTimer?.cancel();
          _aiPollTimer = null;
        }
      }
    } catch (e) {
      if (mounted) {
        setState(() => _aiError = 'Check failed: $e');
        _aiPollTimer?.cancel();
        _aiPollTimer = null;
      }
    }
  }

  void _updateConfig(Config Function(Config c) updater) {
    ref.read(configProvider.notifier).updateConfig(updater);
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
                    onChanged: (v) => _updateConfig((c) => copyConfig(c, watcherEnabled: v)),
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
                    onChanged: (v) => _updateConfig((c) => copyConfig(c, autoDownloadSubs: v)),
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
                      (c) => copyConfig(c, qbitEnabled: v),
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
                      onUpdate: (qbit) => _updateConfig((c) => copyConfig(c, qbittorrent: qbit)),
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
                        _updateConfig((c) => copyConfig(c, qbittorrent: newQbit));
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
                    onChanged: (v) => _updateConfig((c) => copyConfig(c, tmdbApiKey: v)),
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
                    onChanged: (v) => _updateConfig((c) => copyConfig(c, opensubsApiKey: v)),
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
                const Divider(height: 24),
                SettingRow(
                  label: 'Logs',
                  subtitle: _logDir,
                  trailing: SmallButton(
                    icon: Icons.folder_open_outlined,
                    label: 'Open',
                    onTap: _openLogFolder,
                  ),
                ),
              ]),

              const SizedBox(height: 24),

              // --- Updates section ---
              const SectionHeader('Updates'),
              _UpdateSection(appVersion: _appVersion),

              const SizedBox(height: 64),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _pickLibraryFolder() async {
    final result = await FilePicker.platform.getDirectoryPath(
      dialogTitle: 'Choose where to create your Reel library',
    );
    if (result != null) {
      try {
        final libraryRoot = await config_api.ensureLibraryRoot(parent: result);
        _updateConfig((c) => copyConfig(c, libraryPath: libraryRoot));
      } catch (e) {
        if (mounted) {
          ref.read(toastProvider.notifier).show(
            'Failed to create library: $e',
            type: ToastType.error,
          );
        }
      }
    }
  }

  Future<void> _pickWatchFolder() async {
    final result = await FilePicker.platform.getDirectoryPath(
      dialogTitle: 'Choose watch folder',
    );
    if (result != null) {
      _updateConfig((c) => copyConfig(c, watchPath: result));
    }
  }

  void _toggleLanguage(String code, Config config) {
    final langs = List<String>.from(config.subtitleLanguages);
    if (langs.contains(code)) {
      langs.remove(code);
    } else {
      langs.add(code);
    }
    _updateConfig((c) => copyConfig(c, subtitleLanguages: langs));
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
        (c) => copyConfig(c, qbittorrent: detected),
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

  String get _logDir {
    final appData = Platform.environment['APPDATA'] ?? '';
    return appData.isEmpty ? '' : p.join(appData, 'Reel');
  }

  void _openLogFolder() {
    final dir = _logDir;
    if (dir.isEmpty) return;
    pipeline_api.revealInFinder(path: dir).catchError((e) {
      debugPrint('[settings] Failed to open log folder: $e');
    });
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

// ---------------------------------------------------------------------------
// Update Section
// ---------------------------------------------------------------------------

class _UpdateSection extends ConsumerStatefulWidget {
  final String appVersion;
  const _UpdateSection({required this.appVersion});

  @override
  ConsumerState<_UpdateSection> createState() => _UpdateSectionState();
}

class _UpdateSectionState extends ConsumerState<_UpdateSection> {
  bool _showUrlField = false;
  late TextEditingController _urlController;

  @override
  void initState() {
    super.initState();
    _urlController = TextEditingController();
  }

  @override
  void dispose() {
    _urlController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final update = ref.watch(updateProvider);
    final notifier = ref.read(updateProvider.notifier);

    // Sync URL controller with state
    if (_urlController.text != update.manifestUrl && !_showUrlField) {
      _urlController.text = update.manifestUrl;
    }

    return SettingsCard(children: [
      // Update URL configuration
      SettingRow(
        label: 'Update URL',
        subtitle: update.manifestUrl.isEmpty ? 'Not configured' : null,
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (update.manifestUrl.isNotEmpty && !_showUrlField)
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: Text(
                  Uri.tryParse(update.manifestUrl)?.host ?? update.manifestUrl,
                  style: const TextStyle(fontSize: 11, color: AppColors.textQuaternary),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            SmallButton(
              icon: _showUrlField ? Icons.check : Icons.edit_outlined,
              label: _showUrlField ? 'Save' : 'Edit',
              onTap: () {
                if (_showUrlField) {
                  notifier.setManifestUrl(_urlController.text.trim());
                }
                setState(() => _showUrlField = !_showUrlField);
              },
            ),
          ],
        ),
      ),
      if (_showUrlField) ...[
        const SizedBox(height: 8),
        TextField(
          controller: _urlController,
          style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
          decoration: InputDecoration(
            hintText: 'https://example.com/latest.json',
            hintStyle: const TextStyle(fontSize: 12, color: AppColors.textQuaternary),
            isDense: true,
            contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            filled: true,
            fillColor: AppColors.surface,
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: const BorderSide(color: AppColors.border),
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: const BorderSide(color: AppColors.border),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
              borderSide: const BorderSide(color: AppColors.primary),
            ),
          ),
          onSubmitted: (v) {
            notifier.setManifestUrl(v.trim());
            setState(() => _showUrlField = false);
          },
        ),
      ],

      const Divider(height: 24),

      // Update status
      _buildUpdateStatus(update, notifier),
    ]);
  }

  Widget _buildUpdateStatus(UpdateState update, UpdateNotifier notifier) {
    switch (update.status) {
      case UpdateStatus.idle:
        return SettingRow(
          label: 'Up to date',
          subtitle: widget.appVersion.isNotEmpty ? 'v${widget.appVersion}' : null,
          trailing: SmallButton(
            icon: Icons.refresh,
            label: 'Check',
            onTap: update.manifestUrl.isNotEmpty ? () => notifier.checkForUpdate() : null,
          ),
        );

      case UpdateStatus.checking:
        return const SettingRow(
          label: 'Checking for updates...',
          trailing: SizedBox(
            width: 16, height: 16,
            child: CircularProgressIndicator(strokeWidth: 2, color: AppColors.textQuaternary),
          ),
        );

      case UpdateStatus.available:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SettingRow(
              label: 'Update available: v${update.info!.version}',
              trailing: SmallButton(
                icon: Icons.download,
                label: 'Download',
                onTap: () => notifier.downloadAndVerify(),
              ),
            ),
            if (update.info!.releaseNotes.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  update.info!.releaseNotes,
                  style: const TextStyle(fontSize: 10, color: AppColors.textQuaternary),
                  maxLines: 3,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
        );

      case UpdateStatus.downloading:
        final pct = (update.downloadProgress * 100).toInt();
        return SettingRow(
          label: 'Downloading... $pct%',
          trailing: SizedBox(
            width: 100,
            child: LinearProgressIndicator(
              value: update.downloadProgress,
              backgroundColor: AppColors.surface,
              color: AppColors.primary,
              minHeight: 4,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
        );

      case UpdateStatus.verifying:
        return const SettingRow(
          label: 'Verifying integrity...',
          trailing: SizedBox(
            width: 16, height: 16,
            child: CircularProgressIndicator(strokeWidth: 2, color: AppColors.primary),
          ),
        );

      case UpdateStatus.readyToInstall:
        return SettingRow(
          label: 'Ready to install v${update.info!.version}',
          trailing: SmallButton(
            icon: Icons.install_desktop,
            label: 'Install & Restart',
            onTap: () => notifier.installUpdate(),
          ),
        );

      case UpdateStatus.error:
        return SettingRow(
          label: update.error ?? 'Update failed',
          trailing: SmallButton(
            icon: Icons.refresh,
            label: 'Retry',
            onTap: () => notifier.checkForUpdate(),
          ),
        );
    }
  }
}
