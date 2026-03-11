/// Companion file detection and trash management.
///
/// When a video lives in a dedicated folder (one video + companions), this module
/// detects companion files (images, SRTs in subfolders, etc.) and categorizes them
/// for the pipeline: subtitles move with the video, everything else goes to trash.

use crate::config;
use crate::db::{collect_rows, with_connection};
use crate::error::AppResult;
use crate::shared::video::is_video_file;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "tif"];

/// Result of scanning a video file's parent folder for companions.
#[derive(Debug, Clone)]
pub struct CompanionScan {
    /// True if the folder is dedicated to this video (safe to recurse for subtitles/junk).
    /// A folder is dedicated when it has exactly one non-sample video AND no subdirectories
    /// that themselves contain videos (which would indicate a shared drop root).
    pub is_dedicated_folder: bool,
    /// Image files found alongside the video (candidates for trash).
    pub image_files: Vec<String>,
}

/// Scan the parent directory of a video file to detect companion files.
/// Only returns images as companions — subtitles are already handled by
/// `subtitles::find_subtitles`, and junk by `junk::find_junk_files_recursive`.
pub fn scan_companions(video_path: &Path) -> CompanionScan {
    let parent = match video_path.parent() {
        Some(p) => p,
        None => return CompanionScan { is_dedicated_folder: false, image_files: Vec::new() },
    };

    let mut video_count = 0u32;
    let mut image_files = Vec::new();
    let mut has_video_subdir = false;

    let entries = match fs::read_dir(parent) {
        Ok(e) => e,
        Err(_) => return CompanionScan { is_dedicated_folder: false, image_files: Vec::new() },
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if is_video_file(&path) && !is_sample_file(&path) {
                video_count += 1;
            } else if is_image_file(&path) {
                image_files.push(path.to_string_lossy().to_string());
            }
        } else if path.is_dir() && !has_video_subdir {
            has_video_subdir = dir_contains_video(&path);
        }
    }

    // A dedicated folder has exactly one video and no sibling directories containing
    // other videos. True dedicated folders (e.g., "Movie.2024.1080p/") have flat
    // structure: video + Subs/ + junk. A drop root with video folders alongside
    // standalone files has subdirectories with videos and must NOT recurse.
    CompanionScan {
        is_dedicated_folder: video_count == 1 && !has_video_subdir,
        image_files,
    }
}

/// Check if a file looks like a sample (< 100MB with "sample" in the name).
fn is_sample_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase().contains("sample"))
        .unwrap_or(false)
        && fs::metadata(path).map(|m| m.len() < 100 * 1024 * 1024).unwrap_or(false)
}

/// Shallow check: does this directory contain any non-sample video file?
fn dir_contains_video(dir: &Path) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_video_file(&path) && !is_sample_file(&path) {
            return true;
        }
    }
    false
}

fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

// --- Trash DB operations ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrashItem {
    pub id: String,
    pub transaction_id: String,
    pub original_path: String,
    pub trash_path: String,
    pub filename: String,
    pub timestamp: String,
}

/// Move a companion file to the library trash and record it.
pub fn trash_companion(
    original_path: &str,
    transaction_id: &str,
    library_path: &str,
) -> AppResult<()> {
    let src = Path::new(original_path);
    if !src.exists() {
        return Ok(());
    }

    let filename = src.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let trash_dir = config::trash_dir(library_path)?;
    let trash_dest = unique_path(&trash_dir.join(&filename));

    fs::rename(src, &trash_dest).or_else(|_| {
        fs::copy(src, &trash_dest)?;
        fs::remove_file(src)
    }).map_err(|e| crate::error::AppError::Io(e))?;

    let item = TrashItem {
        id: uuid::Uuid::new_v4().to_string(),
        transaction_id: transaction_id.to_string(),
        original_path: original_path.to_string(),
        trash_path: trash_dest.to_string_lossy().to_string(),
        filename,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    record_trash(&item)?;
    Ok(())
}

/// Restore all trashed companions for a transaction back to their original locations.
pub fn restore_companions(transaction_id: &str) -> AppResult<u32> {
    let items = get_trash_by_transaction(transaction_id)?;
    let mut restored = 0u32;

    for item in &items {
        let trash = Path::new(&item.trash_path);
        let original = Path::new(&item.original_path);

        if !trash.exists() {
            log::warn!("[restore] Trash file missing, cleaning DB record: {}", item.trash_path);
            if let Err(e) = delete_trash_record(&item.id) {
                log::warn!("[restore] Failed to clean DB record {}: {}", item.id, e);
            }
            continue;
        }
        if let Some(parent) = original.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::warn!("[restore] Failed to create parent dir {}: {}", parent.display(), e);
                continue;
            }
        }
        let moved = if fs::rename(trash, original).is_ok() {
            true
        } else if fs::copy(trash, original).is_ok() {
            if let Err(e) = fs::remove_file(trash) {
                log::warn!("[restore] Failed to remove trash copy {}: {}", item.trash_path, e);
            }
            true
        } else {
            log::warn!("[restore] Failed to restore {} -> {}", item.trash_path, item.original_path);
            false
        };
        if moved {
            restored += 1;
            if let Err(e) = delete_trash_record(&item.id) {
                log::warn!("[restore] Failed to delete trash record {}: {}", item.id, e);
            }
        }
    }
    Ok(restored)
}

