import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:reel/src/rust/pipeline.dart';

/// Represents a page in the navigation stack.
sealed class AppPage {
  const AppPage();
}

class FormatsPage extends AppPage {
  const FormatsPage();
}

class GenreListPage extends AppPage {
  final String formatName;
  final String formatPath;
  const GenreListPage({required this.formatName, required this.formatPath});
}

class MediaListPage extends AppPage {
  final String formatName;
  final String genreName;
  final String genrePath;
  const MediaListPage({
    required this.formatName,
    required this.genreName,
    required this.genrePath,
  });
}

class MediaDetailAppPage extends AppPage {
  final MediaInfo media;
  const MediaDetailAppPage({required this.media});
}

class SettingsAppPage extends AppPage {
  const SettingsAppPage();
}

class HistoryAppPage extends AppPage {
  const HistoryAppPage();
}

class ReviewAppPage extends AppPage {
  final List<String> batchIds;
  const ReviewAppPage({required this.batchIds});
}

class PlayerAppPage extends AppPage {
  final MediaDetail detail;
  final MediaFile file;
  final List<MediaFile> playlist;
  final int startIndex;
  const PlayerAppPage({
    required this.detail,
    required this.file,
    required this.playlist,
    required this.startIndex,
  });
}

/// Navigation state: a stack of pages.
class NavigationNotifier extends Notifier<List<AppPage>> {
  @override
  List<AppPage> build() {
    return [const FormatsPage()];
  }

  AppPage get current => state.last;

  void push(AppPage page) {
    state = [...state, page];
  }

  void pop() {
    if (state.length > 1) {
      state = state.sublist(0, state.length - 1);
    }
  }

  void popToRoot() {
    state = [const FormatsPage()];
  }

  void goToSettings() {
    push(const SettingsAppPage());
  }

  void goToHistory() {
    push(const HistoryAppPage());
  }

  void goToReview(List<String> batchIds) {
    push(ReviewAppPage(batchIds: batchIds));
  }

  void goToPlayer(MediaDetail detail, MediaFile file, List<MediaFile> playlist, int index) {
    push(PlayerAppPage(detail: detail, file: file, playlist: playlist, startIndex: index));
  }

  void goToGenres(FormatInfo format) {
    push(GenreListPage(formatName: format.name, formatPath: format.path));
  }

  void goToMedia({
    required String formatName,
    required String genreName,
    required String genrePath,
  }) {
    push(MediaListPage(
      formatName: formatName,
      genreName: genreName,
      genrePath: genrePath,
    ));
  }

  void goToMediaDetail(MediaInfo media) {
    push(MediaDetailAppPage(media: media));
  }

  /// Get the title for the current page.
  String get title {
    final page = current;
    return switch (page) {
      FormatsPage() => 'Reel',
      GenreListPage() => page.formatName,
      MediaListPage() => '${page.formatName} > ${page.genreName}',
      MediaDetailAppPage() => page.media.title,
      SettingsAppPage() => 'Settings',
      HistoryAppPage() => 'History',
      ReviewAppPage() => 'Review',
      PlayerAppPage() => page.detail.title,
    };
  }

  bool get canGoBack => state.length > 1;
}

final navigationProvider =
    NotifierProvider<NavigationNotifier, List<AppPage>>(NavigationNotifier.new);
