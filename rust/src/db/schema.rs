/// Database schema creation and migrations.

use crate::db::with_connection;
use crate::error::AppResult;

pub fn init_db() -> AppResult<()> {
    with_connection(|conn| {
        migrate_from_v2(conn)?;
        create_tables(conn)?;
        migrate_to_v3_1(conn)?;
        create_watch_progress_table(conn)?;
        create_trash_table(conn)?;
        Ok(())
    })
}

fn migrate_from_v2(conn: &rusqlite::Connection) -> AppResult<()> {
    let has_old_schema = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='operations'")
        .and_then(|mut stmt| stmt.exists([]))
        .unwrap_or(false);

    if has_old_schema {
        let has_new_columns = conn
            .prepare("SELECT batch_id FROM transactions LIMIT 1")
            .is_ok();

        if !has_new_columns {
            log::info!("Migrating old database schema...");
            if let Err(e) = conn.execute_batch(
                "ALTER TABLE transactions RENAME TO transactions_backup;
                 ALTER TABLE operations RENAME TO operations_backup;",
            ) {
                log::warn!("[migrate] Failed to rename old tables: {}", e);
            }
        }
    }
    Ok(())
}

fn create_tables(conn: &rusqlite::Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS transactions (
            id TEXT PRIMARY KEY,
            batch_id TEXT NOT NULL,
            source_path TEXT NOT NULL,
            dest_path TEXT NOT NULL,
            title TEXT NOT NULL,
            year INTEGER,
            format TEXT NOT NULL,
            genre TEXT NOT NULL,
            media_type TEXT NOT NULL,
            season INTEGER,
            episode INTEGER,
            episode_title TEXT,
            tmdb_id INTEGER,
            poster_url TEXT,
            sha256 TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            undone INTEGER DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_transactions_batch ON transactions(batch_id);
        CREATE INDEX IF NOT EXISTS idx_transactions_format ON transactions(format);
        CREATE INDEX IF NOT EXISTS idx_transactions_genre ON transactions(format, genre);
        CREATE INDEX IF NOT EXISTS idx_transactions_timestamp ON transactions(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_transactions_dest ON transactions(dest_path);

        CREATE TABLE IF NOT EXISTS subtitle_records (
            id TEXT PRIMARY KEY,
            transaction_id TEXT NOT NULL REFERENCES transactions(id),
            source_path TEXT,
            dest_path TEXT NOT NULL,
            language TEXT NOT NULL,
            downloaded INTEGER DEFAULT 0,
            timestamp TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn migrate_to_v3_1(conn: &rusqlite::Connection) -> AppResult<()> {
    let needs_locked = conn
        .prepare("SELECT locked FROM transactions LIMIT 1")
        .is_err();

    if needs_locked {
        conn.execute_batch(
            "ALTER TABLE transactions ADD COLUMN locked INTEGER DEFAULT 0;
             ALTER TABLE transactions ADD COLUMN confidence REAL DEFAULT 0.0;
             CREATE INDEX IF NOT EXISTS idx_transactions_locked ON transactions(locked);",
        )?;
        log::info!("Database migration: added locked + confidence columns");
    }
    Ok(())
}

fn create_trash_table(conn: &rusqlite::Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trash_items (
            id TEXT PRIMARY KEY,
            transaction_id TEXT NOT NULL,
            original_path TEXT NOT NULL,
            trash_path TEXT NOT NULL,
            filename TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_trash_txn ON trash_items(transaction_id);",
    )?;
    Ok(())
}

fn create_watch_progress_table(conn: &rusqlite::Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS watch_progress (
            file_path TEXT PRIMARY KEY,
            position_seconds REAL NOT NULL DEFAULT 0,
            duration_seconds REAL NOT NULL DEFAULT 0,
            completed INTEGER NOT NULL DEFAULT 0,
            last_watched TEXT NOT NULL,
            title TEXT,
            poster_url TEXT,
            media_path TEXT,
            season INTEGER,
            episode INTEGER,
            episode_title TEXT,
            media_type TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_wp_last ON watch_progress(last_watched DESC);
        CREATE INDEX IF NOT EXISTS idx_wp_media ON watch_progress(media_path);
        CREATE INDEX IF NOT EXISTS idx_wp_incomplete ON watch_progress(completed, last_watched DESC);",
    )?;
    Ok(())
}
