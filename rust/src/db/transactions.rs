/// Transaction CRUD — record, query, undo, review operations.

use crate::companion;
use crate::config;
use crate::db::with_connection;
use crate::error::AppResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub batch_id: String,
    pub source_path: String,
    pub dest_path: String,
    pub title: String,
    pub year: Option<u16>,
    pub format: String,
    pub genre: String,
    pub media_type: String,
    pub season: Option<u16>,
    pub episode: Option<u16>,
    pub episode_title: Option<String>,
    pub tmdb_id: Option<u64>,
    pub poster_url: Option<String>,
    pub sha256: String,
    pub timestamp: String,
    pub undone: bool,
    pub locked: bool,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UndoResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UndoBatchResult {
    pub total: u32,
    pub succeeded: u32,
    pub failed: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryPage {
    pub items: Vec<Transaction>,
    pub total: u32,
    pub has_more: bool,
}

const TXN_COLS: &str = "id, batch_id, source_path, dest_path, title, year, format, genre, \
    media_type, season, episode, episode_title, tmdb_id, poster_url, sha256, timestamp, \
    undone, locked, confidence";

fn transaction_from_row(row: &rusqlite::Row) -> rusqlite::Result<Transaction> {
    Ok(Transaction {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        source_path: row.get(2)?,
        dest_path: row.get(3)?,
        title: row.get(4)?,
        year: row.get::<_, Option<i32>>(5)?.and_then(|y| u16::try_from(y.max(0)).ok()),
        format: row.get(6)?,
        genre: row.get(7)?,
        media_type: row.get(8)?,
        season: row.get::<_, Option<i32>>(9)?.and_then(|s| u16::try_from(s.max(0)).ok()),
        episode: row.get::<_, Option<i32>>(10)?.and_then(|e| u16::try_from(e.max(0)).ok()),
        episode_title: row.get(11)?,
        tmdb_id: row.get::<_, Option<i64>>(12)?.map(|id| id.max(0) as u64),
        poster_url: row.get(13)?,
        sha256: row.get(14)?,
        timestamp: row.get(15)?,
        undone: row.get::<_, i32>(16)? != 0,
        locked: row.get::<_, i32>(17).unwrap_or(0) != 0,
        confidence: row.get::<_, f64>(18).unwrap_or(0.0) as f32,
    })
}

use crate::db::collect_rows;

pub fn record(txn: &Transaction) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO transactions (id, batch_id, source_path, dest_path, title, year, format, genre,
             media_type, season, episode, episode_title, tmdb_id, poster_url, sha256, timestamp, undone, locked, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                txn.id, txn.batch_id, txn.source_path, txn.dest_path, txn.title, txn.year,
                txn.format, txn.genre, txn.media_type, txn.season, txn.episode, txn.episode_title,
                txn.tmdb_id.map(|id| id as i64), txn.poster_url, txn.sha256, txn.timestamp,
                txn.undone as i32, txn.locked as i32, txn.confidence as f64,
            ],
        )?;
        Ok(())
    })
}

pub fn record_subtitle(
    transaction_id: &str,
    source_path: Option<&str>,
    dest_path: &str,
    language: &str,
    downloaded: bool,
) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO subtitle_records (id, transaction_id, source_path, dest_path, language, downloaded, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                uuid::Uuid::new_v4().to_string(),
                transaction_id, source_path, dest_path, language,
                downloaded as i32, chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    })
}

pub fn get_history(offset: u32, limit: u32) -> AppResult<HistoryPage> {
    with_connection(|conn| {
        let total: u32 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE undone = 0",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM transactions WHERE undone = 0
             ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
            TXN_COLS
        ))?;
        let items = collect_rows(&mut stmt, params![limit, offset], transaction_from_row)?;
        let has_more = (offset + limit) < total;

        Ok(HistoryPage { items, total, has_more })
    })
}

