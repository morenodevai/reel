import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:media_kit/media_kit.dart';
import 'package:reel/src/rust/frb_generated.dart';
import 'package:reel/src/rust/api/init_api.dart' as init_api;
import 'package:reel/src/rust/api/ai_api.dart' as ai_api;
import 'package:reel/theme/app_theme.dart';
import 'package:reel/providers/navigation_provider.dart';
import 'package:reel/providers/config_provider.dart';
import 'package:reel/providers/qbit_provider.dart';
import 'package:reel/components/title_bar.dart';
import 'package:reel/components/toast_overlay.dart';
import 'package:reel/pages/formats_page.dart';
import 'package:reel/pages/genre_page.dart';
import 'package:reel/pages/media_page.dart';
import 'package:reel/pages/settings_page.dart';
import 'package:reel/pages/history_page.dart';
import 'package:reel/pages/review_page.dart';
import 'package:reel/pages/media_detail_page.dart';
import 'package:reel/pages/player_page.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    MediaKit.ensureInitialized();
  } catch (e) {
    debugPrint('MediaKit init failed: $e');
  }

  // Initialize the Rust bridge
  await RustLib.init();

  // Initialize the database
  try {
    await init_api.initDb();
  } catch (e) {
    debugPrint('DB init failed: $e');
  }

  // Initialize AI classifier in background (loads 668MB model, non-blocking)
  ai_api.initAi().catchError((e) {
    debugPrint('AI classifier init failed: $e');
  });

  // Configure window
  await windowManager.ensureInitialized();
  const windowOptions = WindowOptions(
    size: Size(1280, 800),
    minimumSize: Size(900, 600),
    center: true,
    title: 'Reel',
    backgroundColor: Colors.transparent,
    titleBarStyle: TitleBarStyle.hidden,
  );
  await windowManager.waitUntilReadyToShow(windowOptions, () async {
    await windowManager.show();
    await windowManager.focus();
  });

  runApp(const ProviderScope(child: ReelApp()));
}

class ReelApp extends StatelessWidget {
  const ReelApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Reel',
      debugShowCheckedModeBanner: false,
      theme: buildAppTheme(),
      home: const AppShell(),
    );
  }
}

/// Root shell: title bar at top, page content below, toast overlay floating.
class AppShell extends ConsumerStatefulWidget {
  const AppShell({super.key});

  @override
  ConsumerState<AppShell> createState() => _AppShellState();
}

class _AppShellState extends ConsumerState<AppShell> {
  int _previousStackDepth = 1;

  @override
  Widget build(BuildContext context) {
    // Watch navigation to rebuild on page changes
    final pages = ref.watch(navigationProvider);
    final currentPage = pages.last;
    final stackDepth = pages.length;

    // Determine transition direction: forward (push) or backward (pop)
    final isForward = stackDepth >= _previousStackDepth;

    // Schedule the depth update for after the build
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _previousStackDepth = stackDepth;
    });

    // Eagerly load config so it's available everywhere
    ref.watch(configProvider);

    // Keep qBit poller alive — auto-starts/stops based on config
    ref.watch(qbitProvider);

    final isPlayer = currentPage is PlayerAppPage;

    return Scaffold(
      body: Stack(
        children: [
          Column(
            children: [
              // Custom title bar (hidden during video playback)
              if (!isPlayer) const TitleBar(),

              // Page content with animated transitions
              Expanded(
                child: AnimatedSwitcher(
                  duration: const Duration(milliseconds: 200),
                  switchInCurve: Curves.easeOut,
                  switchOutCurve: Curves.easeIn,
                  transitionBuilder: (child, animation) {
                    // Fade transition combined with subtle horizontal slide
                    final slideOffset = isForward
                        ? Tween<Offset>(
                            begin: const Offset(0.03, 0),
                            end: Offset.zero,
                          )
                        : Tween<Offset>(
                            begin: const Offset(-0.03, 0),
                            end: Offset.zero,
                          );
                    return SlideTransition(
                      position: slideOffset.animate(animation),
                      child: FadeTransition(
                        opacity: animation,
                        child: child,
                      ),
                    );
                  },
                  child: KeyedSubtree(
                    key: ValueKey(_pageKey(currentPage)),
                    child: _buildPage(currentPage),
                  ),
                ),
              ),
            ],
          ),

          // Toast overlay
          const ToastOverlay(),
        ],
      ),
    );
  }

  /// Unique key per page instance for AnimatedSwitcher.
  String _pageKey(AppPage page) {
    return switch (page) {
      FormatsPage() => 'formats',
      GenreListPage() => 'genre:${page.formatPath}',
      MediaListPage() => 'media:${page.genrePath}',
      MediaDetailAppPage() => 'detail:${page.media.path}',
      SettingsAppPage() => 'settings',
      HistoryAppPage() => 'history',
      ReviewAppPage() => 'review:${page.batchIds.join(",")}',
      PlayerAppPage() => 'player:${page.file.path}',
    };
  }

  Widget _buildPage(AppPage page) {
    return switch (page) {
      FormatsPage() => const FormatsPageWidget(),
      GenreListPage() => GenrePageWidget(
          formatName: page.formatName,
          formatPath: page.formatPath,
        ),
      MediaListPage() => MediaPageWidget(genrePath: page.genrePath),
      MediaDetailAppPage() => MediaDetailPageWidget(media: page.media),
      SettingsAppPage() => const SettingsPageWidget(),
      HistoryAppPage() => const HistoryPageWidget(),
      ReviewAppPage() => ReviewPageWidget(batchIds: page.batchIds),
      PlayerAppPage() => PlayerPageWidget(
          detail: page.detail,
          file: page.file,
          playlist: page.playlist,
          startIndex: page.startIndex,
        ),
    };
  }
}
