/// qBittorrent Web API v2 integration.
/// Connects to a running qBittorrent instance to list completed torrents
/// and import their files through the classification pipeline.
/// Includes a background poller that auto-imports new completed torrents.

use crate::config::{self, Config, QbitConfig};
use crate::pipeline;
use crate::renamer;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct QbitTestResult {
    pub connected: bool,
    pub version: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentInfo {
    pub name: String,
    pub hash: String,
    pub save_path: String,
    pub size: u64,
    pub files: Vec<String>,
}

// === qBittorrent Web API v2 response types ===

#[derive(Deserialize)]
struct QbitTorrent {
    name: String,
    hash: String,
    save_path: String,
    size: u64,
    #[serde(default)]
    content_path: String,
}

#[derive(Deserialize)]
struct QbitFile {
    name: String,
}

// === Client helper ===

fn build_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn base_url(config: &QbitConfig) -> String {
    let host = if config.host.is_empty() {
        "localhost"
    } else {
        &config.host
    };
    let host = host.trim_end_matches('/');

    // If host already looks like a full URL with port (e.g. "http://host:9090"), use as-is
    if host.starts_with("http://") || host.starts_with("https://") {
        // Check if port is already present in the URL
        let after_scheme = host.split("://").nth(1).unwrap_or(host);
        if after_scheme.contains(':') {
            // Port already in URL — use host as-is
            host.to_string()
        } else {
            format!("{}:{}", host, config.port)
        }
    } else if host.contains(':') {
        // Bare host:port without scheme
        format!("http://{}", host)
    } else {
        format!("http://{}:{}", host, config.port)
    }
}

fn login(client: &reqwest::blocking::Client, config: &QbitConfig) -> Result<(), String> {
    let url = format!("{}/api/v2/auth/login", base_url(config));
    let resp = client
        .post(&url)
        .header("Referer", &base_url(config))
        .form(&[
            ("username", config.username.as_str()),
            ("password", config.password.as_str()),
        ])
        .send()
        .map_err(|e| format!("Connection failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Login failed with HTTP {}", resp.status()));
    }

    let body = resp.text().unwrap_or_default();
    if body.contains("Fails") || body.contains("fail") {
        return Err("Login failed — check username and password".to_string());
    }

    Ok(())
}

// === Public API ===

pub fn test_connection(config: &QbitConfig) -> Result<QbitTestResult, String> {
    let client = build_client()?;

    // Try to login
    if let Err(e) = login(&client, config) {
        return Ok(QbitTestResult {
            connected: false,
            version: String::new(),
            message: e,
        });
    }

    // Get version
    let version_url = format!("{}/api/v2/app/version", base_url(config));
    let version = client
        .get(&version_url)
        .send()
        .ok()
        .and_then(|r| r.text().ok())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(QbitTestResult {
        connected: true,
        version: version.clone(),
        message: format!("Connected to qBittorrent {}", version),
    })
}

pub fn auto_detect() -> Result<QbitConfig, String> {
    let candidates = vec![
        ("localhost", 8080u16, "admin", "adminadmin"),
        ("localhost", 8080, "", ""),
        ("127.0.0.1", 8080, "admin", "adminadmin"),
        ("127.0.0.1", 8080, "", ""),
        ("localhost", 9090, "admin", "adminadmin"),
    ];

    let client = reqwest::blocking::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    for (host, port, user, pass) in &candidates {
        let config = QbitConfig {
            host: host.to_string(),
            port: *port,
            username: user.to_string(),
            password: pass.to_string(),
            auto_remove: false,
        };

        if login(&client, &config).is_ok() {
            return Ok(config);
        }
    }

    Err("Could not find a running qBittorrent instance on common ports".to_string())
}

pub fn get_completed_torrents(config: &QbitConfig) -> Result<Vec<TorrentInfo>, String> {
    let client = build_client()?;
    login(&client, config)?;

    let url = format!(
        "{}/api/v2/torrents/info?filter=completed",
        base_url(config)
    );
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to fetch torrents: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let torrents: Vec<QbitTorrent> = resp
        .json()
        .map_err(|e| format!("Failed to parse torrent list: {}", e))?;

    let mut results = Vec::new();
    for t in &torrents {
        // Get file list for each torrent
        let files = get_torrent_files(&client, config, &t.hash)?;

        results.push(TorrentInfo {
            name: t.name.clone(),
            hash: t.hash.clone(),
            save_path: t.save_path.clone(),
            size: t.size,
            files,
        });
    }

    Ok(results)
}

