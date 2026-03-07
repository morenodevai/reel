/// TMDb API client — search, direct lookup, episode titles, multi-search.

use crate::shared::cache::TypedCache;
use crate::shared::http;
use crate::shared::rate_limiter::TMDB;
use once_cell::sync::Lazy;
use serde::Deserialize;

static METADATA_CACHE: Lazy<TypedCache<String, MetadataResult>> =
    Lazy::new(|| TypedCache::new(2000));

#[derive(Debug, Clone)]
pub struct MetadataResult {
    pub tmdb_id: u64,
    pub title: String,
    pub year: Option<u16>,
    pub genre_ids: Vec<u32>,
    pub poster_url: Option<String>,
    pub origin_country: Vec<String>,
    pub media_type: String, // "movie" or "tv"
    pub overview: Option<String>,
    pub runtime_minutes: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct EpisodeInfo {
    pub name: Option<String>,
    pub episode_number: Option<u32>,
    pub season_number: Option<u32>,
    pub runtime_minutes: Option<u32>,
}

/// A TMDb match for the review UI's picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TmdbMatch {
    pub tmdb_id: u64,
    pub title: String,
    pub year: Option<u16>,
    pub poster_url: Option<String>,
    pub media_type: String,
    pub overview: Option<String>,
}

#[derive(Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbSearchResult>,
}

#[derive(Deserialize)]
struct TmdbSearchResult {
    id: u64,
    title: Option<String>,
    name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    genre_ids: Option<Vec<u32>>,
    poster_path: Option<String>,
    origin_country: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)] // Deserialized from TMDb API; available for future ranking
    popularity: f64,
}

#[derive(Deserialize)]
struct TmdbSeasonResponse {
    episodes: Option<Vec<TmdbEpisodeResult>>,
}

#[derive(Deserialize)]
struct TmdbEpisodeResult {
    name: Option<String>,
    episode_number: Option<u32>,
    season_number: Option<u32>,
    runtime: Option<u32>,
}

/// Search TMDb for a movie or TV show.
pub fn search_tmdb(
    title: &str,
    year: Option<u16>,
    search_type: &str,
    api_key: &str,
) -> Option<MetadataResult> {
    let cache_key = format!("{}:{}:{}", search_type, title, year.unwrap_or(0));
    if let Some(cached) = METADATA_CACHE.get(&cache_key) {
        return Some(cached);
    }

    let endpoint = if search_type == "tv" { "tv" } else { "movie" };
    let mut url = format!(
        "https://api.themoviedb.org/3/search/{}?api_key={}&query={}",
        endpoint,
        api_key,
        urlencoding::encode(title)
    );

    if let Some(y) = year {
        if search_type == "tv" {
            url.push_str(&format!("&first_air_date_year={}", y));
        } else {
            url.push_str(&format!("&year={}", y));
        }
    }

    log::info!(
        "[tmdb] GET {}/{} query='{}' year={:?}",
        endpoint,
        search_type,
        title,
        year
    );
    TMDB.wait();
    let response = match http::client().get(&url).send() {
        Ok(r) => r,
        Err(e) => {
            log::error!("[tmdb] Request failed: {}", e);
            return None;
        }
    };
    if !response.status().is_success() {
        log::error!(
            "[tmdb] HTTP {} for '{}' ({})",
            response.status(),
            title,
            search_type
        );
        return None;
    }
    let search: TmdbSearchResponse = match response.json() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[tmdb] Parse failed: {}", e);
            return None;
        }
    };
    log::info!("[tmdb] {} results for '{}'", search.results.len(), title);

    let best = find_best_match(&search.results, title, year)?;

    let tmdb_title = if search_type == "tv" {
        best.name.clone().unwrap_or_else(|| title.to_string())
    } else {
        best.title.clone().unwrap_or_else(|| title.to_string())
    };

    let date_str = if search_type == "tv" {
        &best.first_air_date
    } else {
        &best.release_date
    };
    let tmdb_year: Option<u16> = date_str
        .as_ref()
        .and_then(|d: &String| d.get(..4))
        .and_then(|y: &str| y.parse().ok());

    let poster_url = best
        .poster_path
        .as_ref()
        .map(|p| format!("{}{}", super::TMDB_POSTER_BASE, p));

    let result = MetadataResult {
        tmdb_id: best.id,
        title: tmdb_title,
        year: tmdb_year.or(year),
        genre_ids: best.genre_ids.clone().unwrap_or_default(),
        poster_url,
        origin_country: best.origin_country.clone().unwrap_or_default(),
        media_type: search_type.to_string(),
        overview: None,
        runtime_minutes: None,
    };

    METADATA_CACHE.put(cache_key, result.clone());
    Some(result)
}

