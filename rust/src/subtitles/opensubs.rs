/// OpenSubtitles API client — search and download subtitles.

use once_cell::sync::Lazy;
use std::path::Path;

/// Download a subtitle for a single video file from OpenSubtitles.
/// Returns the path to the downloaded subtitle on success.
pub fn download_subtitle(
    video_path: &Path,
    lang: &str,
    api_key: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("OpenSubtitles API key not configured".to_string());
    }

    let video_stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let video_dir = video_path.parent().unwrap_or(Path::new("."));

    let client = crate::shared::http::client();

    let os_lang = super::to_os_lang(lang);
    let base = format!(
        "https://api.opensubtitles.com/api/v1/subtitles?languages={}",
        os_lang
    );

    let parent_name = video_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let is_season_dir = parent_name.to_lowercase().starts_with("season");
    let title_dir = if is_season_dir {
        video_path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(video_stem)
    } else {
        parent_name
    };
    let title = title_dir.split('(').next().unwrap_or(title_dir).trim();
    let tmdb_id = extract_tmdb_id(title_dir);
    let is_tv = is_season_dir;

    // Strategy 1: Hash-based search (finds subtitles pre-synced to this exact release)
    let hash_data = hash_search(&client, &base, video_path, api_key);

    // Strategy 2: TMDB/title search (fallback — may need timing sync)
    let data = if let Some(d) = hash_data {
        d
    } else {
        let search_url = if is_tv {
            let (season, episode) = parse_episode_numbers(video_stem);
            log::info!(
                "[subtitle] TMDB search: title='{}' tmdb={:?} S{:?}E{:?} (TV)",
                title, tmdb_id, season, episode
            );
            build_tv_search_url(&base, tmdb_id, title, season, episode)
        } else {
            log::info!(
                "[subtitle] TMDB search: title='{}' tmdb={:?} (movie)",
                title, tmdb_id
            );
            build_movie_search_url(&base, tmdb_id, title)
        };

        match os_search(&client, &search_url, api_key) {
            Some(d) if !d.is_empty() => {
                log::info!("[subtitle] TMDB match for {}", video_stem);
                d
            }
            _ => {
                let fallback_url = format!("{}&query={}", base, urlencoding::encode(title));
                match os_search(&client, &fallback_url, api_key) {
                    Some(d) if !d.is_empty() => {
                        log::info!("[subtitle] Query fallback match for {}", video_stem);
                        d
                    }
                    _ => return Err(format!("No subtitles found for \"{}\"", title)),
                }
            }
        }
    };

    let best = data
        .iter()
        .filter(|s| !s.attributes.files.is_empty())
        .max_by_key(|s| s.attributes.download_count)
        .ok_or_else(|| "No downloadable subtitles found".to_string())?;

    let (bytes, filename) = os_download(&client, api_key, best.attributes.files[0].file_id)?;
    let sub_lang = &best.attributes.language;
    let ext = filename.rsplit('.').next().unwrap_or("srt");
    let sub_path = video_dir.join(format!("{}.{}.{}", video_stem, sub_lang, ext));

    std::fs::write(&sub_path, &bytes)
        .map_err(|e| format!("Failed to write subtitle: {}", e))?;

    if let Err(e) = super::sync::sync_subtitle(video_path, &sub_path) {
        log::warn!("[subtitle] Sync failed for {}: {}", sub_path.display(), e);
    }

    Ok(sub_path.to_string_lossy().to_string())
}