fn get_torrent_files(
    client: &reqwest::blocking::Client,
    config: &QbitConfig,
    hash: &str,
) -> Result<Vec<String>, String> {
    let url = format!(
        "{}/api/v2/torrents/files?hash={}",
        base_url(config),
        hash
    );
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to fetch torrent files: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let files: Vec<QbitFile> = resp
        .json()
        .map_err(|e| format!("Failed to parse file list: {}", e))?;

    Ok(files.into_iter().map(|f| f.name).collect())
}

pub fn import_torrent(hash: &str, config: &Config) -> Result<pipeline::BatchResult, String> {
    let qbit = &config.qbittorrent;
    let client = build_client()?;
    login(&client, qbit)?;

    // Get torrent info
    let url = format!(
        "{}/api/v2/torrents/info?hashes={}",
        base_url(qbit),
        hash
    );
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to fetch torrent: {}", e))?;

    let torrents: Vec<QbitTorrent> = resp
        .json()
        .map_err(|e| format!("Failed to parse torrent: {}", e))?;

    let torrent = torrents
        .first()
        .ok_or_else(|| format!("Torrent {} not found", hash))?;

    // Get file list
    let files = get_torrent_files(&client, qbit, hash)?;

    // Build full paths and filter for video files
    let save_path = PathBuf::from(&torrent.save_path);
    let content_path = if torrent.content_path.is_empty() {
        save_path.clone()
    } else {
        PathBuf::from(&torrent.content_path)
    };

    let video_paths: Vec<String> = files
        .iter()
        .map(|f| {
            // Files are relative to save_path
            let full = save_path.join(f);
            if full.exists() {
                full
            } else {
                // Some versions use content_path as base
                content_path.join(f)
            }
        })
        .filter(|p| p.exists() && renamer::is_video_file(p))
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    if video_paths.is_empty() {
        return Err("No video files found in this torrent".to_string());
    }

    // Pause the torrent before importing (prevent re-seeding issues)
    let pause_url = format!("{}/api/v2/torrents/pause", base_url(qbit));
    client
        .post(&pause_url)
        .form(&[("hashes", hash)])
        .send()
        .ok();

    // Run through the pipeline: scan → analyze → process
    let video_files = pipeline::scan_video_files(&video_paths);

    let library = config
        .library_path
        .as_ref()
        .ok_or("No library path configured")?;
    let api_key = &config.tmdb_api_key;
    let opensubs_key = &config.opensubs_api_key;

    let mut analyses = Vec::new();
    for video_path in &video_files {
        let filename = video_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Reuse the same analysis function the drop pipeline uses
        let result = pipeline::analyze_single_file_pub(video_path, &filename, api_key, opensubs_key, library);
        analyses.push(result);
    }

    // Process: move files to library
    let batch_id = uuid::Uuid::new_v4().to_string();
    let mut succeeded = 0u32;
    let mut failed = 0u32;
    let mut errors = Vec::new();

    let cleanup_root = save_path.to_string_lossy().to_string();
    for analysis in &analyses {
        match pipeline::process_single_file_pub(
            analysis,
            &batch_id,
            config.auto_download_subs,
            &config.subtitle_languages,
            &config.opensubs_api_key,
            &cleanup_root,
        ) {
            Ok(()) => succeeded += 1,
            Err(e) => {
                failed += 1;
                errors.push(pipeline::FileError {
                    path: analysis.source_path.clone(),
                    error: e,
                });
            }
        }
    }

    // If auto_remove is enabled and all files succeeded, remove torrent from qBittorrent
    if qbit.auto_remove && failed == 0 {
        let delete_url = format!("{}/api/v2/torrents/delete", base_url(qbit));
        client
            .post(&delete_url)
            .form(&[("hashes", hash), ("deleteFiles", "false")])
            .send()
            .ok();
    }

    Ok(pipeline::BatchResult {
        batch_id,
        succeeded,
        failed,
        errors,
    })
}

// === Auto-Poller: background thread that checks for new completed torrents ===

static POLLER_RUNNING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static POLLER_STOP: Lazy<Mutex<Option<Arc<AtomicBool>>>> = Lazy::new(|| Mutex::new(None));

/// Path to the file tracking already-processed torrent hashes.
fn processed_hashes_path() -> PathBuf {
    config::config_dir().join("processed_torrents.txt")
}

/// Load hashes of already-processed torrents from disk.
fn load_processed_hashes() -> HashSet<String> {
    let path = processed_hashes_path();
    let mut set = HashSet::new();
    if let Ok(file) = std::fs::File::open(&path) {
        for line in std::io::BufReader::new(file).lines() {
            if let Ok(hash) = line {
                let hash = hash.trim().to_string();
                if !hash.is_empty() {
                    set.insert(hash);
                }
            }
        }
    }
    set
}

/// Append a processed hash to disk.
fn save_processed_hash(hash: &str) {
    let path = processed_hashes_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{}", hash);
    }
}

