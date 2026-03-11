/// Poster URL resolution via TMDb API.

use crate::shared::cache::TypedCache;
use crate::shared::http;
use crate::shared::rate_limiter::TMDB;
use once_cell::sync::Lazy;

static POSTER_CACHE: Lazy<TypedCache<String, String>> = Lazy::new(|| TypedCache::new(5000));

/// Fetch poster URL for a known TMDb ID.
/// `media_type` must be "tv" or "movie".
pub fn get_poster_by_tmdb_id(tmdb_id: u64, api_key: &str, media_type: &str) -> Option<String> {
    let cache_key = format!("{}:{}", tmdb_id, media_type);

    if let Some(url) = POSTER_CACHE.get(&cache_key) {
        return Some(url);
    }

    let endpoint = if media_type == "tv" { "tv" } else { "movie" };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}?api_key={}",
        endpoint, tmdb_id, api_key
    );

    TMDB.wait();
    let resp = http::client().get(&url).send()
        .map_err(|e| log::debug!("[poster] TMDb request failed for {}/{}: {}", media_type, tmdb_id, e))
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let data: serde_json::Value = resp.json()
        .map_err(|e| log::debug!("[poster] Failed to parse TMDb response for {}/{}: {}", media_type, tmdb_id, e))
        .ok()?;
    let poster_path = data["poster_path"].as_str()?;
    let poster_url = format!("{}{}", super::TMDB_POSTER_BASE, poster_path);
    POSTER_CACHE.put(cache_key, poster_url.clone());
    Some(poster_url)
}