/// Search with fallback strategy: try specific type first, then alternate, then without year.
pub fn search_with_fallback(
    title: &str,
    year: Option<u16>,
    has_episode: bool,
    api_key: &str,
) -> Option<MetadataResult> {
    let primary = if has_episode { "tv" } else { "movie" };
    let secondary = if has_episode { "movie" } else { "tv" };

    if let Some(result) = search_tmdb(title, year, primary, api_key) {
        return Some(result);
    }
    if let Some(result) = search_tmdb(title, year, secondary, api_key) {
        return Some(result);
    }
    if year.is_some() {
        if let Some(result) = search_tmdb(title, None, primary, api_key) {
            return Some(result);
        }
    }
    if year.is_some() {
        if let Some(result) = search_tmdb(title, None, secondary, api_key) {
            return Some(result);
        }
    }
    None
}

/// Get episode title from TMDb.
pub fn get_episode_title(
    tmdb_id: u64,
    season: u16,
    episode: u16,
    api_key: &str,
) -> Option<EpisodeInfo> {
    let url = format!(
        "https://api.themoviedb.org/3/tv/{}/season/{}?api_key={}",
        tmdb_id, season, api_key
    );

    TMDB.wait();
    let response = http::client().get(&url).send().ok()?;
    if !response.status().is_success() {
        log::warn!(
            "[tmdb] Episode lookup failed: HTTP {} for tmdb={} S{}",
            response.status(),
            tmdb_id,
            season
        );
        return None;
    }
    let season_data: TmdbSeasonResponse = response.json().ok()?;

    season_data
        .episodes?
        .into_iter()
        .find(|ep| ep.episode_number == Some(episode as u32))
        .map(|ep| EpisodeInfo {
            name: ep.name,
            episode_number: ep.episode_number,
            season_number: ep.season_number,
            runtime_minutes: ep.runtime,
        })
}

/// Fetch full metadata for a known TMDb ID (direct lookup, not search).
pub fn get_metadata_by_id(
    tmdb_id: u64,
    media_type: &str,
    api_key: &str,
) -> Option<MetadataResult> {
    let cache_key = format!("id:{}:{}", media_type, tmdb_id);
    if let Some(cached) = METADATA_CACHE.get(&cache_key) {
        return Some(cached);
    }

    let endpoint = if media_type == "tv" { "tv" } else { "movie" };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}?api_key={}",
        endpoint, tmdb_id, api_key
    );

    log::info!("[tmdb] GET {}/{} (direct ID lookup)", endpoint, tmdb_id);
    TMDB.wait();
    let resp = http::client().get(&url).send().ok()?;
    if !resp.status().is_success() {
        log::warn!("[tmdb] Direct lookup failed: HTTP {}", resp.status());
        return None;
    }

    let data: serde_json::Value = resp.json().ok()?;

    let title = if media_type == "tv" {
        data["name"].as_str()
    } else {
        data["title"].as_str()
    }?
    .to_string();

    let date_str = if media_type == "tv" {
        data["first_air_date"].as_str()
    } else {
        data["release_date"].as_str()
    };
    let year: Option<u16> = date_str
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse().ok());

    let genre_ids: Vec<u32> = data["genres"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g["id"].as_u64().map(|id| id as u32))
                .collect()
        })
        .unwrap_or_default();

    let poster_url = data["poster_path"]
        .as_str()
        .map(|p| format!("{}{}", super::TMDB_POSTER_BASE, p));

    let origin_country: Vec<String> = data["origin_country"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let overview = data["overview"].as_str().map(|s| s.to_string());

    let runtime_minutes = data["runtime"]
        .as_u64()
        .or_else(|| {
            data["episode_run_time"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_u64())
        })
        .map(|r| r as u32);

    let result = MetadataResult {
        tmdb_id,
        title,
        year,
        genre_ids,
        poster_url,
        origin_country,
        media_type: media_type.to_string(),
        overview,
        runtime_minutes,
    };

    METADATA_CACHE.put(cache_key, result.clone());
    Some(result)
}

