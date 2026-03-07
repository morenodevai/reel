# KGB Agent Memory -- Reel Flutter App

## Project Structure
- Flutter project: `/Users/moreno/Desktop/media-sort/flutter-app/`
- Flutter binary: `/Users/moreno/flutter/3.38.7/bin/flutter`
- Build: `/Users/moreno/flutter/3.38.7/bin/flutter build macos --debug`
- Built app: `build/macos/Build/Products/Debug/reel.app`
- Rust crate: `rust/src/` (flutter_rust_bridge FFI)
- Migration plan: `/Users/moreno/Desktop/media-sort/FLUTTER_MIGRATION_PLAN.md`
- Tauri reference: `/Users/moreno/Desktop/media-sort/tauri-app/`

## Architecture Patterns
- **State**: Riverpod NotifierProvider for navigation, config, library, toast
- **Navigation**: Custom stack-based via `NavigationNotifier` (sealed class `AppPage`)
- **FFI**: `flutter_rust_bridge` auto-generates Dart wrappers in `lib/src/rust/api/`
- **Theme**: `AppColors` constants in `lib/theme/app_theme.dart` -- dark Netflix aesthetic
- **Pages**: StatefulWidget or ConsumerStatefulWidget, each in `lib/pages/`
- **Components**: Reusable in `lib/components/` (MediaCard, FormatCard, GenreRow, etc.)

## Key Types (from Rust via FRB)
- `MediaInfo` -- title, year, path, posterUrl, tmdbId, format, genre
- `MediaDetail` -- extends MediaInfo with files, seasonCount, episodeCount
- `MediaFile` -- path, filename, season, episode, episodeTitle, sizeBytes, hasSubtitles
- `Transaction` -- full processing record with confidence, batch, undo support
- `FormatInfo` -- name, path, genreCount, mediaCount, posterSamples
- `GenreInfo` -- name, path, mediaCount, mediaSamples

## Phase Completion
- Phase 1: Core pipeline + library UI (COMPLETE)
- Phase 2: Media detail page + nav wiring + page transitions (COMPLETE)
  - `MediaDetailAppPage` added to navigation sealed class
  - `media_detail_page.dart` -- hero poster, metadata, season selector, episode list
  - All media card taps wired to navigate to detail
  - AnimatedSwitcher page transitions with directional slide (forward=right, back=left)
  - MediaPageWidget converted from StatefulWidget to ConsumerStatefulWidget for ref access

## Build Notes
- Build succeeds cleanly with no warnings as of Phase 2 completion
- `dart:ui` imported for `FontFeature.tabularFigures()` in episode rows
- `dart:math` used for `_formatBytes()` helper (log/pow for byte formatting)
