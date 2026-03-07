import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/pipeline.dart';
import 'package:reel/src/rust/api/library_api.dart' as library_api;
import 'package:reel/providers/config_provider.dart';

/// Library state: format list + recently added items.
class LibraryState {
  final List<FormatInfo> formats;
  final List<MediaInfo> recentlyAdded;
  final bool loading;

  const LibraryState({
    this.formats = const [],
    this.recentlyAdded = const [],
    this.loading = true,
  });

  LibraryState copyWith({
    List<FormatInfo>? formats,
    List<MediaInfo>? recentlyAdded,
    bool? loading,
  }) {
    return LibraryState(
      formats: formats ?? this.formats,
      recentlyAdded: recentlyAdded ?? this.recentlyAdded,
      loading: loading ?? this.loading,
    );
  }
}

class LibraryNotifier extends Notifier<LibraryState> {
  @override
  LibraryState build() {
    // Watch config to auto-refresh when library path changes.
    final config = ref.watch(configProvider).value;
    if (config?.libraryPath != null) {
      _loadLibrary(config!.libraryPath!);
    }
    return const LibraryState();
  }

  Future<void> _loadLibrary(String libraryPath) async {
    state = state.copyWith(loading: true);
    try {
      final results = await Future.wait([
        library_api.getLibraryContents(libraryPath: libraryPath),
        library_api.getRecentlyAdded(libraryPath: libraryPath, limit: 20),
      ]);
      state = LibraryState(
        formats: results[0] as List<FormatInfo>,
        recentlyAdded: results[1] as List<MediaInfo>,
        loading: false,
      );
    } catch (e) {
      state = const LibraryState(loading: false);
    }
  }

  /// Force refresh the library data.
  Future<void> refresh() async {
    final config = ref.read(configProvider).value;
    if (config?.libraryPath != null) {
      await _loadLibrary(config!.libraryPath!);
    }
  }
}

final libraryProvider =
    NotifierProvider<LibraryNotifier, LibraryState>(LibraryNotifier.new);
