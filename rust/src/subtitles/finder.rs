/// Local subtitle file discovery — find subtitle files near a video.

use crate::shared::video::is_subtitle_file;
use std::path::Path;

/// Find subtitle files associated with a video file (recursive: searches subdirectories too).
/// Use this for videos in dedicated folders where Subs/ subdirectories belong to the video.
pub fn find_subtitles(dir: &str, video_stem: &str) -> Vec<String> {
    let mut found = Vec::new();
    let dir_path = Path::new(dir);
    let mut visited = std::collections::HashSet::new();

    // Search in the same directory (filename-matched only)
    find_subs_in_dir(dir_path, video_stem, &mut found);

    // Search ALL subdirectories at the same level as the video.
    // Subtitle files can live in any subfolder (Subs/, Subtitles/, or even
    // folders named after the original release like "Kynodontas [2009] 1080p subtitles/srt/").
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let canonical = path.to_string_lossy().to_string();
            if visited.insert(canonical) {
                find_subs_in_dir_recursive(&path, video_stem, &mut found);
            }
        }
    }

    found
}

/// Find subtitle files in the same directory only (no subdirectory recursion).
/// Use this for standalone files in shared directories to prevent cross-contamination.
pub fn find_subtitles_local(dir: &str, video_stem: &str) -> Vec<String> {
    let mut found = Vec::new();
    find_subs_in_dir(Path::new(dir), video_stem, &mut found);
    found
}

fn find_subs_in_dir(dir: &Path, video_stem: &str, results: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_subtitle_file(&path) {
                let sub_stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let video_lower = video_stem.to_lowercase();

                if sub_stem.starts_with(&video_lower) || sub_stem.contains(&video_lower) {
                    results.push(path.to_string_lossy().to_string());
                } else {
                    let sub_base = sub_stem
                        .rsplit_once('.')
                        .map(|(base, _)| base)
                        .unwrap_or(&sub_stem);
                    if sub_base.len() > 5 && video_lower.starts_with(sub_base) {
                        results.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
}

fn find_subs_in_dir_recursive(dir: &Path, _video_stem: &str, results: &mut Vec<String>) {
    // Inside dedicated subtitle folders (Subs/, Subtitles/, etc.), take ALL subtitle
    // files — they belong to the parent video. Filenames are often just language names
    // (e.g., "English.srt", "Spanish.srt") with no reference to the video title.
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && is_subtitle_file(path) {
            results.push(path.to_string_lossy().to_string());
        }
    }
}
