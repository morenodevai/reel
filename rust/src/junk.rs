use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

static JUNK_FILENAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(RARBG.*|Torrent.downloaded.*|www\..*|.*-sample\.(mkv|mp4|avi))$").unwrap()
});

const JUNK_EXTENSIONS: &[&str] = &[
    "nfo", "txt", "exe", "bat", "cmd", "url", "lnk", "htm", "html",
    "torrent", "nzb",
];
// NOTE: Image extensions (jpg, png, gif, etc.) intentionally excluded.
// They can be poster art, fanart, or folder.jpg used by media players.

const JUNK_FILENAMES: &[&str] = &["Thumbs.db", "desktop.ini", ".DS_Store"];

use crate::shared::video::SUBTITLE_EXTENSIONS;

/// Check if a file is a junk file that should be removed.
pub fn is_junk_file(path: &Path) -> bool {
    let filename = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };

    // Never delete subtitle files
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if SUBTITLE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            return false;
        }
    }

    // Check exact filenames
    if JUNK_FILENAMES.contains(&filename) {
        return true;
    }

    // Check extensions
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if JUNK_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            return true;
        }
    }

    // Check filename patterns
    if JUNK_FILENAME_RE.is_match(filename) {
        // For sample videos, only delete if < 100MB
        if filename.to_lowercase().contains("sample") {
            if let Ok(metadata) = std::fs::metadata(path) {
                return metadata.len() < 100 * 1024 * 1024; // 100MB
            }
        }
        return true;
    }

    false
}

/// Check if a file is OS-generated detritus (.DS_Store, Thumbs.db, desktop.ini).
/// Narrower than is_junk_file — only matches platform junk, not torrent extras.
pub fn is_platform_junk(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |name| JUNK_FILENAMES.contains(&name))
}

/// Find all junk files in a directory (non-recursive).
pub fn find_junk_files(dir: &Path) -> Vec<String> {
    let mut junk = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_junk_file(&path) {
                junk.push(path.to_string_lossy().to_string());
            }
        }
    }

    junk
}

/// Find junk files recursively in a directory.
pub fn find_junk_files_recursive(dir: &Path) -> Vec<String> {
    let mut junk = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && is_junk_file(path) {
            junk.push(path.to_string_lossy().to_string());
        }
    }

    junk
}
