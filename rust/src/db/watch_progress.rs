/// Watch progress CRUD — save/load playback position, mark complete/unwatched.

use crate::db::{collect_rows, with_connection};
use crate::error::AppResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WatchProgress {
    pub file_path: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub completed: bool,
    pub last_watched: String,
    pub title: Option<String>,
    pub poster_url: Option<String>,
    pub media_path: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub episode_title: Option<String>,
    pub media_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContinueWatchingItem {
    pub media_path: String,
    pub title: String,
    pub poster_url: Option<String>,
    pub media_type: String,
    pub file_path: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub episode_title: Option<String>,
    pub last_watched: String,
}

const WP_COLS: &str = "file_path, position_seconds, duration_seconds, completed, last_watched, \
    title, poster_url, media_path, season, episode, episode_title, media_type";

fn watch_progress_from_row(row: &rusqlite::Row) -> rusqlite::Result<WatchProgress> {
    Ok(WatchProgress {
        file_path: row.get(0)?,
        position_seconds: row.get(1)?,
        duration_seconds: row.get(2)?,
        completed: row.get::<_, i32>(3)? != 0,
        last_watched: row.get(4)?,
        title: row.get(5)?,
        poster_url: row.get(6)?,
        media_path: row.get(7)?,
        season: row.get::<_, Option<i32>>(8)?.map(|v| v as u32),
        episode: row.get::<_, Option<i32>>(9)?.map(|v| v as u32),
        episode_title: row.get(10)?,
        media_type: row.get(11)?,
    })
}

pub fn update_watch_progress(
    file_path: &str,
    position: f64,
    duration: f64,
    title: Option<&str>,
    poster_url: Option<&str>,
    media_path: Option<&str>,
    season: Option<u32>,
    episode: Option<u32>,
    episode_title: Option<&str>,
    media_type: Option<&str>,
) -> AppResult<()> {
    let completed = if duration > 0.0 { position / duration > 0.9 } else { false };
    let now = chrono::Utc::now().to_rfc3339();
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO watch_progress (file_path, position_seconds, duration_seconds, completed, last_watched, title, poster_url, media_path, season, episode, episode_title, media_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(file_path) DO UPDATE SET
                position_seconds = ?2, duration_seconds = ?3, completed = ?4, last_watched = ?5,
                title = COALESCE(?6, title), poster_url = COALESCE(?7, poster_url),
                media_path = COALESCE(?8, media_path), season = COALESCE(?9, season),
                episode = COALESCE(?10, episode), episode_title = COALESCE(?11, episode_title),
                media_type = COALESCE(?12, media_type)",
            params![
                file_path, position, duration, completed as i32, now,
                title, poster_url, media_path, season, episode, episode_title, media_type,
            ],
        )?;
        Ok(())
    })
}

pub fn get_watch_progress(file_path: &str) -> AppResult<Option<WatchProgress>> {
    with_connection(|conn| {
        conn.query_row(
            &format!("SELECT {} FROM watch_progress WHERE file_path = ?1", WP_COLS),
            params![file_path],
            watch_progress_from_row,
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            _ => Err(e.into()),
        })
    })
}

pub fn get_all_progress_for_media(media_path: &str) -> AppResult<Vec<WatchProgress>> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM watch_progress WHERE media_path = ?1 ORDER BY season ASC, episode ASC",
            WP_COLS
        ))?;
        collect_rows(&mut stmt, params![media_path], watch_progress_from_row)
    })
}

pub fn mark_file_completed(file_path: &str) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    with_connection(|conn| {
        let rows = conn.execute(
            "UPDATE watch_progress SET completed = 1, position_seconds = duration_seconds, last_watched = ?2 WHERE file_path = ?1",
            params![file_path, now],
        )?;
        if rows == 0 {
            conn.execute(
                "INSERT INTO watch_progress (file_path, position_seconds, duration_seconds, completed, last_watched)
                 VALUES (?1, 0, 0, 1, ?2)",
                params![file_path, now],
            )?;
        }
        Ok(())
    })
}