/// Search OpenSubtitles for a title directory and download subtitles.
/// For movies: downloads one subtitle.
/// For TV shows: downloads subtitles for ALL episodes, skipping any that already have one.
pub fn search_and_download(
    title_dir: &Path,
    languages: &[String],
    api_key: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("OpenSubtitles API key not configured — set it in Settings".to_string());
    }

    let dir_name = title_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let tmdb_id = extract_tmdb_id(dir_name);
    let title = dir_name.split('(').next().unwrap_or(dir_name).trim();

    let format = title_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Movies");
    let media_type = crate::shared::video::format_to_media_type(format);

    let videos = find_all_videos_in_dir(title_dir);
    if videos.is_empty() {
        return Err("No video files found in this directory".to_string());
    }

    let client = crate::shared::http::client();

    let lang_param = if languages.is_empty() {
        "en".to_string()
    } else {
        languages
            .iter()
            .map(|l| super::to_os_lang(l))
            .collect::<Vec<_>>()
            .join(",")
    };
    let base = format!(
        "https://api.opensubtitles.com/api/v1/subtitles?languages={}",
        lang_param
    );

    let mut downloaded = 0u32;
    let mut synced = 0u32;
    let mut failed = 0u32;

    for video in &videos {
        let video_stem = video
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let video_dir = video.parent().unwrap_or(title_dir);

        // If subtitle already exists, sync it instead of re-downloading
        if let Some(sub_path) = find_first_subtitle(video_dir, video_stem) {
            match super::sync::sync_subtitle(video, &sub_path) {
                Ok(()) => {
                    log::info!("[subtitle] Synced existing sub: {}", sub_path.display());
                    synced += 1;
                }
                Err(e) => {
                    log::warn!("[subtitle] Sync failed for {}: {}", sub_path.display(), e);
                    failed += 1;
                }
            }
            continue;
        }

        // Strategy 1: Hash-based search (pre-synced to exact release).
        // Skip when TMDB ID is known — TMDB search is reliable and sync_subtitle
        // handles timing. Avoids extra API call per episode for full seasons.
        let hash_data = if tmdb_id.is_some() {
            None
        } else {
            hash_search(&client, &base, video, api_key)
        };

        // Strategy 2: TMDB/title search (fallback)
        let data = if let Some(d) = hash_data {
            d
        } else {
            let search_url = if media_type == "tv" {
                let (season, episode) = parse_episode_numbers(video_stem);
                build_tv_search_url(&base, tmdb_id, title, season, episode)
            } else {
                build_movie_search_url(&base, tmdb_id, title)
            };

            match os_search(&client, &search_url, api_key) {
                Some(d) if !d.is_empty() => {
                    log::info!("[subtitle] TMDB match for {}", video_stem);
                    d
                }
                _ => {
                    let fallback_url = format!("{}&query={}", base, urlencoding::encode(title));
                    match os_search(&client, &fallback_url, api_key) {
                        Some(d) if !d.is_empty() => {
                            log::info!("[subtitle] Query fallback match for {}", video_stem);
                            d
                        }
                        _ => {
                            failed += 1;
                            continue;
                        }
                    }
                }
            }
        };

        let best = match data
            .iter()
            .filter(|s| !s.attributes.files.is_empty())
            .max_by_key(|s| s.attributes.download_count)
        {
            Some(b) => b,
            None => {
                failed += 1;
                continue;
            }
        };

        match os_download(&client, api_key, best.attributes.files[0].file_id) {
            Ok((bytes, filename)) => {
                let lang = &best.attributes.language;
                let ext = filename.rsplit('.').next().unwrap_or("srt");
                let sub_path = video_dir.join(format!("{}.{}.{}", video_stem, lang, ext));
                match std::fs::write(&sub_path, &bytes) {
                    Ok(()) => {
                        if let Err(e) = super::sync::sync_subtitle(video, &sub_path) {
                            log::warn!("[subtitle] Sync failed for {}: {}", sub_path.display(), e);
                        }
                        downloaded += 1;
                    }
                    Err(e) => {
                        log::warn!("[subtitle] Failed to write {}: {}", sub_path.display(), e);
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                log::warn!("[subtitle] Download failed for {}: {}", video_stem, e);
                failed += 1;
            }
        }
    }

    if downloaded == 0 && synced == 0 {
        return Err(format!("No subtitles found for \"{}\"", title));
    }

    let mut parts = Vec::new();
    if downloaded > 0 {
        parts.push(format!("{} downloaded", downloaded));
    }
    if synced > 0 {
        parts.push(format!("{} synced", synced));
    }
    if failed > 0 {
        parts.push(format!("{} failed", failed));
    }
    Ok(parts.join(", "))
}

// === Internal helpers ===

/// Hash-based subtitle search — finds subtitles pre-synced to the exact video release.
/// Returns None if hash computation fails or no results found (callers fall back to TMDB/title).
fn hash_search(
    client: &reqwest::blocking::Client,
    base: &str,
    video_path: &Path,
    api_key: &str,
) -> Option<Vec<OsSubtitle>> {
    let (hash, file_size) = super::compute_os_hash(video_path)
        .map_err(|e| log::debug!("[subtitle] Hash computation failed for {}: {}", video_path.display(), e))
        .ok()?;
    let stem = video_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    log::info!("[subtitle] Hash search: hash={} size={} for {}", hash, file_size, stem);
    let hash_url = format!("{}&moviehash={}&moviebytesize={}", base, hash, file_size);
    let results = os_search(client, &hash_url, api_key)?;
    if results.is_empty() {
        log::info!("[subtitle] Hash search returned 0 results for {}", stem);
        return None;
    }
    log::info!("[subtitle] Hash match: {} results for {} (pre-synced to release)", results.len(), stem);
    Some(results)
}

#[derive(serde::Deserialize)]
struct OsSearchResponse {
    data: Vec<OsSubtitle>,
}

#[derive(serde::Deserialize)]
struct OsSubtitle {
    attributes: OsAttributes,
}

#[derive(serde::Deserialize)]
struct OsAttributes {
    language: String,
    download_count: u64,
    files: Vec<OsFile>,
}

#[derive(serde::Deserialize)]
struct OsFile {
    file_id: u64,
}

#[derive(serde::Deserialize)]
struct OsDownloadResponse {
    link: String,
    file_name: String,
}

fn find_all_videos_in_dir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut videos = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && crate::shared::video::is_video_file(path) {
            videos.push(path.to_path_buf());
        }
    }
    videos
}

