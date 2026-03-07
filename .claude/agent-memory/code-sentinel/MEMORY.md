# Code Sentinel Memory - Reel Flutter App

## Project Structure

### Flutter App Location
- Root: `/Users/moreno/Desktop/media-sort/flutter-app/`
- Flutter SDK: `/Users/moreno/flutter/3.38.7/bin/flutter`
- Package name: `reel`

### File Decomposition Pattern (Established March 2026)
- Pages contain state management + orchestration logic
- Widget files (`*_widgets.dart`, `*_controls.dart`) contain pure presentational widgets
- State/data classes extracted to `*_state.dart` files
- Re-export pattern used to preserve existing import paths:
  ```dart
  export 'package:reel/providers/playback_state.dart';
  import 'package:reel/providers/playback_state.dart';
  ```

### Key File Map
- `lib/providers/playback_state.dart` -- PlaybackState, ExternalSubInfo, PlayTarget, resolvePlayTarget
- `lib/providers/playback_provider.dart` -- PlaybackNotifier (re-exports playback_state.dart)
- `lib/pages/player_controls.dart` -- All player UI widgets
- `lib/pages/media_detail_widgets.dart` -- All media detail UI widgets
- `lib/pages/settings_widgets.dart` -- All settings primitive widgets

### Pre-existing Analyzer Warnings
- 91 issues total, ALL in `rust_builder/cargokit/` and test files
- Zero issues in app code
- `media_detail_widgets.dart` has 3 info-level `unnecessary_underscores` from CachedNetworkImage callbacks (standard Dart convention)

### Naming Conventions
- Public widgets: PascalCase, descriptive (e.g., `PlayerProgressBar`, `DetailActionButton`)
- Private widgets: underscore prefix (e.g., `_CenterPlayIcon`, `_SmallButton`)
- Widget files: snake_case matching their parent page relationship
- Theme: `AppColors.textPrimary`, `AppColors.surface`, etc. from `app_theme.dart`

### Provider Pattern
- Uses `flutter_riverpod` with `Notifier<T>` pattern
- State classes are immutable with `copyWith` methods
- Providers defined at file bottom: `final xProvider = NotifierProvider<...>(...)`