pub fn mark_file_unwatched(file_path: &str) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute("DELETE FROM watch_progress WHERE file_path = ?1", params![file_path])?;
        Ok(())
    })
}

pub fn mark_season_watched(
    media_path: &str,
    _season: u32,
    episode_files: &[String],
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    with_connection(|conn| {
        for fp in episode_files {
            conn.execute(
                "INSERT INTO watch_progress (file_path, position_seconds, duration_seconds, completed, last_watched, media_path)
                 VALUES (?1, 0, 0, 1, ?2, ?3)
                 ON CONFLICT(file_path) DO UPDATE SET completed = 1, position_seconds = duration_seconds, last_watched = ?2",
                params![fp, now, media_path],
            )?;
        }
        Ok(())
    })
}

pub fn mark_season_unwatched(media_path: &str, season: u32) -> AppResult<()> {
    with_connection(|conn| {
        conn.execute(
            "DELETE FROM watch_progress WHERE media_path = ?1 AND season = ?2",
            params![media_path, season],
        )?;
        Ok(())
    })
}

pub fn get_continue_watching(limit: u32) -> AppResult<Vec<ContinueWatchingItem>> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM watch_progress WHERE (completed = 0 AND position_seconds > 0) OR completed = 1
             ORDER BY last_watched DESC",
            WP_COLS
        ))?;

        let all = collect_rows(&mut stmt, [], watch_progress_from_row)?;

        let mut seen_media: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut result: Vec<ContinueWatchingItem> = Vec::new();

        for wp in &all {
            let key = wp.media_path.as_deref().unwrap_or(&wp.file_path).to_string();
            if seen_media.contains(&key) {
                continue;
            }
            seen_media.insert(key.clone());

            if wp.completed && wp.media_type.as_deref() == Some("tv") {
                if let Some(media_path) = &wp.media_path {
                    if let Ok(Some(next)) = get_next_episode(media_path) {
                        result.push(ContinueWatchingItem {
                            media_path: media_path.clone(),
                            title: next.title.unwrap_or_else(|| wp.title.clone().unwrap_or_default()),
                            poster_url: next.poster_url.or_else(|| wp.poster_url.clone()),
                            media_type: "tv".to_string(),
                            file_path: next.file_path.clone(),
                            position_seconds: next.position_seconds,
                            duration_seconds: next.duration_seconds,
                            season: next.season,
                            episode: next.episode,
                            episode_title: next.episode_title,
                            last_watched: wp.last_watched.clone(),
                        });
                        continue;
                    }
                    continue;
                }
            }

            if !wp.completed && wp.position_seconds > 0.0 {
                result.push(ContinueWatchingItem {
                    media_path: wp.media_path.clone().unwrap_or_else(|| wp.file_path.clone()),
                    title: wp.title.clone().unwrap_or_default(),
                    poster_url: wp.poster_url.clone(),
                    media_type: wp.media_type.clone().unwrap_or_else(|| "movie".to_string()),
                    file_path: wp.file_path.clone(),
                    position_seconds: wp.position_seconds,
                    duration_seconds: wp.duration_seconds,
                    season: wp.season,
                    episode: wp.episode,
                    episode_title: wp.episode_title.clone(),
                    last_watched: wp.last_watched.clone(),
                });
            }

            if result.len() >= limit as usize {
                break;
            }
        }

        Ok(result)
    })
}

pub fn get_next_episode(media_path: &str) -> AppResult<Option<WatchProgress>> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM watch_progress WHERE media_path = ?1 ORDER BY season ASC, episode ASC",
            WP_COLS
        ))?;
        let episodes = collect_rows(&mut stmt, params![media_path], watch_progress_from_row)?;

        for ep in &episodes {
            if !ep.completed {
                return Ok(Some(ep.clone()));
            }
        }
        Ok(None)
    })
}