/// Get all trash items (for the Trash page UI).
pub fn get_all_trash() -> AppResult<Vec<TrashItem>> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, original_path, trash_path, filename, timestamp
             FROM trash_items ORDER BY timestamp DESC"
        )?;
        collect_rows(&mut stmt, [], trash_from_row)
    })
}

/// Permanently delete a single trash item from disk and DB.
pub fn delete_trash_item(id: &str) -> AppResult<()> {
    let item = get_trash_by_id(id)?;
    let trash = Path::new(&item.trash_path);
    if trash.exists() {
        fs::remove_file(trash)?;
    }
    delete_trash_record(id)
}

/// Permanently delete all trash items.
pub fn empty_trash() -> AppResult<u32> {
    let items = get_all_trash()?;
    let mut deleted = 0u32;
    for item in &items {
        let trash = Path::new(&item.trash_path);
        if trash.exists() {
            if let Err(e) = fs::remove_file(trash) {
                log::warn!("[trash] Failed to delete {}: {}", item.trash_path, e);
                continue;
            }
        }
        deleted += 1;
        if let Err(e) = delete_trash_record(&item.id) {
            log::warn!("[trash] Failed to delete DB record {}: {}", item.id, e);
        }
    }
    Ok(deleted)
}

/// Restore a single trash item to its original location.
pub fn restore_trash_item(id: &str) -> AppResult<String> {
    let item = get_trash_by_id(id)?;
    let trash = Path::new(&item.trash_path);
    let original = Path::new(&item.original_path);

    if !trash.exists() {
        delete_trash_record(id)?;
        return Err(crate::error::AppError::NotFound(item.trash_path.into()));
    }
    if let Some(parent) = original.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(trash, original).is_err() {
        fs::copy(trash, original)?;
        fs::remove_file(trash)?;
    }
    delete_trash_record(id)?;
    Ok(item.original_path)
}

/// Get the number of items in the trash using SQL COUNT.
pub fn get_trash_count() -> AppResult<u32> {
    with_connection(|conn| {
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM trash_items",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    })
}

// --- DB helpers ---

fn record_trash(item: &TrashItem) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO trash_items (id, transaction_id, original_path, trash_path, filename, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![item.id, item.transaction_id, item.original_path, item.trash_path, item.filename, item.timestamp],
        )?;
        Ok(())
    })
}

fn get_trash_by_id(id: &str) -> AppResult<TrashItem> {
    with_connection(|conn| {
        conn.query_row(
            "SELECT id, transaction_id, original_path, trash_path, filename, timestamp
             FROM trash_items WHERE id = ?1",
            params![id],
            trash_from_row,
        ).map_err(Into::into)
    })
}

fn get_trash_by_transaction(transaction_id: &str) -> AppResult<Vec<TrashItem>> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, transaction_id, original_path, trash_path, filename, timestamp
             FROM trash_items WHERE transaction_id = ?1"
        )?;
        collect_rows(&mut stmt, params![transaction_id], trash_from_row)
    })
}

fn delete_trash_record(id: &str) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute("DELETE FROM trash_items WHERE id = ?1", params![id])?;
        Ok(())
    })
}

fn trash_from_row(row: &rusqlite::Row) -> rusqlite::Result<TrashItem> {
    Ok(TrashItem {
        id: row.get(0)?,
        transaction_id: row.get(1)?,
        original_path: row.get(2)?,
        trash_path: row.get(3)?,
        filename: row.get(4)?,
        timestamp: row.get(5)?,
    })
}

/// Generate a unique file path by appending (1), (2), etc. if the file exists.
fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let parent = path.parent().unwrap_or(Path::new("."));

    for i in 1..1000 {
        let name = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };
        let candidate = parent.join(&name);
        if !candidate.exists() {
            return candidate;
        }
    }
    // Exhausted numeric suffixes — use UUID to guarantee uniqueness
    let uuid = uuid::Uuid::new_v4();
    let name = if ext.is_empty() {
        format!("{}_{}", stem, uuid)
    } else {
        format!("{}_{}.{}", stem, uuid, ext)
    };
    log::warn!("[companion] Exhausted 1000 unique path attempts for {}, using UUID", path.display());
    parent.join(&name)
}
