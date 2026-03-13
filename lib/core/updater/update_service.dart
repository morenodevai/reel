import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:flutter/foundation.dart';

/// Metadata about an available update.
class UpdateInfo {
  final String version;
  final String downloadUrl;
  final String sha256;
  final int fileSize;
  final String releaseNotes;

  const UpdateInfo({
    required this.version,
    required this.downloadUrl,
    required this.sha256,
    required this.fileSize,
    this.releaseNotes = '',
  });

  factory UpdateInfo.fromJson(Map<String, dynamic> json) {
    return UpdateInfo(
      version: json['version'] as String,
      downloadUrl: json['download_url'] as String,
      sha256: json['sha256'] as String,
      fileSize: json['file_size'] as int,
      releaseNotes: json['release_notes'] as String? ?? '',
    );
  }
}

/// Handles update check, download, verification, and application.
class UpdateService {
  /// Check for a newer version by fetching the manifest URL.
  /// Returns [UpdateInfo] if a newer version is available, null otherwise.
  Future<UpdateInfo?> checkForUpdate(String manifestUrl, String currentVersion) async {
    if (manifestUrl.isEmpty) return null;

    final client = HttpClient();
    client.connectionTimeout = const Duration(seconds: 10);
    try {
      final request = await client.getUrl(Uri.parse(manifestUrl));
      final response = await request.close();
      if (response.statusCode != 200) return null;

      final body = await response.transform(utf8.decoder).join();
      if (body.length > 10240) return null; // FIX-8: bound manifest size
      final json = jsonDecode(body) as Map<String, dynamic>;
      final info = UpdateInfo.fromJson(json);

      if (_isNewer(info.version, currentVersion)) return info;
      return null;
    } catch (e) {
      debugPrint('[updater] Check failed: $e');
      return null;
    } finally {
      client.close();
    }
  }

  /// Download the update installer to [destDir].
  /// [manifestUri] is used for same-origin validation of the download URL.
  /// Calls [onProgress] with 0.0–1.0 progress.
  Future<File> downloadUpdate(
    UpdateInfo info,
    String destDir,
    void Function(double progress) onProgress, {
    required Uri manifestUri,
  }) async {
    // BLOCK-1: Validate download URL
    final downloadUri = Uri.parse(info.downloadUrl);
    if (downloadUri.scheme != 'http' && downloadUri.scheme != 'https') {
      throw ArgumentError('Invalid download URL scheme: ${downloadUri.scheme}');
    }
    if (downloadUri.host != manifestUri.host) {
      throw ArgumentError(
        'Download host (${downloadUri.host}) does not match manifest host (${manifestUri.host})',
      );
    }

    // FIX-4: Sanitize version in filename
    final safeVersion = info.version.replaceAll(RegExp(r'[^0-9.]'), '');
    if (safeVersion.isEmpty) throw ArgumentError('Invalid version string');
    final fileName = 'Reel_${safeVersion}_x64-setup.exe';

    final client = HttpClient();
    client.connectionTimeout = const Duration(seconds: 30); // FIX-3
    try {
      final request = await client.getUrl(downloadUri);
      final response = await request.close();
      if (response.statusCode != 200) { // FIX-2
        throw HttpException('Download failed: HTTP ${response.statusCode}');
      }

      final file = File('$destDir${Platform.pathSeparator}$fileName');
      final sink = file.openWrite();
      var received = 0;

      await for (final chunk in response) {
        sink.add(chunk);
        received += chunk.length;
        if (info.fileSize > 0) {
          onProgress((received / info.fileSize).clamp(0.0, 1.0));
        }
      }

      await sink.close();
      return file;
    } finally {
      client.close();
    }
  }

  /// Verify the downloaded file's SHA-256 matches the manifest.
  /// Streams the file to avoid loading it entirely into memory.
  Future<bool> verifyUpdate(File file, UpdateInfo info) async {
    final hash = await _fileSha256(file.path);
    final matches = hash == info.sha256;
    if (!matches) {
      debugPrint('[updater] SHA-256 mismatch: expected ${info.sha256}, got $hash');
    }
    return matches;
  }

  /// Launch the installer silently and exit the app.
  /// Inno Setup handles closing the running instance and upgrading in-place.
  Future<void> applyUpdate(String installerPath) async {
    // FIX-5: Validate installer path
    final file = File(installerPath);
    if (!await file.exists()) {
      throw StateError('Installer not found: $installerPath');
    }
    if (!installerPath.endsWith('.exe')) {
      throw StateError('Installer is not an .exe: $installerPath');
    }
    await Process.start(
      installerPath,
      ['/VERYSILENT', '/SUPPRESSMSGBOXES'],
      mode: ProcessStartMode.detached,
    );
    exit(0);
  }

  /// Compare semver strings. Returns true if [latest] is newer than [current].
  /// Rejects non-numeric version segments to avoid silent misbehavior.
  bool _isNewer(String latest, String current) {
    final lParts = latest.split('.').map(int.tryParse).toList();
    final cParts = current.split('.').map(int.tryParse).toList();
    if (lParts.any((p) => p == null) || cParts.any((p) => p == null)) {
      debugPrint('[updater] Non-numeric version segment: latest=$latest, current=$current');
      return false;
    }
    for (var i = 0; i < 3; i++) {
      final l = i < lParts.length ? lParts[i]! : 0;
      final c = i < cParts.length ? cParts[i]! : 0;
      if (l > c) return true;
      if (l < c) return false;
    }
    return false;
  }

  /// Streaming SHA-256 of a file.
  Future<String> _fileSha256(String path) async {
    final sink = _DigestSink();
    final input = sha256.startChunkedConversion(sink);
    await for (final chunk in File(path).openRead()) {
      input.add(chunk);
    }
    input.close();
    return sink.value.toString();
  }
}

/// Minimal sink to capture the final [Digest] from chunked SHA-256.
class _DigestSink implements Sink<Digest> {
  late Digest value;
  @override
  void add(Digest data) => value = data;
  @override
  void close() {}
}