/// Start the qBittorrent auto-poller.
/// Polls every 30 seconds for newly completed torrents and imports them.
pub fn start_poller<F>(config: Config, emit: F) -> Result<(), String>
where
    F: Fn(&str, serde_json::Value) + Send + Sync + 'static,
{
    // Must have library path
    if config.library_path.is_none() {
        return Err("No library path configured".to_string());
    }

    // Must have qBit configured -- try auto-detect if not
    let config = if config.qbittorrent.host.is_empty() && config.qbittorrent.username.is_empty() {
        let detected = auto_detect().map_err(|_| {
            "qBittorrent not configured and auto-detect failed".to_string()
        })?;
        let mut cfg = config;
        cfg.qbittorrent = detected.clone();
        // Don't persist auto-detected default credentials (admin/adminadmin) to config file.
        // The poller will re-detect each session. User must explicitly save credentials via UI.
        log::info!("qBit poller: auto-detected qBittorrent at {}:{} (not persisting to config)", detected.host, detected.port);
        cfg
    } else {
        config
    };

    // Test connection
    let test = test_connection(&config.qbittorrent)?;
    if !test.connected {
        return Err(format!("Cannot connect to qBittorrent: {}", test.message));
    }

    // Stop existing poller
    if POLLER_RUNNING.load(Ordering::SeqCst) {
        stop_poller()?;
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    if let Ok(mut guard) = POLLER_STOP.lock() {
        *guard = Some(stop_flag.clone());
    }
    POLLER_RUNNING.store(true, Ordering::SeqCst);

    let emit = Arc::new(emit);

    std::thread::spawn(move || {
        log::info!("qBit poller: started — polling every 30s");

        // Load already-processed hashes (persisted across restarts)
        let mut processed = load_processed_hashes();
        log::info!("qBit poller: {} previously processed torrents", processed.len());

        while !stop_flag_clone.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(30));

            if stop_flag_clone.load(Ordering::SeqCst) {
                break;
            }

            // Poll for completed torrents
            let completed = match get_completed_torrents(&config.qbittorrent) {
                Ok(list) => list,
                Err(e) => {
                    log::warn!("qBit poller: failed to fetch torrents: {}", e);
                    continue;
                }
            };

            // Find torrents we haven't processed yet
            let new_torrents: Vec<&TorrentInfo> = completed
                .iter()
                .filter(|t| !processed.contains(&t.hash))
                .collect();

            if new_torrents.is_empty() {
                log::debug!("qBit poller: {} completed, 0 new", completed.len());
                continue;
            }

            log::info!(
                "qBit poller: {} unprocessed completed torrent(s) found",
                new_torrents.len()
            );

            for torrent in new_torrents {
                let hash = torrent.hash.clone();
                let name = torrent.name.clone();

                // Check if any video files still exist at the save path
                // (if already moved to library, skip and mark as processed)
                let save_dir = PathBuf::from(&torrent.save_path);
                let has_video_files = torrent.files.iter().any(|f| {
                    let full = save_dir.join(f);
                    full.exists() && renamer::is_video_file(&full)
                });

                if !has_video_files {
                    log::info!(
                        "qBit poller: '{}' — no video files at save path, marking as processed",
                        name
                    );
                    processed.insert(hash.clone());
                    save_processed_hash(&hash);
                    continue;
                }

                log::info!("qBit poller: importing '{}'", name);

                match import_torrent(&hash, &config) {
                    Ok(result) => {
                        log::info!(
                            "qBit poller: imported '{}' — {} succeeded, {} failed",
                            name, result.succeeded, result.failed
                        );

                        // Emit event to frontend
                        emit(
                            "qbit-imported",
                            serde_json::json!({
                                "name": name,
                                "succeeded": result.succeeded,
                                "failed": result.failed,
                            }),
                        );
                    }
                    Err(e) => {
                        log::warn!("qBit poller: failed to import '{}': {}", name, e);
                    }
                }

                // Mark as processed (don't retry every 30s)
                processed.insert(hash.clone());
                save_processed_hash(&hash);
            }
        }

        log::info!("qBit poller: stopped");
    });

    log::info!("qBit poller: monitoring for new completed torrents");
    Ok(())
}

/// Stop the qBittorrent auto-poller.
pub fn stop_poller() -> Result<(), String> {
    if let Ok(mut guard) = POLLER_STOP.lock() {
        if let Some(flag) = guard.take() {
            flag.store(true, Ordering::SeqCst);
        }
    }
    POLLER_RUNNING.store(false, Ordering::SeqCst);
    log::info!("qBit poller: stop requested");
    Ok(())
}

/// Check if the qBittorrent poller is running.
pub fn is_poller_running() -> bool {
    POLLER_RUNNING.load(Ordering::SeqCst)
}
