/// Database module — connection management, schema, transactions, watch progress.

pub mod schema;
pub mod transactions;
pub mod watch_progress;

use crate::config;
use crate::error::{AppError, AppResult};
use once_cell::sync::Lazy;
use rusqlite::Connection;
use std::fs;
use std::sync::{Mutex, MutexGuard};

static DB_CONNECTION: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));

pub fn get_connection() -> AppResult<MutexGuard<'static, Option<Connection>>> {
    let mut guard = DB_CONNECTION
        .lock()
        .map_err(|e| AppError::Db(format!("DB lock error: {}", e)))?;

    if guard.is_none() {
        let db_path = config::db_path();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        *guard = Some(conn);
    }

    Ok(guard)
}

pub fn with_connection<F, T>(f: F) -> AppResult<T>
where
    F: FnOnce(&Connection) -> AppResult<T>,
{
    let guard = get_connection()?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| AppError::Db("Database connection not initialized".into()))?;
    f(conn)
}

/// Collect rows from a query, skipping corrupt rows with a warning.
pub fn collect_rows<T>(
    stmt: &mut rusqlite::Statement,
    params: impl rusqlite::Params,
    mapper: fn(&rusqlite::Row) -> rusqlite::Result<T>,
) -> AppResult<Vec<T>> {
    let items: Vec<T> = stmt
        .query_map(params, mapper)?
        .filter_map(|r| match r {
            Ok(val) => Some(val),
            Err(e) => {
                log::warn!("Skipping corrupt database row: {}", e);
                None
            }
        })
        .collect();
    Ok(items)
}
