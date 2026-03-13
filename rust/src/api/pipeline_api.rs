/// Pipeline FFI API -- process files, reveal in file manager, rescan library.
///
/// Background processing uses StreamSink to emit progress events back to Dart.

use crate::config;
use crate::pipeline;
use crate::transaction;
use crate::frb_generated::StreamSink;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pipeline::CONFIDENCE_THRESHOLD;

/// Find the common parent directory of multiple paths.
fn common_parent_of<'a>(paths: impl Iterator<Item = &'a str>) -> String {
    let mut common: Option<PathBuf> = None;
    for p in paths {
        let path = Path::new(p);
        let parent = path.parent().unwrap_or(path);
        match common {
            None => common = Some(parent.to_path_buf()),
            Some(ref mut c) => {
                let mut shared = PathBuf::new();
                for (a, b) in c.components().zip(parent.components()) {
                    if a == b {
                        shared.push(a);
                    } else {
                        break;
                    }
                }
                *c = shared;
            }
        }
    }
    common
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Process files in the background. Returns immediately.
/// Emits JSON-encoded progress events via StreamSink.
///
/// Event format (JSON):
/// - {"type":"started","total":N}
/// - {"type":"progress","done":N,"total":N,"title":"...","succeeded":N,"failed":N,"batch_id":"..."}
/// - {"type":"done","total":N,"succeeded":N,"failed":N,"batch_id":"..."}
pub fn process_background(
    paths: Vec<String>,
    sink: StreamSink<String>,
) -> Result<(), String> {
    if !pipeline::try_start_bg_processing() {
        return Ok(());
    }

    let cfg = config::load_config()?;
    let cleanup_root = common_parent_of(paths.iter().map(|s| s.as_str()));
    let library = cfg.library_path.clone().ok_or_else(|| {
        pipeline::finish_bg_processing();
        "No library path configured".to_string()
    })?;

    std::thread::spawn(move || {
        // 1. Scan for video files
        let all_video_files = pipeline::scan_video_files(&paths);

        // Filter out files already inside the library to prevent re-processing
        // organized content (which would move/delete properly named folders).
        let library_path = Path::new(&library);
        let video_files: Vec<_> = all_video_files
            .into_iter()
            .filter(|p| {
                if p.starts_with(library_path) {
                    log::info!("[process] Skipping file already in library: {}", p.display());
                    false
                } else {
                    true
                }
            })
            .collect();

        let total = video_files.len();
        if total == 0 {
            pipeline::finish_bg_processing();
            let _ = sink.add(serde_json::json!({
                "type": "done", "total": 0, "succeeded": 0, "failed": 0
            }).to_string());
            return;
        }

        let _ = sink.add(serde_json::json!({
            "type": "started", "total": total
        }).to_string());

        // 2. Analyze files sequentially (each may do network calls)
        let api_key = cfg.tmdb_api_key.clone();
        let opensubs_key = cfg.opensubs_api_key.clone();
        let mut analyses = Vec::with_capacity(total);

        for video_path in &video_files {
            let filename = video_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let analysis =
                pipeline::analyze_single_file_pub(video_path, &filename, &api_key, &opensubs_key, &library, None);
            analyses.push(analysis);
        }

        // 2.5. Route low-confidence items to "Needs Review" staging area.
        // Proposed metadata is preserved in the transaction DB so the review
        // page can show rename previews and let the user confirm or correct.
        let library_path = Path::new(&library);
        let mut needs_review_count = 0u32;

        for analysis in &mut analyses {
            if analysis.confidence < CONFIDENCE_THRESHOLD {
                // Reroute dest_path: keep title_folder and filename structure,
                // but place under "Needs Review/Pending" instead of the guessed format/genre.
                let proposed_format = analysis.format.clone();
                let proposed_genre = analysis.genre.clone();
                let dest = Path::new(&analysis.dest_path);
                if let Ok(relative) = dest.strip_prefix(library_path) {
                    let components: Vec<_> = relative.components().collect();
                    // components[0]=format, [1]=genre, [2..]=title_folder/season/file
                    if components.len() >= 3 {
                        let mut new_dest = library_path.join("Needs Review").join("Pending");
                        for c in &components[2..] {
                            new_dest = new_dest.join(c);
                        }
                        analysis.dest_path = new_dest.to_string_lossy().to_string();
                        analysis.format = "Needs Review".to_string();
                        analysis.genre = "Pending".to_string();
                        needs_review_count += 1;
                        log::info!(
                            "[triage] Low confidence ({:.2}) — routing to Needs Review: {} (proposed: {}/{})",
                            analysis.confidence, analysis.title, proposed_format, proposed_genre,
                        );
                    } else {
                        log::warn!(
                            "[triage] Low confidence ({:.2}) but path too short to reroute: {}",
                            analysis.confidence, analysis.dest_path,
                        );
                    }
                } else {
                    log::warn!(
                        "[triage] Low confidence ({:.2}) but dest not in library: {}",
                        analysis.confidence, analysis.dest_path,
                    );
                }
            }
        }

        // 3. Process files sequentially
        let batch_id = uuid::Uuid::new_v4().to_string();
        let mut succeeded = 0u32;
        let mut failed = 0u32;

        for (i, analysis) in analyses.iter().enumerate() {
            let is_review = analysis.format == "Needs Review";
            let download_subs = cfg.auto_download_subs && !is_review;
            match pipeline::process_single_file_pub(
                analysis,
                &batch_id,
                download_subs,
                &cfg.subtitle_languages,
                &cfg.opensubs_api_key,
                &cleanup_root,
                &library,
            ) {
                Ok(()) => succeeded += 1,
                Err(_) => failed += 1,
            }

            let _ = sink.add(serde_json::json!({
                "type": "progress",
                "done": i + 1,
                "total": total,
                "title": analysis.title,
                "succeeded": succeeded,
                "failed": failed,
                "batch_id": batch_id,
                "needs_review": is_review,
            }).to_string());
        }

        pipeline::finish_bg_processing();
        let _ = sink.add(serde_json::json!({
            "type": "done",
            "total": total,
            "succeeded": succeeded,
            "failed": failed,
            "batch_id": batch_id,
            "needs_review_count": needs_review_count,
        }).to_string());
    });

    Ok(())
}

/// Cancel any in-progress pipeline processing.
pub fn cancel_processing() {
    pipeline::cancel_processing();
}

/// Reveal a file or folder in the system file manager.
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    let path_obj = Path::new(&path);
    if !path_obj.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .status()
            .map_err(|e| format!("Failed to reveal: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path])
            .status()
            .map_err(|e| format!("Failed to reveal: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path.clone());
        Command::new("xdg-open")
            .arg(&parent)
            .status()
            .map_err(|e| format!("Failed to reveal: {}", e))?;
    }

    Ok(())
}

/// Open a file with the system default application.
pub fn open_file(path: String) -> Result<(), String> {
    let path_obj = Path::new(&path);
    if !path_obj.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .status()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .args(["/C", "start", "", &path])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .status()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .status()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    Ok(())
}

/// Rescan the library -- fix misidentified files, download missing subtitles.
/// Emits progress events via StreamSink as JSON strings.
pub fn rescan_library(
    sink: StreamSink<String>,
) -> Result<(), String> {
    let cfg = config::load_config()?;
    if cfg.library_path.is_none() {
        return Err("Library path not configured".to_string());
    }

    std::thread::spawn(move || {
        let result = pipeline::rescan_library(&cfg, |progress| {
            let msg = serde_json::to_string(progress).unwrap_or_default();
            let _ = sink.add(msg);
        });
        // Send final done message
        let done_msg = serde_json::to_string(&result).unwrap_or_default();
        let _ = sink.add(format!("DONE:{}", done_msg));
    });

    Ok(())
}

/// Move entire library from one location to another.
pub fn move_library(from_path: String, to_path: String) -> Result<u32, String> {
    pipeline::move_library(&from_path, &to_path)
}

/// Edit a transaction's metadata and relocate the file.
pub fn edit_and_relocate(edit: pipeline::EditRequest) -> Result<transaction::Transaction, String> {
    let cfg = config::load_config()?;
    let library = cfg.library_path.ok_or("No library path configured")?;
    pipeline::edit_and_relocate(&edit, &library)
}

/// Search TMDb for matching media.
pub fn search_tmdb_multi(
    query: String,
    year: Option<u16>,
) -> Result<Vec<crate::metadata::TmdbMatch>, String> {
    let cfg = config::load_config()?;
    Ok(crate::metadata::search_tmdb_multi(&query, year, &cfg.tmdb_api_key))
}

/// Get the list of available format options (Movies, Shows, Anime, etc.).
pub fn get_format_options() -> Vec<crate::formats::FormatOption> {
    crate::formats::get_format_options()
}
