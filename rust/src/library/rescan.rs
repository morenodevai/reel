/// Library rescan — re-analyze broken files, download missing subs, skip perfect ones.

use crate::config::Config;
use crate::pipeline;
use crate::renamer;
use crate::subtitles;
use crate::transaction;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static RESCAN_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
pub struct RescanProgress {
    pub current: u32,
    pub total: u32,
    pub title: String,
    pub action: String,
    pub fixed: u32,
    pub subs_downloaded: u32,
    pub skipped: u32,
    pub failed: u32,
}

/// Smart library rescan: re-analyze broken files, download missing subs, skip perfect ones.
pub fn rescan_library(config: &Config, mut progress_fn: impl FnMut(&RescanProgress)) -> RescanProgress {
    // Prevent concurrent rescans
    if RESCAN_RUNNING.swap(true, Ordering::SeqCst) {
        log::warn!("[rescan] Rescan already in progress, ignoring duplicate request");
        return RescanProgress {
            current: 0, total: 0, title: String::new(), action: "Already running".into(),
            fixed: 0, subs_downloaded: 0, skipped: 0, failed: 0,
        };
    }
    // Ensure flag is cleared on exit (even on panic)
    struct RescanGuard;
    impl Drop for RescanGuard {
        fn drop(&mut self) {
            RESCAN_RUNNING.store(false, Ordering::SeqCst);
        }
    }
    let _rescan_guard = RescanGuard;

    // Acquire pipeline sync lock to prevent concurrent file moves from watcher/qBit
    let _sync_guard = pipeline::PIPELINE_SYNC_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let library_path = match &config.library_path {
        Some(p) => p.clone(),
        None => {
            log::warn!("[rescan] No library path configured");
            return RescanProgress {
                current: 0, total: 0, title: String::new(), action: "No library configured".into(),
                fixed: 0, subs_downloaded: 0, skipped: 0, failed: 0,
            };
        }
    };
    let library = Path::new(&library_path);
    let api_key = &config.tmdb_api_key;
    let opensubs_key = &config.opensubs_api_key;

    // 1. Collect all title folders
    let mut title_folders: Vec<PathBuf> = Vec::new();
    if let Ok(formats) = fs::read_dir(library) {
        for fe in formats.flatten() {
            if !fe.path().is_dir() { continue; }
            if let Ok(genres) = fs::read_dir(fe.path()) {
                for ge in genres.flatten() {
                    if !ge.path().is_dir() { continue; }
                    if let Ok(titles) = fs::read_dir(ge.path()) {
                        for te in titles.flatten() {
                            if te.path().is_dir() {
                                title_folders.push(te.path());
                            }
                        }
                    }
                }
            }
        }
    }

    let total = title_folders.len() as u32;
    let mut progress = RescanProgress {
        current: 0, total, title: String::new(), action: String::new(),
        fixed: 0, subs_downloaded: 0, skipped: 0, failed: 0,
    };
    log::info!("[rescan] Starting smart rescan: {} title folders", total);

    let batch_id = uuid::Uuid::new_v4().to_string();

    for folder in &title_folders {
        let name = folder.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        progress.current += 1;
        progress.title = name.clone();
        log::info!("[rescan] [{}/{}] Processing: {}", progress.current, total, name);

        if needs_reanalysis(folder) {
            // Skip locked folders — user has explicitly confirmed these
            let videos_for_lock_check = collect_videos_in_title(folder);
            let any_locked = videos_for_lock_check.iter().any(|v| {
                transaction::get_transaction_by_dest(&v.to_string_lossy())
                    .map(|t| t.locked)
                    .unwrap_or(false)
            });
            if any_locked {
                progress.skipped += 1;
                progress.action = "Locked".into();
                progress_fn(&progress);
                continue;
            }

            // === RE-ANALYZE: broken name, uncategorized, or missing tmdbid ===
            progress.action = "Fixing".into();
            progress_fn(&progress);

            let videos = collect_videos_in_title(folder);
            if videos.is_empty() {
                progress.skipped += 1;
                continue;
            }

            // Extract current format from directory structure: Library/{format}/{genre}/{title}/
            let current_format = folder
                .parent()                    // genre dir
                .and_then(|g| g.parent())    // format dir
                .and_then(|f| f.file_name())
                .and_then(|n| n.to_str())
                .filter(|s| crate::formats::is_valid_format(s))
                .map(|s| s.to_string());

            let mut folder_fixed = false;
            for video in &videos {
                let filename = video.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                let analysis = pipeline::analyze_single_file_pub(
                    video, &filename, api_key, opensubs_key, &library_path,
                    current_format.as_deref(),
                );
                let current_path = video.to_string_lossy().to_string();

                if analysis.dest_path == current_path {
                    continue;
                }

                // Move to correct location
                let dest = Path::new(&analysis.dest_path);
                if let Some(parent) = dest.parent() {
                    if fs::create_dir_all(parent).is_err() { progress.failed += 1; continue; }
                }

                if dest.exists() {
                    // Asymmetric defaults (0 vs 1) prevent false-positive match when both stat calls fail
                    let src_size = fs::metadata(video).map(|m| m.len()).unwrap_or(0);
                    let dst_size = fs::metadata(dest).map(|m| m.len()).unwrap_or(1);
                    if src_size == dst_size && pipeline::partial_hash_match(video, dest) {
                        // Confirmed duplicate
                        log::info!(
                            "[rescan] Dedup: confirmed duplicate {} ({}B, hash match with {})",
                            video.display(), src_size, dest.display()
                        );
                        if let Err(e) = fs::remove_file(video) {
                            log::warn!("[rescan] Failed to delete duplicate source {}: {}", video.display(), e);
                            progress.failed += 1;
                            continue;
                        }

                        // Remove co-located subtitle/thumbnail files for the deleted video
                        let video_stem = video.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let src_dir = video.parent().unwrap_or(Path::new("."));
                        if !video_stem.is_empty() {
                            if let Ok(entries) = fs::read_dir(src_dir) {
                                for entry in entries.flatten() {
                                    let ename = entry.file_name().to_string_lossy().to_string();
                                    let ext = Path::new(&ename).extension().and_then(|e| e.to_str()).unwrap_or("");
                                    if matches!(ext, "srt" | "ass" | "ssa" | "sub" | "vtt")
                                        && ename.to_lowercase().starts_with(&video_stem.to_lowercase())
                                    {
                                        if let Err(e) = fs::remove_file(entry.path()) {
                                            log::warn!("[rescan] Failed to delete co-located file {}: {}", entry.path().display(), e);
                                        }
                                    }
                                }
                            }
                        }

                        folder_fixed = true;
                    } else if src_size != dst_size {
                        log::warn!(
                            "[rescan] Dest exists with different size (src={}B dst={}B): {} → {}",
                            src_size, dst_size, video.display(), dest.display()
                        );
                        progress.failed += 1;
                    } else {
                        log::warn!(
                            "[rescan] Dest exists with same size but different content ({}B): {} → {}",
                            src_size, video.display(), dest.display()
                        );
                        progress.failed += 1;
                    }
                    continue;
                }

                let moved = if fs::rename(video, dest).is_ok() {
                    true
                } else {
                    let src_len = match fs::metadata(video) {
                        Ok(m) => m.len(),
                        Err(e) => {
                            log::error!("[rescan] Failed to read source metadata for {}: {}", video.display(), e);
                            progress.failed += 1;
                            continue;
                        }
                    };
                    match fs::copy(video, dest) {
                        Ok(copied) => {
                            if copied == src_len {
                                if let Err(e) = fs::remove_file(video) {
                                    log::warn!("[rescan] Failed to remove source after copy: {}", e);
                                }
                                true
                            } else {
                                log::error!("[rescan] Copy verification failed for {} (src={}B copied={}B)", video.display(), src_len, copied);
                                if let Err(e) = fs::remove_file(dest) {
                                    log::warn!("[rescan] Failed to clean up dest after copy verification failure {}: {}", dest.display(), e);
                                }
                                false
                            }
                        }
                        Err(e) => {
                            log::error!("[rescan] Failed to copy {} → {}: {}", video.display(), dest.display(), e);
                            false
                        }
                    }
                };

                if !moved { progress.failed += 1; continue; }
                folder_fixed = true;

                // Move co-located subtitle files, renaming to match the new video name
                let old_stem = video.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let new_stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let src_dir = video.parent().unwrap_or(Path::new("."));
                let dest_dir = dest.parent().unwrap_or(Path::new("."));
                if let Ok(entries) = fs::read_dir(src_dir) {
                    for entry in entries.flatten() {
                        let epath = entry.path();
                        let ename = entry.file_name().to_string_lossy().to_string();
                        let ext = Path::new(&ename).extension().and_then(|e| e.to_str()).unwrap_or("");
                        if matches!(ext, "srt" | "ass" | "ssa" | "sub" | "vtt")
                            && ename.to_lowercase().starts_with(&old_stem.to_lowercase())
                        {
                            let lang = subtitles::detect_language(&ename);
                            let forced = subtitles::is_forced(&ename);
                            let sdh = subtitles::is_sdh(&ename);
                            let new_sub_name = renamer::subtitle_filename(new_stem, &lang, forced, sdh, ext);
                            let sub_dest = dest_dir.join(&new_sub_name);
                            if fs::rename(&epath, &sub_dest).is_err() {
                                match fs::copy(&epath, &sub_dest) {
                                    Ok(_) => {
                                        if let Err(e) = fs::remove_file(&epath) {
                                            log::warn!("[rescan] Failed to remove source subtitle {}: {}", epath.display(), e);
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("[rescan] Failed to copy subtitle {} → {}: {}", epath.display(), sub_dest.display(), e);
                                    }
                                }
                            }
                        } else if matches!(ext, "jpg" | "png") && ename.to_lowercase().starts_with(&old_stem.to_lowercase()) {
                            let art_dest = dest_dir.join(&ename);
                            if fs::rename(&epath, &art_dest).is_err() {
                                match fs::copy(&epath, &art_dest) {
                                    Ok(_) => {
                                        if let Err(e) = fs::remove_file(&epath) {
                                            log::warn!("[rescan] Failed to remove source artwork {}: {}", epath.display(), e);
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("[rescan] Failed to copy artwork {} → {}: {}", epath.display(), art_dest.display(), e);
                                    }
                                }
                            }
                        }
                    }
                }

                // Update transaction records
                if let Some(old_txn) = transaction::get_transaction_by_dest(&current_path) {
                    if let Err(e) = transaction::mark_undone(&old_txn.id) {
                        log::warn!("[rescan] Failed to mark old transaction {} as undone: {}", old_txn.id, e);
                    }
                }
                if let Err(e) = transaction::record(&transaction::Transaction {
                    id: uuid::Uuid::new_v4().to_string(),
                    batch_id: batch_id.clone(),
                    source_path: current_path,
                    dest_path: analysis.dest_path.clone(),
                    title: analysis.title,
                    year: analysis.year,
                    format: analysis.format,
                    genre: analysis.genre,
                    media_type: analysis.media_type,
                    season: analysis.season,
                    episode: analysis.episode,
                    episode_title: analysis.episode_title,
                    tmdb_id: analysis.tmdb_id,
                    poster_url: analysis.poster_url,
                    sha256: String::new(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    undone: false,
                    locked: false,
                    confidence: analysis.confidence,
                }) {
                    log::warn!("[rescan] Failed to record transaction: {}", e);
                }
            }

            if folder_fixed {
                progress.fixed += 1;
                log::info!("[rescan] Fixed: {}", name);
                cleanup_empty_dir_recursive(folder);
            }
        } else {
            progress.skipped += 1;
        }
    }

    // Final cleanup: remove any now-empty genre/format directories
    if let Ok(formats) = fs::read_dir(library) {
        for fe in formats.flatten() {
            if fe.path().is_dir() {
                cleanup_empty_dir_recursive(&fe.path());
            }
        }
    }

    log::info!(
        "[rescan] Done: {} scanned, {} fixed, {} skipped, {} failed",
        total, progress.fixed, progress.skipped, progress.failed
    );
    progress
}

/// Determine if a title folder needs re-analysis.
fn needs_reanalysis(title_path: &Path) -> bool {
    let title_name = title_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let genre_name = title_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if genre_name == "Uncategorized" {
        return true;
    }
    if !title_name.contains("[tmdbid-") {
        return true;
    }
    if title_name.contains("( (") {
        return true;
    }
    false
}

/// Collect all video files inside a title folder (direct + Season subdirs).
fn collect_videos_in_title(title_path: &Path) -> Vec<PathBuf> {
    let mut videos = Vec::new();
    if let Ok(entries) = fs::read_dir(title_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && renamer::is_video_file(&p) {
                videos.push(p);
            } else if p.is_dir() {
                if let Ok(sub) = fs::read_dir(&p) {
                    for se in sub.flatten() {
                        if se.path().is_file() && renamer::is_video_file(&se.path()) {
                            videos.push(se.path());
                        }
                    }
                }
            }
        }
    }
    videos
}

/// Remove empty directories recursively (bottom-up), cleaning platform junk first.
fn cleanup_empty_dir_recursive(dir: &Path) {
    if !dir.is_dir() { return; }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                cleanup_empty_dir_recursive(&p);
            } else if crate::junk::is_platform_junk(&p)
                || p.file_name()
                    .map(|n| n.to_string_lossy().ends_with(".thumb.jpg"))
                    .unwrap_or(false)
            {
                if let Err(e) = fs::remove_file(&p) {
                    log::warn!("[rescan] Failed to remove junk file {}: {}", p.display(), e);
                }
            }
        }
    }
    let _ = fs::remove_dir(dir); // only succeeds if empty
}