pub fn undo_transaction(id: &str) -> AppResult<UndoResult> {
    with_connection(|conn| {
        let txn: Transaction = conn.query_row(
            &format!("SELECT {} FROM transactions WHERE id = ?1", TXN_COLS),
            params![id],
            transaction_from_row,
        )?;

        if txn.undone {
            return Ok(UndoResult {
                success: false,
                message: "Transaction already undone".to_string(),
            });
        }

        let dest_path = Path::new(&txn.dest_path);
        if !dest_path.exists() {
            if let Err(e) = conn.execute("UPDATE transactions SET undone = 1 WHERE id = ?1", params![id]) {
                log::warn!("[undo] Failed to mark transaction {} as undone: {}", id, e);
            }
            return Ok(UndoResult {
                success: false,
                message: "File has been moved or deleted since organizing".to_string(),
            });
        }

        if !txn.sha256.is_empty() {
            match compute_sha256(&txn.dest_path) {
                Ok(hash) if hash == txn.sha256 => {}
                Ok(_) => {
                    return Ok(UndoResult {
                        success: false,
                        message: "File has been modified since organizing. Can't undo automatically.".to_string(),
                    });
                }
                Err(e) => {
                    return Ok(UndoResult {
                        success: false,
                        message: format!("Can't verify file: {}", e),
                    });
                }
            }
        }

        // Canonicalize BEFORE the rename — dest_path won't exist after the move.
        let library_root = config::load_config()
            .map_err(|e| log::debug!("[undo] Failed to load config: {}", e))
            .ok()
            .and_then(|c| c.library_path)
            .and_then(|p| fs::canonicalize(&p)
                .map_err(|e| log::debug!("[undo] Failed to canonicalize library path: {}", e))
                .ok());
        let dest_canonical = fs::canonicalize(dest_path)
            .map_err(|e| log::debug!("[undo] Failed to canonicalize dest path {}: {}", dest_path.display(), e))
            .ok();

        let source_path = Path::new(&txn.source_path);
        if let Some(parent) = source_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if fs::rename(dest_path, source_path).is_err() {
            let src_size = fs::metadata(dest_path)?.len();
            let copied = fs::copy(dest_path, source_path)?;
            if copied != src_size {
                let _ = fs::remove_file(source_path);
                return Err(crate::error::AppError::Process(format!(
                    "Undo copy verification failed: expected {}B, copied {}B",
                    src_size, copied
                )));
            }
            if let Err(e) = fs::remove_file(dest_path) {
                log::warn!("[undo] Failed to remove dest after copy {}: {}", dest_path.display(), e);
            }
        }

        let mut sub_stmt = conn.prepare(
            "SELECT dest_path FROM subtitle_records WHERE transaction_id = ?1",
        )?;
        let sub_paths: Vec<String> = collect_rows(&mut sub_stmt, params![id], |row| row.get(0))?;
        for sub_path in &sub_paths {
            if let Err(e) = fs::remove_file(sub_path) {
                log::error!("[undo] Failed to remove subtitle {}: {}", sub_path, e);
            }
        }

        // Restore trashed companion files back to original locations
        if let Err(e) = companion::restore_companions(id) {
            log::warn!("[undo] Failed to restore companions for {}: {}", id, e);
        }

        // Clean empty library folders up to the library root.
        if let (Some(root), Some(dest_c)) = (library_root.as_deref(), dest_canonical.as_deref()) {
            clean_empty_parents(dest_c, root);
        }

        conn.execute("UPDATE transactions SET undone = 1 WHERE id = ?1", params![id])?;

        Ok(UndoResult {
            success: true,
            message: format!("Moved back to {}", txn.source_path),
        })
    })
}

pub fn undo_batch(batch_id: &str) -> AppResult<UndoBatchResult> {
    let ids: Vec<String> = with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id FROM transactions WHERE batch_id = ?1 AND undone = 0",
        )?;
        collect_rows(&mut stmt, params![batch_id], |row| row.get(0))
    })?;

    let total = ids.len() as u32;
    let mut succeeded = 0u32;
    let mut failed = 0u32;

    for id in ids {
        match undo_transaction(&id) {
            Ok(result) if result.success => succeeded += 1,
            _ => failed += 1,
        }
    }

    Ok(UndoBatchResult { total, succeeded, failed })
}

pub fn compute_sha256(path: &str) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn get_transaction_by_dest(dest_path: &str) -> Option<Transaction> {
    with_connection(|conn| {
        conn.query_row(
            &format!(
                "SELECT {} FROM transactions WHERE dest_path = ?1 AND undone = 0 ORDER BY timestamp DESC LIMIT 1",
                TXN_COLS
            ),
            params![dest_path],
            transaction_from_row,
        ).map_err(Into::into)
    })
    .ok()
}

pub fn mark_undone(id: &str) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute("UPDATE transactions SET undone = 1 WHERE id = ?1", params![id])?;
        Ok(())
    })
}

pub fn get_subtitle_paths(transaction_id: &str) -> Vec<String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT dest_path FROM subtitle_records WHERE transaction_id = ?1",
        )?;
        collect_rows(&mut stmt, params![transaction_id], |row| row.get(0))
    })
    .unwrap_or_default()
}

pub fn get_poster_for_path(dest_path: &str) -> Option<String> {
    with_connection(|conn| {
        conn.query_row(
            "SELECT poster_url FROM transactions WHERE dest_path = ?1 AND poster_url IS NOT NULL",
            params![dest_path],
            |row| row.get(0),
        ).map_err(Into::into)
    })
    .ok()
}

