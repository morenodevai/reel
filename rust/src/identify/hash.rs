/// OpenSubtitles hash-based media identification.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Result of identifying a media file via OpenSubtitles hash lookup.
#[derive(Debug, Clone)]
pub struct HashIdentification {
    pub title: String,
    pub year: Option<u16>,
    pub imdb_id: Option<u64>,
    pub tmdb_id: Option<u64>,
    pub feature_type: String,
    /// For episodes: the series name
    pub parent_title: Option<String>,
    /// For episodes: the series TMDb ID
    pub parent_tmdb_id: Option<u64>,
    /// For episodes: season number
    pub season_number: Option<u16>,
    /// For episodes: episode number
    pub episode_number: Option<u16>,
}

/// Compute the OpenSubtitles hash (oshash / moviehash) for a video file.
pub fn compute_os_hash(path: &Path) -> Result<(String, u64), String> {
    const CHUNK_SIZE: u64 = 65536;

    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let file_size = file
        .metadata()
        .map_err(|e| format!("Failed to get file metadata: {}", e))?
        .len();

    if file_size < CHUNK_SIZE {
        return Err("File too small for hash computation".to_string());
    }

    let mut hash: u64 = file_size;

    let mut buf = [0u8; 65536];
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Seek error: {}", e))?;
    file.read_exact(&mut buf)
        .map_err(|e| format!("Read error: {}", e))?;
    for chunk in buf.chunks_exact(8) {
        let val = u64::from_le_bytes(chunk.try_into().unwrap());
        hash = hash.wrapping_add(val);
    }

    file.seek(SeekFrom::End(-(CHUNK_SIZE as i64)))
        .map_err(|e| format!("Seek error: {}", e))?;
    file.read_exact(&mut buf)
        .map_err(|e| format!("Read error: {}", e))?;
    for chunk in buf.chunks_exact(8) {
        let val = u64::from_le_bytes(chunk.try_into().unwrap());
        hash = hash.wrapping_add(val);
    }

    Ok((format!("{:016x}", hash), file_size))
}

/// Identify a media file using OpenSubtitles hash lookup.
pub fn identify_by_hash(path: &Path, api_key: &str) -> Option<HashIdentification> {
    if api_key.is_empty() {
        return None;
    }

    let (hash, file_size) = compute_os_hash(path).ok()?;
    log::info!(
        "[identify] OpenSubtitles hash: {} size: {} for {}",
        hash,
        file_size,
        path.display()
    );

    crate::shared::rate_limiter::OPENSUBS.wait();

    let url = format!(
        "https://api.opensubtitles.com/api/v1/subtitles?moviehash={}&moviebytesize={}",
        hash, file_size
    );

    let resp = crate::shared::http::client()
        .get(&url)
        .header("Api-Key", api_key)
        .header("Accept", "*/*")
        .header("Content-Type", "application/json")
        .header("User-Agent", crate::shared::http::USER_AGENT)
        .send()
        .ok()?;

    if !resp.status().is_success() {
        log::warn!(
            "[identify] OpenSubtitles hash lookup failed: HTTP {}",
            resp.status()
        );
        return None;
    }

    let body: serde_json::Value = resp.json().ok()?;
    let data = body.get("data")?.as_array()?;

    if data.is_empty() {
        log::info!("[identify] No OpenSubtitles results for hash {}", hash);
        return None;
    }

    let first = &data[0];
    let attrs = first.get("attributes")?;
    let feature = attrs.get("feature_details")?;

    let title = feature
        .get("title")
        .or_else(|| feature.get("movie_name"))
        .or_else(|| feature.get("parent_title"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())?;

    let year = feature
        .get("year")
        .and_then(|v| v.as_u64())
        .and_then(|y| u16::try_from(y).ok());

    let imdb_id = feature.get("imdb_id").and_then(|v| v.as_u64());
    let tmdb_id = feature.get("tmdb_id").and_then(|v| v.as_u64());

    let feature_type = feature
        .get("feature_type")
        .and_then(|v| v.as_str())
        .unwrap_or("Movie")
        .to_string();

    let parent_title = feature
        .get("parent_title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let parent_tmdb_id = feature.get("parent_tmdb_id").and_then(|v| v.as_u64());
    let season_number = feature
        .get("season_number")
        .and_then(|v| v.as_u64())
        .and_then(|n| u16::try_from(n).ok());
    let episode_number = feature
        .get("episode_number")
        .and_then(|v| v.as_u64())
        .and_then(|n| u16::try_from(n).ok());

    log::info!(
        "[identify] Hash match: '{}' ({:?}) tmdb={:?} type={} parent='{:?}' parent_tmdb={:?} S{:?}E{:?}",
        title, year, tmdb_id, feature_type, parent_title, parent_tmdb_id, season_number, episode_number
    );

    Some(HashIdentification {
        title,
        year,
        imdb_id,
        tmdb_id,
        feature_type,
        parent_title,
        parent_tmdb_id,
        season_number,
        episode_number,
    })
}
