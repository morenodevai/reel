/// Local subtitle file discovery — find subtitle files near a video.

use crate::shared::video::is_subtitle_file;
use std::path::Path;

/// Minimum stem length for reverse-prefix matching to avoid false-matching
/// short language-only stems like "eng", "2_Eng", or "spa".
const MIN_REVERSE_PREFIX_LEN: usize = 6;

/// Find subtitle files for a video in a dedicated folder.
/// Searches same directory (stem-matched) and ALL subdirectories (takes every subtitle
/// regardless of name — for language-only files like "English.srt", "Spanish.srt").
pub fn find_subtitles_dedicated(dir: &str, video_stem: &str) -> Vec<String> {
    let mut found = Vec::new();
    let dir_path = Path::new(dir);

    // Search in the same directory (stem-matched only)
    find_subs_in_dir(dir_path, video_stem, &mut found);

    // In dedicated folders, subdirectories (Subs/, Subtitles/, etc.) contain subtitles
    // named by language only. Take ALL subtitle files from every subdirectory.
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_subs_in_dir_all(&path, &mut found);
            }
        }
    }

    found
}

/// Find subtitle files for a video in a shared directory (multiple videos).
/// Searches same directory AND subdirectories, but always requires stem matching.
/// This prevents cross-contamination while still finding subs in Subs/ folders
/// when the subtitle filename matches the video (e.g., "Show.S01E01.English.srt").
pub fn find_subtitles_local(dir: &str, video_stem: &str) -> Vec<String> {
    let mut found = Vec::new();
    let dir_path = Path::new(dir);

    // Search same directory (stem-matched)
    find_subs_in_dir(dir_path, video_stem, &mut found);

    // Search subdirectories (stem-matched only — safe for shared directories)
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_subs_stem_matched_recursive(&path, video_stem, &mut found);
            }
        }
    }

    found
}

/// Check if a subtitle filename matches a video stem.
/// Handles: "Movie.eng.srt" matching "Movie.2024.1080p", and reverse prefix where
/// a shorter subtitle base matches the start of a longer video stem.
pub fn matches_video_stem(sub_path: &Path, video_stem: &str) -> bool {
    if video_stem.is_empty() {
        return false;
    }
    let sub_stem = sub_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let video_lower = video_stem.to_lowercase();

    if sub_stem.starts_with(&video_lower) {
        return true;
    }

    // Strip language/tag suffix (e.g., "Movie.eng" → "Movie") and check reverse prefix.
    // The subtitle stem may be shorter than the video stem when the video has quality
    // tags (1080p, BluRay, x264) that the subtitle omits.
    let sub_base = sub_stem
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(&sub_stem);
    sub_base.len() >= MIN_REVERSE_PREFIX_LEN && video_lower.starts_with(sub_base)
}

fn find_subs_in_dir(dir: &Path, video_stem: &str, results: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_subtitle_file(&path) && matches_video_stem(&path, video_stem) {
                results.push(path.to_string_lossy().to_string());
            }
        }
    }
}

/// Search subdirectories recursively but only take stem-matched subtitles.
fn find_subs_stem_matched_recursive(dir: &Path, video_stem: &str, results: &mut Vec<String>) {
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && is_subtitle_file(path) && matches_video_stem(path, video_stem) {
            results.push(path.to_string_lossy().to_string());
        }
    }
}

/// Search subdirectories and take ALL subtitles regardless of name.
/// Only for dedicated folders where subs are named by language only (e.g., "English.srt").
fn find_subs_in_dir_all(dir: &Path, results: &mut Vec<String>) {
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
