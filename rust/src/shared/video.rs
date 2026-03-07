/// Video and subtitle file detection — single source of truth.
///
/// Previously duplicated across renamer.rs and subtitles.rs.

use std::path::Path;

pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "m2ts", "mpg", "mpeg",
];

pub const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "sub", "ass", "ssa", "vtt"];

pub fn is_video_extension(ext: &str) -> bool {
    VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Check if a path points to a video file.
/// For `.ts` files, requires >1MB since TypeScript uses the same extension
/// as Transport Stream video.
pub fn is_video_file(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    if !is_video_extension(ext) {
        return false;
    }
    if ext.eq_ignore_ascii_case("ts") {
        return std::fs::metadata(path).map(|m| m.len() > 1_000_000).unwrap_or(false);
    }
    true
}

pub fn is_subtitle_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| SUBTITLE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}
