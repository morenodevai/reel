import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path/path.dart' as p;
import 'package:reel/core/updater/update_service.dart';

const _kCheckIntervalMinutes = 30;

/// Global update provider.
final updateProvider = NotifierProvider<UpdateNotifier, UpdateState>(UpdateNotifier.new);

/// Persistent update config stored outside the Rust config to avoid FRB codegen.
/// Lives at %APPDATA%/Reel/update-config.json.
class _UpdateConfig {
  final String manifestUrl;
  const _UpdateConfig({this.manifestUrl = ''});

  static String get _path {
    final appData = Platform.environment['APPDATA'] ?? '.';
    return p.join(appData, 'Reel', 'update-config.json');
  }

  static Future<_UpdateConfig> load() async {
    try {
      final file = File(_path);
      if (!await file.exists()) return const _UpdateConfig();
      final json = jsonDecode(await file.readAsString()) as Map<String, dynamic>;
      return _UpdateConfig(manifestUrl: json['manifest_url'] as String? ?? '');
    } catch (e) {
      debugPrint('[updater] Failed to load update config: $e');
      return const _UpdateConfig();
    }
  }

  Future<void> save() async {
    final file = File(_path);
    await file.writeAsString(jsonEncode({'manifest_url': manifestUrl}));
  }
}

enum UpdateStatus { idle, checking, available, downloading, verifying, readyToInstall, error }

class UpdateState {
  final UpdateStatus status;
  final UpdateInfo? info;
  final double downloadProgress;
  final String? installerPath;
  final String? error;
  final String manifestUrl;

  const UpdateState({
    this.status = UpdateStatus.idle,
    this.info,
    this.downloadProgress = 0,
    this.installerPath,
    this.error,
    this.manifestUrl = '',
  });
}

class UpdateNotifier extends Notifier<UpdateState> {
  String _currentVersion = '';
  final UpdateService _service = UpdateService();
  Timer? _checkTimer;
  Timer? _initialCheckTimer;

  @override
  UpdateState build() {
    PackageInfo.fromPlatform().then((info) {
      _currentVersion = info.version;
      _initChecks();
    });

    ref.onDispose(() {
      _checkTimer?.cancel();
      _initialCheckTimer?.cancel();
    });

    return const UpdateState();
  }

  Future<void> _initChecks() async {
    final config = await _UpdateConfig.load();
    state = UpdateState(manifestUrl: config.manifestUrl);

    if (config.manifestUrl.isNotEmpty) {
      _initialCheckTimer = Timer(const Duration(seconds: 5), checkForUpdate);
    }

    _checkTimer = Timer.periodic(const Duration(minutes: _kCheckIntervalMinutes), (_) {
      if (state.manifestUrl.isNotEmpty && state.status == UpdateStatus.idle) {
        checkForUpdate();
      }
    });
  }

  /// Update the manifest URL and persist it.
  Future<void> setManifestUrl(String url) async {
    state = UpdateState(manifestUrl: url);
    await _UpdateConfig(manifestUrl: url).save();
  }

  /// Check for updates against the manifest URL.
  Future<void> checkForUpdate() async {
    if (state.manifestUrl.isEmpty || _currentVersion.isEmpty) return;
    if (state.status == UpdateStatus.checking || state.status == UpdateStatus.downloading) return;

    state = UpdateState(status: UpdateStatus.checking, manifestUrl: state.manifestUrl);
    try {
      final info = await _service.checkForUpdate(state.manifestUrl, _currentVersion);
      if (info != null) {
        state = UpdateState(status: UpdateStatus.available, info: info, manifestUrl: state.manifestUrl);
      } else {
        state = UpdateState(manifestUrl: state.manifestUrl);
      }
    } catch (e) {
      state = UpdateState(status: UpdateStatus.error, error: 'Check failed: $e', manifestUrl: state.manifestUrl);
    }
  }

  /// Download, verify, and prepare the update for installation.
  Future<void> downloadAndVerify() async {
    final info = state.info;
    if (info == null) return;
    final url = state.manifestUrl;

    state = UpdateState(status: UpdateStatus.downloading, info: info, manifestUrl: url);
    Directory? tempDir;
    try {
      tempDir = await Directory.systemTemp.createTemp('reel_update_');
      final file = await _service.downloadUpdate(
        info,
        tempDir.path,
        (progress) {
          state = UpdateState(
            status: UpdateStatus.downloading,
            info: info,
            downloadProgress: progress,
            manifestUrl: url,
          );
        },
        manifestUri: Uri.parse(url),
      );

      state = UpdateState(status: UpdateStatus.verifying, info: info, manifestUrl: url);
      final valid = await _service.verifyUpdate(file, info);
      if (!valid) {
        await tempDir.delete(recursive: true);
        state = UpdateState(status: UpdateStatus.error, error: 'Integrity check failed', manifestUrl: url);
        return;
      }

      state = UpdateState(
        status: UpdateStatus.readyToInstall,
        info: info,
        installerPath: file.path,
        manifestUrl: url,
      );
    } catch (e) {
      debugPrint('[updater] Download failed: $e');
      if (tempDir != null) {
        try { await tempDir.delete(recursive: true); } catch (_) {}
      }
      state = UpdateState(status: UpdateStatus.error, error: 'Download failed: $e', manifestUrl: url);
    }
  }

  /// Launch the installer and exit the app.
  Future<void> installUpdate() async {
    final path = state.installerPath;
    if (path == null) return;
    await _service.applyUpdate(path);
  }

  /// Dismiss an available update (go back to idle).
  void dismiss() {
    state = UpdateState(manifestUrl: state.manifestUrl);
  }
}
