import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/config.dart';
import 'package:reel/src/rust/api/config_api.dart' as config_api;

/// Global config state: loads on startup, saves on mutation.
class ConfigNotifier extends AsyncNotifier<Config?> {
  Timer? _saveDebounce;

  @override
  Future<Config?> build() async {
    ref.onDispose(() => _saveDebounce?.cancel());
    try {
      return await config_api.loadConfig();
    } catch (e) {
      debugPrint('[config] Failed to load config: $e');
      return null;
    }
  }

  /// Reload config from disk.
  Future<void> reload() async {
    state = const AsyncValue.loading();
    try {
      state = AsyncValue.data(await config_api.loadConfig());
    } catch (e) {
      state = AsyncValue.error(e, StackTrace.current);
    }
  }

  /// Update a single field via the updater and debounce-save.
  Future<void> updateConfig(Config Function(Config current) updater, {bool immediate = false}) async {
    final current = state.value;
    if (current == null) return;
    final updated = updater(current);
    state = AsyncValue.data(updated);
    if (immediate) {
      _saveDebounce?.cancel();
      try {
        await config_api.saveConfig(cfg: updated);
      } catch (e) {
        debugPrint('[config] Immediate save failed: $e');
        try {
          state = AsyncValue.data(await config_api.loadConfig());
        } catch (e2) {
          debugPrint('[config] Fallback reload also failed: $e2');
          state = AsyncValue.error(e, StackTrace.current);
        }
        return;
      }
    } else {
      _debounceSave(updated);
    }
  }

  /// Immediately persist config to disk.
  Future<void> save(Config cfg) async {
    try {
      await config_api.saveConfig(cfg: cfg);
      state = AsyncValue.data(cfg);
    } catch (e) {
      debugPrint('[config] Save failed: $e');
      try {
        state = AsyncValue.data(await config_api.loadConfig());
      } catch (e2) {
        debugPrint('[config] Fallback reload also failed: $e2');
        state = AsyncValue.error(e, StackTrace.current);
      }
    }
  }

  void _debounceSave(Config cfg) {
    _saveDebounce?.cancel();
    _saveDebounce = Timer(const Duration(milliseconds: 600), () async {
      try {
        await config_api.saveConfig(cfg: cfg);
      } catch (e) {
        debugPrint('[config] Debounced save failed: $e');
        try {
          state = AsyncValue.data(await config_api.loadConfig());
        } catch (e2) {
          debugPrint('[config] Fallback reload also failed: $e2');
        }
      }
    });
  }
}

final configProvider =
    AsyncNotifierProvider<ConfigNotifier, Config?>(ConfigNotifier.new);