/// Find the first existing subtitle file for a video (for re-syncing).
fn find_first_subtitle(dir: &Path, video_stem: &str) -> Option<std::path::PathBuf> {
    // Check direct stem.ext matches first
    for ext in super::SUBTITLE_EXTENSIONS {
        let path = dir.join(format!("{}.{}", video_stem, ext));
        if path.exists() {
            return Some(path);
        }
    }
    // Check stem.lang.ext pattern
    let prefix = format!("{}.", video_stem);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                for ext in super::SUBTITLE_EXTENSIONS {
                    if name.ends_with(&format!(".{}", ext)) {
                        return Some(entry.path());
                    }
                }
            }
        }
    }
    None
}

fn parse_episode_numbers(stem: &str) -> (Option<u32>, Option<u32>) {
    static EP_RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap());
    static EP_NX_NN_RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"(?i)\b(\d{1,2})x(\d{1,3})\b").unwrap());
    if let Some(caps) = EP_RE.captures(stem) {
        (
            caps.get(1).and_then(|m| m.as_str().parse().ok()),
            caps.get(2).and_then(|m| m.as_str().parse().ok()),
        )
    } else if let Some(caps) = EP_NX_NN_RE.captures(stem) {
        (
            caps.get(1).and_then(|m| m.as_str().parse().ok()),
            caps.get(2).and_then(|m| m.as_str().parse().ok()),
        )
    } else {
        (None, None)
    }
}

fn build_tv_search_url(
    base: &str,
    tmdb_id: Option<u64>,
    title: &str,
    season: Option<u32>,
    episode: Option<u32>,
) -> String {
    let mut url = base.to_string();
    if let Some(id) = tmdb_id {
        url.push_str(&format!("&parent_tmdb_id={}", id));
    } else {
        url.push_str(&format!("&query={}", urlencoding::encode(title)));
    }
    if let Some(s) = season {
        url.push_str(&format!("&season_number={}", s));
    }
    if let Some(e) = episode {
        url.push_str(&format!("&episode_number={}", e));
    }
    url
}

fn build_movie_search_url(base: &str, tmdb_id: Option<u64>, title: &str) -> String {
    let mut url = base.to_string();
    if let Some(id) = tmdb_id {
        url.push_str(&format!("&tmdb_id={}&type=movie", id));
    } else {
        url.push_str(&format!("&query={}", urlencoding::encode(title)));
    }
    url
}

fn os_search(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
) -> Option<Vec<OsSubtitle>> {
    crate::shared::rate_limiter::OPENSUBS.wait();
    let resp = client
        .get(url)
        .header("Api-Key", api_key)
        .header("Accept", "*/*")
        .header("Content-Type", "application/json")
        .header("User-Agent", crate::shared::http::USER_AGENT)
        .send()
        .map_err(|e| log::debug!("[subtitle] Search request failed for {}: {}", url, e))
        .ok()?;
    if !resp.status().is_success() {
        log::warn!(
            "[subtitle] Search failed: HTTP {} for {}",
            resp.status(),
            url
        );
        return None;
    }
    resp.json::<OsSearchResponse>()
        .map_err(|e| log::debug!("[subtitle] Failed to parse search response: {}", e))
        .ok()
        .map(|r| r.data)
}

fn os_download(
    client: &reqwest::blocking::Client,
    api_key: &str,
    file_id: u64,
) -> Result<(Vec<u8>, String), String> {
    crate::shared::rate_limiter::OPENSUBS.wait();
    let resp = client
        .post("https://api.opensubtitles.com/api/v1/download")
        .header("Api-Key", api_key)
        .header("Accept", "*/*")
        .header("Content-Type", "application/json")
        .header("User-Agent", crate::shared::http::USER_AGENT)
        .json(&serde_json::json!({ "file_id": file_id }))
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if body.contains("\"remaining\":0") || body.contains("your allowed") {
            let reset = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| {
                    v.get("reset_time")
                        .and_then(|r| r.as_str().map(String::from))
                })
                .unwrap_or_else(|| "~24 hours".to_string());
            return Err(format!(
                "QUOTA_EXHAUSTED: Daily subtitle download limit reached. Resets in {}",
                reset
            ));
        }
        return Err(format!("HTTP {} {}", status, body));
    }
    let dl: OsDownloadResponse = resp.json().map_err(|e| e.to_string())?;
    let bytes = client
        .get(&dl.link)
        .send()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    Ok((bytes.to_vec(), dl.file_name))
}

fn extract_tmdb_id(name: &str) -> Option<u64> {
    static RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"\[tmdbid-(\d+)\]").unwrap());
    RE.captures(name)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}