pub fn get_episode_title_for_path(dest_path: &str) -> Option<String> {
    with_connection(|conn| {
        conn.query_row(
            "SELECT episode_title FROM transactions WHERE dest_path = ?1 AND episode_title IS NOT NULL",
            params![dest_path],
            |row| row.get(0),
        ).map_err(Into::into)
    })
    .ok()
}

pub fn get_transaction_by_id(id: &str) -> AppResult<Transaction> {
    with_connection(|conn| {
        conn.query_row(
            &format!("SELECT {} FROM transactions WHERE id = ?1", TXN_COLS),
            params![id],
            transaction_from_row,
        ).map_err(Into::into)
    })
}

pub fn get_review_items(batch_ids: &[String]) -> AppResult<Vec<Transaction>> {
    if batch_ids.is_empty() {
        return Ok(Vec::new());
    }
    with_connection(|conn| {
        let placeholders: String = (1..=batch_ids.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT {} FROM transactions WHERE batch_id IN ({}) AND undone = 0 AND locked = 0
             ORDER BY format, genre, title, season, episode",
            TXN_COLS, placeholders
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> =
            batch_ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
        collect_rows(&mut stmt, params.as_slice(), transaction_from_row)
    })
}

pub fn get_review_count(batch_ids: &[String]) -> AppResult<u32> {
    if batch_ids.is_empty() {
        return Ok(0);
    }
    with_connection(|conn| {
        let placeholders: String = (1..=batch_ids.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT COUNT(*) FROM transactions WHERE batch_id IN ({}) AND undone = 0 AND locked = 0",
            placeholders
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> =
            batch_ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
        let count: u32 = stmt.query_row(params.as_slice(), |row| row.get(0))?;
        Ok(count)
    })
}

pub fn lock_transactions(ids: &[String]) -> AppResult<u32> {
    if ids.is_empty() {
        return Ok(0);
    }
    with_connection(|conn| {
        let placeholders: String = (1..=ids.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "UPDATE transactions SET locked = 1 WHERE id IN ({}) AND undone = 0",
            placeholders
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
        let affected = stmt.execute(params.as_slice())?;
        Ok(affected as u32)
    })
}

pub fn undo_all_pending() -> AppResult<(u32, u32)> {
    let ids: Vec<String> = with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id FROM transactions WHERE locked = 0 AND undone = 0",
        )?;
        collect_rows(&mut stmt, [], |row| row.get(0))
    })?;

    let mut succeeded = 0u32;
    let mut failed = 0u32;

    for id in &ids {
        match undo_transaction(id) {
            Ok(result) if result.success => succeeded += 1,
            _ => failed += 1,
        }
    }

    Ok((succeeded, failed))
}

pub fn update_transaction_metadata(
    id: &str,
    title: &str,
    year: Option<u16>,
    format: &str,
    genre: &str,
    tmdb_id: Option<u64>,
    poster_url: Option<&str>,
) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute(
            "UPDATE transactions SET title = ?1, year = ?2, format = ?3, genre = ?4, tmdb_id = ?5, poster_url = ?6 WHERE id = ?7",
            params![title, year, format, genre, tmdb_id.map(|id| id as i64), poster_url, id],
        )?;
        Ok(())
    })
}

pub fn update_subtitle_paths(
    transaction_id: &str,
    old_stem: &str,
    new_stem: &str,
    new_dir: &str,
) -> AppResult<()> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, dest_path FROM subtitle_records WHERE transaction_id = ?1",
        )?;
        let records: Vec<(String, String)> =
            collect_rows(&mut stmt, params![transaction_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;

        for (sub_id, old_path) in &records {
            let old_filename = Path::new(old_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let new_filename = if old_filename.to_lowercase().starts_with(&old_stem.to_lowercase()) {
                format!("{}{}", new_stem, &old_filename[old_stem.len()..])
            } else {
                old_filename.to_string()
            };
            let new_path = Path::new(new_dir).join(&new_filename);
            if let Err(e) = conn.execute(
                "UPDATE subtitle_records SET dest_path = ?1 WHERE id = ?2",
                params![new_path.to_string_lossy().to_string(), sub_id],
            ) {
                log::warn!("[relocate] Failed to update subtitle record {}: {}", sub_id, e);
            }
        }
        Ok(())
    })
}

pub fn reassign_subtitle_records(
    old_transaction_id: &str,
    new_transaction_id: &str,
) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute(
            "UPDATE subtitle_records SET transaction_id = ?1 WHERE transaction_id = ?2",
            params![new_transaction_id, old_transaction_id],
        )?;
        Ok(())
    })
}

fn clean_empty_parents(path: &Path, stop_at: &Path) {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == stop_at || !dir.starts_with(stop_at) {
            break;
        }
        if dir.parent().is_none() {
            break;
        }
        if fs::remove_dir(dir).is_err() {
            break;
        }
        current = dir.parent();
    }
}
