/// File system watcher module.
/// Watches a SEPARATE drop/incoming folder for new video files.
/// When a file appears and stabilizes (done writing), it gets analyzed
/// and moved to the organized library via the classification pipeline.
///
/// IMPORTANT: watch_path must be DIFFERENT from library_path.
/// Watching the library itself would cause an infinite loop
/// (pipeline moves file to library → watcher sees it → processes again).

use crate::config::Config;
use crate::pipeline;
use crate::renamer;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Shared state for the watcher + processing thread.
struct WatcherState {
    watcher: RecommendedWatcher,
    /// Signal the processing thread to stop.
    stop_flag: Arc<AtomicBool>,
}

static RUNNING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static STATE: Lazy<Mutex<Option<WatcherState>>> = Lazy::new(|| Mutex::new(None));

pub fn start_watcher<F>(
    watch_path: &str,
    config: Config,
    emit: F,
) -> Result<(), String>
where
    F: Fn(&str, serde_json::Value) + Send + Sync + 'static,
{
    // Stop any existing watcher first (prevents thread leak)
    if RUNNING.load(Ordering::SeqCst) {
        stop_watcher()?;
    }

    let watch_dir = PathBuf::from(watch_path);
    if !watch_dir.exists() {
        return Err("Watch folder does not exist".to_string());
    }

    // Safety: watch_path must not be inside library_path or vice versa
    if let Some(ref lib) = config.library_path {
        let lib_path = PathBuf::from(lib);
        if watch_dir.starts_with(&lib_path) || lib_path.starts_with(&watch_dir) {
            return Err(
                "Watch folder must be separate from your library folder".to_string(),
            );
        }
    }

    // Shared pending map: notify callback adds files, processing thread drains them.
    // Value = retry count (how many times stability check failed).
    let pending: Arc<Mutex<HashMap<PathBuf, u32>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending_for_notify = pending.clone();
    let pending_for_thread = pending.clone();

    // Stop flag: shared between start_watcher and the processing thread
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_for_thread = stop_flag.clone();

    let config_for_thread = config.clone();
    let watch_path_owned = watch_path.to_string();
    let emit = Arc::new(emit);
    let emit_for_thread = emit.clone();

    const MAX_STABILITY_RETRIES: u32 = 20; // ~60s+ of checking before giving up

    // === Processing thread ===
    // Checks pending files every 3 seconds, waits for stability, processes them.
    std::thread::spawn(move || {
        // Track recently processed files to avoid duplicates from repeated FSEvents
        let mut recently_processed: HashMap<PathBuf, Instant> = HashMap::new();

        while !stop_flag_for_thread.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(3));

            if stop_flag_for_thread.load(Ordering::SeqCst) {
                break;
            }

            // Drain pending files with their retry counts
            let files: Vec<(PathBuf, u32)> = {
                if let Ok(mut lock) = pending_for_thread.lock() {
                    lock.drain().collect()
                } else {
                    continue;
                }
            };

            if files.is_empty() {
                // Clean up old entries from recently_processed (older than 60s)
                let cutoff = Instant::now() - Duration::from_secs(60);
                recently_processed.retain(|_, t| *t > cutoff);
                continue;
            }

            let library = match config_for_thread.library_path.as_ref() {
                Some(l) => l.clone(),
                None => continue,
            };

            for (video_path, retries) in files {
                // Skip if we just processed this file
                if let Some(last_time) = recently_processed.get(&video_path) {
                    if last_time.elapsed() < Duration::from_secs(30) {
                        continue;
                    }
                }

                // Skip if file doesn't exist (moved/deleted between event and now)
                if !video_path.exists() {
                    continue;
                }

                // Wait for file to stabilize (stop being written to)
                if !is_file_stable(&video_path) {
                    if retries < MAX_STABILITY_RETRIES {
                        // Put it back in pending with incremented retry count
                        if let Ok(mut lock) = pending_for_thread.lock() {
                            lock.insert(video_path, retries + 1);
                        }
                    } else {
                        log::warn!(
                            "Watcher: giving up on {} after {} stability checks",
                            video_path.display(),
                            retries
                        );
                    }
                    continue;
                }

                // Check stop flag before expensive work
                if stop_flag_for_thread.load(Ordering::SeqCst) {
                    break;
                }

                let filename = video_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                log::info!("Watcher: processing {}", filename);

                let analysis = pipeline::analyze_single_file_pub(
                    &video_path,
                    &filename,
                    &config_for_thread.tmdb_api_key,
                    &config_for_thread.opensubs_api_key,
                    &library,
                );

                let batch_id = uuid::Uuid::new_v4().to_string();
                match pipeline::process_single_file_pub(
                    &analysis,
                    &batch_id,
                    config_for_thread.auto_download_subs,
                    &config_for_thread.subtitle_languages,
                    &config_for_thread.opensubs_api_key,
                    &watch_path_owned,
                ) {
                    Ok(()) => {
                        log::info!("Watcher: organized {} → {}", filename, analysis.dest_path);
                        emit_for_thread("watcher-processed", serde_json::json!(analysis.dest_path));
                    }
                    Err(e) => {
                        log::warn!("Watcher: failed to process {}: {}", filename, e);
                    }
                }

                // Mark as recently processed
                recently_processed.insert(video_path, Instant::now());
            }
        }

        log::info!("Watcher: processing thread exiting");
    });

    // === File system watcher (notify) ===
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in &event.paths {
                        if path.is_file() && renamer::is_video_file(path) {
                            if let Ok(mut lock) = pending_for_notify.lock() {
                                // Only add if not already pending (don't reset retry count)
                                lock.entry(path.clone()).or_insert(0);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    })
    .map_err(|e| format!("Failed to create watcher: {}", e))?;

    watcher
        .watch(Path::new(watch_path), RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch path: {}", e))?;

    // Store state so watcher stays alive and can be stopped cleanly
    if let Ok(mut state) = STATE.lock() {
        *state = Some(WatcherState {
            watcher,
            stop_flag,
        });
    }
    RUNNING.store(true, Ordering::SeqCst);

    log::info!("Watcher: monitoring {} → organizing to library", watch_path);
    Ok(())
}

pub fn stop_watcher() -> Result<(), String> {
    if let Ok(mut state) = STATE.lock() {
        if let Some(s) = state.take() {
            // Signal processing thread to stop
            s.stop_flag.store(true, Ordering::SeqCst);
            // Drop the watcher (stops filesystem monitoring)
            drop(s.watcher);
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
    log::info!("Watcher: stopped");
    Ok(())
}

pub fn is_watcher_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// Check if a file has stopped being written to.
/// Compares size at two points 2 seconds apart.
fn is_file_stable(path: &Path) -> bool {
    let size1 = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return false,
    };
    if size1 == 0 {
        return false;
    }
    std::thread::sleep(Duration::from_secs(2));
    let size2 = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return false,
    };
    size1 == size2 && size2 > 0
}
