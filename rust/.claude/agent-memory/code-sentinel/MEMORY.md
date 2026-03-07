# Code Sentinel Memory -- Reel Rust Codebase

## Architecture

- **Pure Rust lib** (no framework deps) exposed to Flutter via `flutter_rust_bridge`
- FFI boundary uses `Result<T, String>` (acceptable); internal code uses `AppResult<T>` from `error.rs`
- DB: single `Mutex<Option<Connection>>` with `with_connection` helper pattern
- Shared utilities: `shared/cache.rs` (TypedCache), `shared/rate_limiter.rs` (RateLimiter), `shared/http.rs` (HTTP client), `shared/video.rs` (file detection)
- Re-export layers: `transaction.rs` re-exports from `db/*` for FFI compat

## Known Technical Debt

- `metadata.rs` is the OLD TMDb client (inline cache/rate-limiter/HTTP). `identify/tmdb.rs` is the NEW one using shared utilities. Migration pending -- both exist in parallel. See audit-2026-03-06.md for details.
- `subtitles.rs` contains OS hash computation + ffprobe logic that is duplicated in `identify/hash.rs` and `identify/probe.rs`. Consolidation needed.
- TMDb poster URL `https://image.tmdb.org/t/p/w500` is a magic string in 8+ locations. Needs const.
- `watch_progress.rs` uses `u32` for season/episode; `transactions.rs` uses `u16`. Inconsistent.
- `get_continue_watching` loads all rows then filters in-memory (N+1 query pattern).

## Patterns to Enforce

- New code MUST use `AppResult<T>` / `AppError`, never `Result<T, String>`
- Never `.ok()` without logging -- use `if let Err(e) = ... { log::warn!(...) }` or `match`
- Max 4 function params -- use structs for more
- Max 3 levels of nesting -- use early returns
- FFI API structs MUST derive Serialize (for flutter_rust_bridge)
- No hardcoded version strings -- use `env!("CARGO_PKG_VERSION")`

## Recurring Agent Mistakes

- Creating new files without wiring them (no mod.rs, no lib.rs declaration). ALWAYS verify module tree.
- Copy-pasting existing code instead of extracting to shared module and importing.
- Forgetting to add Serialize/Debug derives on FFI-crossing structs.

## File Details (see audit-2026-03-06.md for full report)