/// Multi-search TMDb returning up to 5 movie/tv results for the review picker.
pub fn search_tmdb_multi(query: &str, year: Option<u16>, api_key: &str) -> Vec<TmdbMatch> {
    if api_key.is_empty() || query.is_empty() {
        return Vec::new();
    }

    let mut url = format!(
        "https://api.themoviedb.org/3/search/multi?api_key={}&query={}",
        api_key,
        urlencoding::encode(query)
    );
    if let Some(y) = year {
        url.push_str(&format!("&year={}", y));
    }

    TMDB.wait();
    let response = match http::client().get(&url).send() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let data: serde_json::Value = match response.json() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let results = match data["results"].as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    results
        .iter()
        .filter_map(|r| {
            let media_type = r["media_type"].as_str()?;
            if media_type != "movie" && media_type != "tv" {
                return None;
            }
            let title = if media_type == "tv" {
                r["name"].as_str()
            } else {
                r["title"].as_str()
            }?
            .to_string();

            let date_str = if media_type == "tv" {
                r["first_air_date"].as_str()
            } else {
                r["release_date"].as_str()
            };
            let year: Option<u16> = date_str
                .and_then(|d| d.get(..4))
                .and_then(|y| y.parse().ok());

            let poster_url = r["poster_path"]
                .as_str()
                .map(|p| format!("{}{}", super::TMDB_POSTER_BASE, p));

            let overview = r["overview"].as_str().map(|s| s.to_string());

            Some(TmdbMatch {
                tmdb_id: r["id"].as_u64()?,
                title,
                year,
                poster_url,
                media_type: media_type.to_string(),
                overview,
            })
        })
        .take(5)
        .collect()
}

/// Check if genre IDs contain Animation (16).
pub fn has_animation_genre(genre_ids: &[u32]) -> bool {
    genre_ids.contains(&16)
}

/// Check if origin countries include Japan.
pub fn is_japanese_origin(countries: &[String]) -> bool {
    countries.iter().any(|c| c == "JP")
}

fn find_best_match<'a>(
    results: &'a [TmdbSearchResult],
    query_title: &str,
    year: Option<u16>,
) -> Option<&'a TmdbSearchResult> {
    if results.is_empty() {
        return None;
    }

    let query_lower = query_title.to_lowercase();

    let result_year = |r: &TmdbSearchResult| -> Option<u16> {
        r.release_date
            .as_ref()
            .or(r.first_air_date.as_ref())
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse().ok())
    };

    let result_title = |r: &TmdbSearchResult| -> String {
        r.title
            .as_ref()
            .or(r.name.as_ref())
            .map(|t| t.to_lowercase())
            .unwrap_or_default()
    };

    // 1. Exact title + exact year
    if let Some(y) = year {
        for r in results {
            if result_title(r) == query_lower && result_year(r) == Some(y) {
                return Some(r);
            }
        }
    }

    // 2. Exact title (any year)
    for r in results {
        if result_title(r) == query_lower {
            return Some(r);
        }
    }

    // 3. Short titles (<=5 chars): only accept exact matches
    if query_lower.len() <= 5 {
        return None;
    }

    // 4. Title prefix match with year
    if let Some(y) = year {
        for r in results {
            let rt = result_title(r);
            if (rt.starts_with(&query_lower) || query_lower.starts_with(&rt))
                && result_year(r) == Some(y)
            {
                return Some(r);
            }
        }
    }

    // 5. Significant word overlap
    let query_words: std::collections::HashSet<&str> = query_lower
        .split_whitespace()
        .filter(|w| w.len() > 2 && !matches!(*w, "the" | "and" | "for" | "from"))
        .collect();

    if query_words.is_empty() {
        return None;
    }

    for r in results {
        let rt = result_title(r);
        let result_words: std::collections::HashSet<&str> = rt
            .split_whitespace()
            .filter(|w| w.len() > 2 && !matches!(*w, "the" | "and" | "for" | "from"))
            .collect();

        let overlap = query_words.intersection(&result_words).count();
        let threshold = (query_words.len() + 1) / 2;
        if overlap >= threshold {
            return Some(r);
        }
    }

    // 6. Trust TMDb top result for specific multi-word queries
    if query_words.len() >= 2 && query_lower.len() >= 8 {
        log::info!(
            "[tmdb] Trusting TMDb top result for alternative title query '{}'",
            query_title
        );
        return Some(&results[0]);
    }

    None
}
