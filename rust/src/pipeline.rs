use crate::classifier;
use crate::config::Config;
use crate::junk;
use crate::metadata;
use crate::renamer;
use crate::subtitles;
use crate::transaction;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

static PIPELINE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
/// Blocking mutex for synchronous callers (watcher, qBittorrent import).
/// Both this and PIPELINE_LOCK must be acquired to prevent concurrent file moves.
pub(crate) static PIPELINE_SYNC_LOCK: Lazy<std::sync::Mutex<()>> = Lazy::new(|| std::sync::Mutex::new(()));
static CANCEL_FLAG: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// Guard to prevent duplicate background batch processing (drag events fire multiple times).
static BG_PROCESSING: AtomicBool = AtomicBool::new(false);

/// Try to acquire the background processing guard. Returns true if acquired, false if already running.
pub fn try_start_bg_processing() -> bool {
    BG_PROCESSING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok()
}

/// Release the background processing guard.
pub fn finish_bg_processing() {
    BG_PROCESSING.store(false, Ordering::SeqCst);
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisResult {
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
    pub subtitle_files: Vec<String>,
    pub junk_files: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchResult {
    pub batch_id: String,
    pub succeeded: u32,
    pub failed: u32,
    pub errors: Vec<FileError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileError {
    pub path: String,
    pub error: String,
}

/// Scan paths and find all video files (recursively for directories).
pub fn scan_video_files(paths: &[String]) -> Vec<PathBuf> {
    let mut video_files = Vec::new();

    for path_str in paths {
        log::info!("[scan] Scanning path: {}", path_str);
        let path = Path::new(path_str);
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() && renamer::is_video_file(entry.path()) {
                    log::info!("[scan] Found video: {}", entry.path().display());
                    video_files.push(entry.path().to_path_buf());
                }
            }
        } else if path.is_file() && renamer::is_video_file(path) {
            log::info!("[scan] Found video: {}", path.display());
            video_files.push(path.to_path_buf());
        }
    }

    log::info!("[scan] Total video files found: {}", video_files.len());
    video_files
}

/// Analyze files without moving them. Returns previews.
pub async fn analyze_files(
    paths: Vec<String>,
    config: &Config,
) -> Result<Vec<AnalysisResult>, String> {
    let _guard = PIPELINE_LOCK.lock().await;

    let library = config
        .library_path
        .as_ref()
        .ok_or("No library path configured")?;

    let video_files = tokio::task::spawn_blocking({
        let paths = paths.clone();
        move || scan_video_files(&paths)
    })
    .await
    .map_err(|e| format!("Scan task failed: {}", e))?;

    let mut results = Vec::new();
    let api_key = config.tmdb_api_key.clone();
    let opensubs_key = config.opensubs_api_key.clone();
    let library = library.clone();

    for video_path in video_files {
        let filename = video_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let api_key = api_key.clone();
        let opensubs_key = opensubs_key.clone();
        let library = library.clone();
        let video_path_clone = video_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            analyze_single_file(&video_path_clone, &filename, &api_key, &opensubs_key, &library)
        })
        .await
        .map_err(|e| format!("Analysis task failed: {}", e))?;

        results.push(result);
    }

    Ok(results)
}

/// Public entry point for analyzing a single file (used by qBittorrent import).
pub fn analyze_single_file_pub(
    video_path: &Path,
    filename: &str,
    api_key: &str,
    opensubs_key: &str,
    library: &str,
) -> AnalysisResult {
    analyze_single_file(video_path, filename, api_key, opensubs_key, library)
}

fn analyze_single_file(
    video_path: &Path,
    filename: &str,
    api_key: &str,
    opensubs_key: &str,
    library: &str,
) -> AnalysisResult {
    log::info!("[analyze] === Analyzing: {} ===", video_path.display());

    // 1. Parse filename
    let parsed = renamer::parse_filename(filename);
    log::info!("[analyze] Filename parsed: title='{}' year={:?} S{:?}E{:?}",
        parsed.title, parsed.year, parsed.season, parsed.episode);

    // 1.5. Container metadata via ffprobe is the ground truth — always prefer it
    // over filename parsing. Filenames can be garbage or wrong after renaming,
    // but embedded metadata persists.
    let probe_data = subtitles::probe_file_metadata(video_path);
    if let Some(ref pd) = probe_data {
        log::info!("[analyze] ffprobe: title={:?} year={:?} ja_audio={}",
            pd.title, pd.year, pd.has_japanese_audio);
    } else {
        log::info!("[analyze] ffprobe: no container metadata found");
    }

    // Parse the probe title the same way we parse filenames, since it often
    // contains "Title (Year) - S02E01 - Episode Title" format
    let probe_parsed = probe_data.as_ref()
        .and_then(|p| p.title.as_ref())
        .map(|pt| {
            let cleaned = clean_probe_title(pt);
            let mut pp = renamer::parse_filename(&cleaned);
            // Probe titles often have "Title (Year) - S02E01 - Episode" format.
            // parse_filename extracts year separately but leaves "(Year)" in the title.
            // Strip it so TMDb search gets a clean query.
            if let Some(y) = pp.year {
                pp.title = pp.title
                    .replace(&format!("({})", y), "")
                    .trim()
                    .to_string();
            }
            pp
        });

    // For TV episodes (file has S01E05 markers), ffprobe's year is almost always
    // the ENCODING date (2024) not the show's premiere (2021). Using that year in
    // TMDb search filters out the correct show. Only trust filename-embedded years
    // for episodes; for movies, ffprobe year is a reasonable fallback.
    let file_has_episode = parsed.season.is_some() || parsed.episode.is_some()
        || probe_parsed.as_ref().map(|pp| pp.season.is_some() || pp.episode.is_some()).unwrap_or(false);

    let (search_title, search_year) = if let Some(ref pp) = probe_parsed {
        let probe_valid = pp.title.len() >= 3
            && !(pp.title.chars().all(|c| !c.is_lowercase()) && parsed.title.chars().any(|c| c.is_lowercase()));

        if probe_valid {
            // For episodes: only use year if the filename explicitly had one.
            // For movies: filename year > probe parsed year > ffprobe raw year.
            let year = if file_has_episode {
                parsed.year.or(pp.year)
            } else {
                parsed.year.or(pp.year).or(probe_data.as_ref().and_then(|p| p.year))
            };
            log::info!("[analyze] Using probe title: '{}' year={:?} (episode={})", pp.title, year, file_has_episode);
            (pp.title.clone(), year)
        } else {
            let year = if file_has_episode {
                parsed.year
            } else {
                parsed.year.or(probe_data.as_ref().and_then(|p| p.year))
            };
            log::info!("[analyze] Probe title '{}' rejected, using filename: '{}' year={:?}",
                pp.title, parsed.title, year);
            (parsed.title.clone(), year)
        }
    } else {
        let year = if file_has_episode {
            parsed.year
        } else {
            parsed.year.or(probe_data.as_ref().and_then(|p| p.year))
        };
        log::info!("[analyze] Using filename title: '{}' year={:?}", parsed.title, year);
        (parsed.title.clone(), year)
    };

    // Merge season/episode from probe metadata if filename didn't have it
    let mut has_episode = probe_parsed.as_ref()
        .and_then(|pp| pp.season.or(pp.episode))
        .or(parsed.season)
        .or(parsed.episode)
        .is_some();

    // === IDENTIFICATION CHAIN ===
    // TMDb search leads (reliable for well-named files). OpenSubtitles hash runs
    // second as a validator: confirms TMDb match (confidence boost), provides a
    // fallback when TMDb fails (garbage filenames), or gets ignored when it
    // disagrees with a good TMDb match (hash collisions are common).

    let mut tmdb_data: Option<metadata::MetadataResult> = None;
    let mut hash_season: Option<u16> = None;
    let mut hash_episode: Option<u16> = None;
    let mut hash_confirmed = false; // true when hash agrees with TMDb result

    // Step 1: TMDb search using title from filename or ffprobe
    if !api_key.is_empty() {
        log::info!("[analyze] Step 1: TMDb search query='{}' year={:?} has_episode={}",
            search_title, search_year, has_episode);
        tmdb_data = metadata::search_with_fallback(&search_title, search_year, has_episode, api_key);
    }

    // Step 1.5: If probe title didn't match, try the filename title.
    if tmdb_data.is_none() && !api_key.is_empty() && search_title != parsed.title && parsed.title.len() >= 2 {
        log::info!("[analyze] Step 1.5: Probe title '{}' failed, trying filename title '{}'",
            search_title, parsed.title);
        tmdb_data = metadata::search_with_fallback(
            &parsed.title, parsed.year, has_episode, api_key
        );
    }

    // Step 2: Try parent folder name (skip "Season XX" folders → use grandparent instead)
    if tmdb_data.is_none() && !api_key.is_empty() {
        if let Some(parent) = video_path.parent() {
            let folder_name = parent.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let title_dir = if folder_name.to_lowercase().starts_with("season") {
                parent.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
            } else {
                folder_name
            };
            if title_dir.len() > 3 && title_dir != "test_drop" {
                let folder_parsed = renamer::parse_filename(title_dir);
                log::info!("[analyze] Step 2: trying parent folder '{}' → parsed '{}'",
                    title_dir, folder_parsed.title);
                let folder_year = folder_parsed.year.or(search_year);
                let folder_has_ep = folder_parsed.season.is_some() || has_episode;
                tmdb_data = metadata::search_with_fallback(
                    &folder_parsed.title, folder_year, folder_has_ep, api_key
                );
            }
        }
    }

    // Step 3: OpenSubtitles hash — always run when available.
    // If TMDb already matched: hash validates (same tmdb_id = confirmed, different = ignored).
    // If TMDb failed: hash provides the identification (garbage filename fallback).
    if !opensubs_key.is_empty() {
        log::info!("[analyze] Step 3: OpenSubtitles hash lookup for {}", video_path.display());
        if let Some(hash_id) = subtitles::identify_by_hash(video_path, opensubs_key) {
            let is_episode = hash_id.feature_type == "Episode";
            let media_type_str = if is_episode { "tv" } else { "movie" };

            let hash_tmdb_id = if is_episode {
                hash_id.parent_tmdb_id.or(hash_id.tmdb_id)
            } else {
                hash_id.tmdb_id
            };
            let hash_title = if is_episode {
                hash_id.parent_title.as_deref().unwrap_or(&hash_id.title)
            } else {
                &hash_id.title
            };

            if let Some(ref td) = tmdb_data {
                if hash_tmdb_id == Some(td.tmdb_id) {
                    // Hash confirms TMDb — high confidence
                    hash_confirmed = true;
                    log::info!("[analyze] Hash CONFIRMS TMDb match: '{}' (tmdb={})", td.title, td.tmdb_id);
                } else {
                    // Hash disagrees — but who's right? Compare title similarity.
                    // If TMDb returned a fuzzy match ("Squid Game: Fireplace") but
                    // hash points to an exact match ("Squid Game"), hash wins.
                    let query = search_title.to_lowercase();
                    let tmdb_title_lc = td.title.to_lowercase();
                    let hash_title_lc = hash_title.to_lowercase();
                    let tmdb_exact = tmdb_title_lc == query;
                    let hash_exact = hash_title_lc == query;

                    if hash_exact && !tmdb_exact {
                        // Hash is exact match, TMDb is fuzzy — hash wins
                        log::warn!("[analyze] Hash override: hash='{}' (exact) vs TMDb='{}' (fuzzy) — switching to hash",
                            hash_title, td.title);
                        if let Some(tid) = hash_tmdb_id {
                            if !api_key.is_empty() {
                                if let Some(hash_meta) = metadata::get_metadata_by_id(tid, media_type_str, api_key) {
                                    tmdb_data = Some(hash_meta);
                                    hash_confirmed = true;
                                }
                            }
                        }
                    } else {
                        log::info!("[analyze] Hash disagrees: hash='{}' (tmdb={:?}) vs TMDb='{}' (tmdb={}) — keeping TMDb",
                            hash_title, hash_tmdb_id, td.title, td.tmdb_id);
                    }
                }
                // Only adopt hash season/episode when hash confirmed the TMDb match.
                // If hash disagrees (wrong show), its episode data is irrelevant.
                if hash_confirmed && is_episode && parsed.season.is_none() && parsed.episode.is_none() {
                    hash_season = hash_id.season_number;
                    hash_episode = hash_id.episode_number;
                    if hash_season.is_some() || hash_episode.is_some() {
                        has_episode = true;
                    }
                }
            } else {
                // TMDb failed — hash is our only shot, use it as identification
                log::info!("[analyze] TMDb failed, using hash result: '{}' (tmdb={:?})", hash_title, hash_tmdb_id);
                if let Some(tid) = hash_tmdb_id {
                    if !api_key.is_empty() {
                        tmdb_data = metadata::get_metadata_by_id(tid, media_type_str, api_key);
                    }
                }
                if tmdb_data.is_none() && !api_key.is_empty() {
                    tmdb_data = metadata::search_with_fallback(
                        hash_title, hash_id.year, is_episode, api_key
                    );
                }
                if is_episode {
                    hash_season = hash_id.season_number;
                    hash_episode = hash_id.episode_number;
                    has_episode = true;
                }
            }
        }
    }

    if let Some(ref td) = tmdb_data {
        log::info!("[analyze] Final ID: '{}' ({:?}) tmdb={} type={} hash_confirmed={}",
            td.title, td.year, td.tmdb_id, td.media_type, hash_confirmed);
        // TMDb media_type is authoritative — if it says movie, don't treat as episode
        if td.media_type == "movie" && has_episode && parsed.season.is_none() && parsed.episode.is_none() {
            log::info!("[analyze] TMDb says movie but has_episode=true (from hash?) — resetting to movie");
            has_episode = false;
        }
    } else {
        log::warn!("[analyze] No match found for '{}'", search_title);
    }

    // Season/episode priority: probe metadata > filename > hash
    // Filename S01E05 is almost always right (torrent naming conventions).
    // Hash only fills in when filename had no episode info at all.
    let final_season = probe_parsed.as_ref().and_then(|pp| pp.season).or(parsed.season).or(hash_season);
    let final_episode = probe_parsed.as_ref().and_then(|pp| pp.episode).or(parsed.episode).or(hash_episode);
    let final_episode_end = probe_parsed.as_ref().and_then(|pp| pp.episode_end).or(parsed.episode_end);

    // 3. Get episode title (and runtime) if applicable
    let mut episode_runtime_min: Option<u32> = None;
    let episode_title = match (&tmdb_data, final_season, final_episode) {
        (Some(tmdb), Some(season), Some(episode)) if tmdb.media_type == "tv" => {
            let info = metadata::get_episode_title(tmdb.tmdb_id, season, episode, api_key);
            episode_runtime_min = info.as_ref().and_then(|i| i.runtime_minutes);
            info.and_then(|i| i.name)
        }
        _ => None,
    };

    // Expected runtime: episode-specific from TMDb, or series average, or movie runtime
    let expected_runtime_min = episode_runtime_min
        .or_else(|| tmdb_data.as_ref().and_then(|td| td.runtime_minutes));

    // Actual file duration from ffprobe
    let file_duration_min = probe_data.as_ref()
        .and_then(|p| p.duration_secs)
        .map(|secs| (secs / 60.0) as u32);

    // Duration match: compare expected vs actual runtime.
    // Within 2% = exact match (boost). Way off = suspicious.
    let duration_match = match (expected_runtime_min, file_duration_min) {
        (Some(expected), Some(actual)) if expected > 0 => {
            let diff_pct = ((actual as f64 - expected as f64) / expected as f64).abs();
            if diff_pct <= 0.02 {
                log::info!("[analyze] Duration EXACT match: expected={}min actual={}min ({:.1}% off)",
                    expected, actual, diff_pct * 100.0);
                1 // exact
            } else if diff_pct <= 0.10 {
                log::info!("[analyze] Duration close: expected={}min actual={}min ({:.1}% off)",
                    expected, actual, diff_pct * 100.0);
                0 // close enough, neutral
            } else {
                log::warn!("[analyze] Duration MISMATCH: expected={}min actual={}min ({:.1}% off)",
                    expected, actual, diff_pct * 100.0);
                -1 // suspicious
            }
        }
        _ => 0, // no data to compare
    };

    // 4. Classify (format + genre)
    let has_japanese_audio = probe_data
        .as_ref()
        .map(|p| p.has_japanese_audio)
        .unwrap_or(false);
    let classification = classifier::classify(filename, &parsed, tmdb_data.as_ref(), has_japanese_audio);

    // Confidence: combine classifier, hash validation, and duration match.
    //   hash confirmed + duration exact = 1.00 (bulletproof)
    //   hash confirmed = 0.99
    //   duration exact match = boost by 0.05
    //   TMDb only = classifier as-is
    //   duration mismatch = drop by 0.10
    //   hash-only (TMDb failed) = cap at 0.60
    let mut id_confidence = if hash_confirmed {
        classification.confidence.max(0.99)
    } else if tmdb_data.is_some() {
        classification.confidence
    } else {
        classification.confidence.min(0.60)
    };
    match duration_match {
        1 => id_confidence = (id_confidence + 0.05).min(1.0),  // exact duration → boost
        -1 => id_confidence = (id_confidence - 0.10).max(0.0), // way off → drop
        _ => {}
    }
    log::info!("[analyze] Classification: format='{}' genre='{}' confidence={:.2} (hash_confirmed={}, duration_match={})",
        classification.format, classification.genre, id_confidence, hash_confirmed, duration_match);

    // 5. Use TMDb data or parsed data for final values
    let final_title = tmdb_data
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| parsed.title.clone());
    let final_year = tmdb_data
        .as_ref()
        .and_then(|t| t.year)
        .or(parsed.year);
    let tmdb_id = tmdb_data.as_ref().map(|t| t.tmdb_id);
    let poster_url = tmdb_data.as_ref().and_then(|t| t.poster_url.clone());
    let media_type = tmdb_data
        .as_ref()
        .map(|t| t.media_type.clone())
        .unwrap_or_else(|| if has_episode { "tv".to_string() } else { "movie".to_string() });

    // 6. Build destination path
    let ext = renamer::get_extension(filename);
    let dest_path = build_dest_path(
        library,
        &classification.format,
        &classification.genre,
        &final_title,
        final_year,
        tmdb_id,
        final_season,
        final_episode,
        final_episode_end,
        episode_title.as_deref(),
        &ext,
        has_episode,
    );

    // 7. Find subtitles near the video
    let parent_dir = video_path.parent().unwrap_or(Path::new("."));
    let subtitle_files = subtitles::find_subtitles(
        parent_dir.to_str().unwrap_or("."),
        video_path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
    );

    // 8. Find junk files recursively (catches files in Other/, Subs/ etc.)
    let junk_files = if video_path.parent().is_some() {
        junk::find_junk_files_recursive(parent_dir)
    } else {
        Vec::new()
    };

    log::info!("[analyze] Result: '{}' ({:?}) → {} | subs={} junk={}",
        final_title, final_year, &dest_path, subtitle_files.len(), junk_files.len());

    AnalysisResult {
        source_path: video_path.to_string_lossy().to_string(),
        dest_path,
        title: final_title,
        year: final_year,
        format: classification.format,
        genre: classification.genre,
        media_type,
        season: final_season,
        episode: final_episode,
        episode_title,
        tmdb_id,
        poster_url,
        subtitle_files,
        junk_files,
        confidence: id_confidence,
    }
}

/// Clean release group junk from ffprobe container titles.
/// Encoders often embed "GroupName - Title" or "Title [GroupTag]" patterns.
fn clean_probe_title(raw: &str) -> String {
    let mut title = raw.to_string();

    // Strip bracket tags like [TGx], [YTS.MX], [rartv], (GalaxyRG), etc.
    static BRACKET_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"\s*[\[\(][A-Za-z0-9._-]+[\]\)]\s*").unwrap()
    });
    title = BRACKET_RE.replace_all(&title, " ").trim().to_string();

    // Strip "GroupName - Title" prefix: a single word (no spaces, often CamelCase
    // or all-lowercase) followed by " - " at the start.
    // e.g. "GalaxyRG - Titane" → "Titane", "YIFY - Movie Name" → "Movie Name"
    // BUT NOT "From - S02E03 - Tether" → "From" is the show title, not a group name.
    static GROUP_PREFIX_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"^[A-Za-z0-9]+\s*-\s+").unwrap()
    });
    static EP_MARKER_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"(?i)^S\d{1,2}E\d{1,3}").unwrap()
    });
    if GROUP_PREFIX_RE.is_match(&title) {
        let cleaned = GROUP_PREFIX_RE.replace(&title, "").trim().to_string();
        // Only strip if: remaining part is long enough AND doesn't start with
        // an episode marker (SxxExx). If it starts with SxxExx, the "prefix"
        // is actually the show title (e.g. "From - S02E03 - ...")
        if cleaned.len() >= 2 && !EP_MARKER_RE.is_match(&cleaned) {
            title = cleaned;
        }
    }

    // Strip trailing " - GroupName" suffix (single word after last " - ")
    static GROUP_SUFFIX_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"\s+-\s+[A-Za-z0-9]+$").unwrap()
    });
    if GROUP_SUFFIX_RE.is_match(&title) {
        let cleaned = GROUP_SUFFIX_RE.replace(&title, "").trim().to_string();
        if cleaned.len() >= 2 {
            title = cleaned;
        }
    }

    title
}

fn build_dest_path(
    library: &str,
    format: &str,
    genre: &str,
    title: &str,
    year: Option<u16>,
    tmdb_id: Option<u64>,
    season: Option<u16>,
    episode: Option<u16>,
    episode_end: Option<u16>,
    episode_title: Option<&str>,
    ext: &str,
    is_episode: bool,
) -> String {
    let title_dir = renamer::title_folder(title, year, tmdb_id);
    let mut dest = PathBuf::from(library)
        .join(format)
        .join(genre)
        .join(&title_dir);

    if is_episode {
        if let Some(s) = season {
            dest = dest.join(format!("Season {:02}", s));
        }
        let filename = renamer::episode_filename(
            title,
            season.unwrap_or(1),
            episode.unwrap_or(1),
            episode_end,
            episode_title,
            ext,
        );
        dest = dest.join(filename);
    } else {
        let filename = renamer::movie_filename(title, year, ext);
        dest = dest.join(filename);
    }

    dest.to_string_lossy().to_string()
}

/// Process confirmed analyses: move files, rename, record transactions.
/// `cleanup_root` is the parent directory of all dropped paths — cleanup stops here.
pub async fn process_files(
    analyses: Vec<AnalysisResult>,
    config: &Config,
    emit_progress: impl Fn(u32, u32),
    cleanup_root: &str,
) -> Result<BatchResult, String> {
    let _guard = PIPELINE_LOCK.lock().await;
    CANCEL_FLAG.store(false, Ordering::Relaxed);

    let batch_id = uuid::Uuid::new_v4().to_string();
    let mut succeeded = 0u32;
    let mut failed = 0u32;
    let mut errors = Vec::new();
    let total = analyses.len() as u32;

    for analysis in &analyses {
        if CANCEL_FLAG.load(Ordering::Relaxed) {
            break;
        }

        let analysis_clone = analysis.clone();
        let source_path = analysis.source_path.clone();
        let batch_id = batch_id.clone();
        let auto_download_subs = config.auto_download_subs;
        let subtitle_languages = config.subtitle_languages.clone();
        let opensubs_key = config.opensubs_api_key.clone();
        let root = cleanup_root.to_string();

        match tokio::task::spawn_blocking(move || {
            process_single_file(&analysis_clone, &batch_id, auto_download_subs, &subtitle_languages, &opensubs_key, &root)
        })
        .await
        {
            Ok(Ok(())) => succeeded += 1,
            Ok(Err(e)) => {
                failed += 1;
                errors.push(FileError {
                    path: source_path.clone(),
                    error: e,
                });
            }
            Err(e) => {
                failed += 1;
                errors.push(FileError {
                    path: source_path.clone(),
                    error: format!("Task error: {}", e),
                });
            }
        }

        emit_progress(succeeded + failed, total);
    }

    Ok(BatchResult {
        batch_id,
        succeeded,
        failed,
        errors,
    })
}

/// Public entry point for processing a single file (used by watcher, qBittorrent import).
/// `cleanup_root` is the top-level folder that was dropped/imported — cleanup will never
/// delete at or above this path.
pub fn process_single_file_pub(
    analysis: &AnalysisResult,
    batch_id: &str,
    auto_download_subs: bool,
    subtitle_languages: &[String],
    opensubs_key: &str,
    cleanup_root: &str,
) -> Result<(), String> {
    process_single_file(analysis, batch_id, auto_download_subs, subtitle_languages, opensubs_key, cleanup_root)
}

fn process_single_file(
    analysis: &AnalysisResult,
    batch_id: &str,
    auto_download_subs: bool,
    subtitle_languages: &[String],
    opensubs_key: &str,
    cleanup_root: &str,
) -> Result<(), String> {
    log::info!("[process] === Processing: {} → {} ===", analysis.source_path, analysis.dest_path);

    // Acquire sync lock to prevent concurrent file moves from watcher/qBit/UI
    let _sync_guard = PIPELINE_SYNC_LOCK
        .lock()
        .map_err(|e| format!("Pipeline lock error: {}", e))?;

    // 1. Create destination directory
    let dest_path = Path::new(&analysis.dest_path);
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // 2. Check if destination already exists (size + partial hash dedup)
    if dest_path.exists() {
        let src = Path::new(&analysis.source_path);
        let src_size = fs::metadata(src).map(|m| m.len()).unwrap_or(0);
        let dst_size = fs::metadata(dest_path).map(|m| m.len()).unwrap_or(1);
        if src_size == dst_size && partial_hash_match(src, dest_path) {
            log::info!("[process] Dedup: confirmed duplicate (size+hash match): {}", analysis.dest_path);
            if let Err(e) = fs::remove_file(src) {
                log::warn!("[process] Failed to remove duplicate source: {}", e);
            }
            return Ok(());
        }
        return Err(format!(
            "File already exists at destination with different content (src={}B dst={}B)",
            src_size, dst_size
        ));
    }

    // 3. Move + rename video
    log::info!("[process] Moving: {} → {}", analysis.source_path, analysis.dest_path);
    if fs::rename(&analysis.source_path, dest_path).is_err() {
        log::info!("[process] Rename failed (cross-device?), falling back to copy+delete");
        let src_size = fs::metadata(&analysis.source_path)
            .map(|m| m.len()).unwrap_or(0);
        fs::copy(&analysis.source_path, dest_path)
            .map_err(|e| format!("Failed to copy file: {}", e))?;
        // Verify copy by size
        let dst_size = fs::metadata(dest_path)
            .map(|m| m.len()).unwrap_or(0);
        if dst_size != src_size {
            if let Err(e) = fs::remove_file(dest_path) {
                log::error!("[process] Failed to clean up bad copy {}: {}", dest_path.display(), e);
            }
            return Err(format!("Copy verification failed - size mismatch ({} vs {})", src_size, dst_size));
        }
        fs::remove_file(&analysis.source_path)
            .map_err(|e| format!("Failed to remove source after copy: {}", e))?;
    }

    // 4.5. Extract thumbnail for Uncategorized files (helps manual identification)
    if analysis.genre == "Uncategorized" || analysis.format == "Uncategorized" {
        extract_thumbnail(dest_path);
    }

    // 5. Handle subtitles
    let video_stem = dest_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let dest_dir = dest_path.parent().unwrap_or(Path::new("."));
    let txn_id = uuid::Uuid::new_v4().to_string();

    for sub in &analysis.subtitle_files {
        let lang = subtitles::detect_language(sub);
        let forced = subtitles::is_forced(sub);
        let sdh = subtitles::is_sdh(sub);
        let sub_ext = Path::new(sub.as_str())
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("srt");
        let sub_name = renamer::subtitle_filename(video_stem, &lang, forced, sdh, sub_ext);
        let sub_dest = dest_dir.join(&sub_name);

        let sub_path = Path::new(sub.as_str());
        if fs::rename(sub_path, &sub_dest).is_err() {
            match fs::copy(sub_path, &sub_dest) {
                Ok(_) => {
                    if let Err(e) = fs::remove_file(sub_path) {
                        log::error!("[process] Failed to remove source subtitle after copy: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("[process] Failed to copy subtitle {} → {}: {}", sub, sub_dest.display(), e);
                    continue;
                }
            }
        }

        if let Err(e) = transaction::record_subtitle(
            &txn_id,
            Some(sub),
            sub_dest.to_str().unwrap_or(""),
            &lang,
            false,
        ) {
            log::error!("[process] Failed to record subtitle: {}", e);
        }
    }

    // 6. Download missing subs if enabled
    if auto_download_subs && analysis.subtitle_files.is_empty() {
        for lang in subtitle_languages {
            log::info!("[process] Downloading {} subtitle for: {}", lang, analysis.dest_path);
            match subtitles::download_subtitle(dest_path, lang, opensubs_key) {
                Ok(sub_path) => {
                    log::info!("[process] Downloaded subtitle: {}", sub_path);
                    if let Err(e) = transaction::record_subtitle(&txn_id, None, &sub_path, lang, true) {
                        log::error!("[process] Failed to record downloaded subtitle: {}", e);
                    }
                }
                Err(e) => {
                    log::warn!("[process] Subtitle download failed ({}): {}", lang, e);
                }
            }
        }
    }

    // 7. Remove junk files and clean their parent directories
    // Use the cleanup_root (the folder the user dropped) as the upper boundary to prevent
    // deleting the drop folder itself or any well-known parent directories.
    let source_path = Path::new(&analysis.source_path);
    let source_root = Path::new(cleanup_root);

    for junk_path in &analysis.junk_files {
        let junk = Path::new(junk_path);
        if let Err(e) = fs::remove_file(junk) {
            log::warn!("[process] Failed to remove junk file {}: {}", junk.display(), e);
        }
        if let Some(junk_parent) = junk.parent() {
            clean_empty_dirs(junk_parent, source_root);
        }
    }

    // 7.5. Clean empty subtitle source directories (Subs/, subtitles/, etc.)
    for sub in &analysis.subtitle_files {
        let sub_path = Path::new(sub.as_str());
        if let Some(sub_parent) = sub_path.parent() {
            if let Some(video_parent) = source_path.parent() {
                if sub_parent != video_parent {
                    clean_empty_dirs(sub_parent, video_parent);
                }
            }
        }
    }

    // 8. Clean empty source folder (trash it so user can undo)
    if let Some(parent) = source_path.parent() {
        clean_empty_dirs(parent, source_root);
    }

    // 9. Record transaction
    transaction::record(&transaction::Transaction {
        id: txn_id,
        batch_id: batch_id.to_string(),
        source_path: analysis.source_path.clone(),
        dest_path: analysis.dest_path.clone(),
        title: analysis.title.clone(),
        year: analysis.year,
        format: analysis.format.clone(),
        genre: analysis.genre.clone(),
        media_type: analysis.media_type.clone(),
        season: analysis.season,
        episode: analysis.episode,
        episode_title: analysis.episode_title.clone(),
        tmdb_id: analysis.tmdb_id,
        poster_url: analysis.poster_url.clone(),
        sha256: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        undone: false,
        locked: false,
        confidence: analysis.confidence,
    })?;

    log::info!("[process] Done: '{}' → {}/{}", analysis.title, analysis.format, analysis.genre);
    Ok(())
}

// === EDIT & RELOCATE (Review feature) ===

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditRequest {
    pub transaction_id: String,
    pub title: String,
    pub year: Option<u16>,
    pub format: String,
    pub genre: String,
    pub media_type: String,
    pub tmdb_id: Option<u64>,
    pub poster_url: Option<String>,
    pub season: Option<u16>,
    pub episode: Option<u16>,
    pub episode_title: Option<String>,
}

/// Edit a transaction's metadata and relocate the file to its new correct path.
/// Returns the new Transaction after the move.
pub fn edit_and_relocate(edit: &EditRequest, library_path: &str) -> Result<transaction::Transaction, String> {
    // 1. Acquire sync lock
    let _sync_guard = PIPELINE_SYNC_LOCK
        .lock()
        .map_err(|e| format!("Pipeline lock error: {}", e))?;

    // 2. Look up existing transaction
    let old_txn = transaction::get_transaction_by_id(&edit.transaction_id)?;
    if old_txn.undone {
        return Err("Transaction has been undone".to_string());
    }

    // 3. Verify file exists
    let old_path = Path::new(&old_txn.dest_path);
    if !old_path.exists() {
        return Err(format!("File no longer exists at: {}", old_txn.dest_path));
    }

    // 4. Extract extension from existing file
    let ext = old_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv")
        .to_string();

    let is_episode = edit.media_type == "tv";

    // 5. Build new dest path
    let new_dest = build_dest_path(
        library_path,
        &edit.format,
        &edit.genre,
        &edit.title,
        edit.year,
        edit.tmdb_id,
        edit.season,
        edit.episode,
        None, // episode_end
        edit.episode_title.as_deref(),
        &ext,
        is_episode,
    );

    let new_dest_path = Path::new(&new_dest);

    // 6. If same path → just update metadata + lock, no file move needed
    if new_dest == old_txn.dest_path {
        transaction::update_transaction_metadata(
            &edit.transaction_id,
            &edit.title,
            edit.year,
            &edit.format,
            &edit.genre,
            edit.tmdb_id,
            edit.poster_url.as_deref(),
        )?;
        transaction::lock_transactions(&[edit.transaction_id.clone()])?;
        return Ok(transaction::get_transaction_by_id(&edit.transaction_id)?);
    }

    // 7. Check destination conflict
    if new_dest_path.exists() {
        return Err(format!("Destination already exists: {}", new_dest));
    }

    // 8. Create dest dir
    if let Some(parent) = new_dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // 9. Move video file (rename with copy+delete fallback)
    log::info!("[edit] Moving: {} → {}", old_txn.dest_path, new_dest);
    if fs::rename(old_path, new_dest_path).is_err() {
        fs::copy(old_path, new_dest_path)
            .map_err(|e| format!("Failed to copy file: {}", e))?;
        fs::remove_file(old_path)
            .map_err(|e| format!("Failed to remove original after copy: {}", e))?;
    }

    // 10. Move co-located subtitles and update DB records
    let old_stem = old_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let new_stem = new_dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let old_dir = old_path.parent().unwrap_or(Path::new("."));
    let new_dir = new_dest_path.parent().unwrap_or(Path::new("."));
    let sub_paths = transaction::get_subtitle_paths(&edit.transaction_id);

    for sub_path_str in &sub_paths {
        let sub_path = Path::new(sub_path_str);
        if !sub_path.exists() {
            continue;
        }
        let sub_filename = sub_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Replace old video stem with new one in subtitle filename
        let new_sub_filename = if sub_filename.to_lowercase().starts_with(&old_stem.to_lowercase()) {
            format!("{}{}", new_stem, &sub_filename[old_stem.len()..])
        } else {
            sub_filename.to_string()
        };
        let new_sub_path = new_dir.join(&new_sub_filename);
        if fs::rename(sub_path, &new_sub_path).is_err() {
            match fs::copy(sub_path, &new_sub_path) {
                Ok(_) => {
                    if let Err(e) = fs::remove_file(sub_path) {
                        log::error!("[edit] Failed to remove source subtitle after copy: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("[edit] Failed to copy subtitle {} → {}: {}", sub_path.display(), new_sub_path.display(), e);
                }
            }
        }
    }

    // Update subtitle DB records to point to new paths
    if let Err(e) = transaction::update_subtitle_paths(
        &edit.transaction_id,
        old_stem,
        new_stem,
        new_dir.to_str().unwrap_or(""),
    ) {
        log::error!("[edit] Failed to update subtitle paths: {}", e);
    }

    // 11. Clean empty source dirs
    clean_empty_dirs(old_dir, Path::new(library_path));

    // 12. Mark old transaction as undone, record new one
    transaction::mark_undone(&edit.transaction_id)?;

    // Reassign subtitle records to the new transaction
    let new_txn_id = uuid::Uuid::new_v4().to_string();
    if let Err(e) = transaction::reassign_subtitle_records(&edit.transaction_id, &new_txn_id) {
        log::error!("[edit] Failed to reassign subtitle records: {}", e);
    }

    let new_txn = transaction::Transaction {
        id: new_txn_id,
        batch_id: old_txn.batch_id,
        source_path: old_txn.source_path,
        dest_path: new_dest,
        title: edit.title.clone(),
        year: edit.year,
        format: edit.format.clone(),
        genre: edit.genre.clone(),
        media_type: edit.media_type.clone(),
        season: edit.season,
        episode: edit.episode,
        episode_title: edit.episode_title.clone(),
        tmdb_id: edit.tmdb_id,
        poster_url: edit.poster_url.clone(),
        sha256: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        undone: false,
        locked: true,
        confidence: 1.0,
    };
    transaction::record(&new_txn)?;

    log::info!("[edit] Done: '{}' → {}/{}", edit.title, edit.format, edit.genre);
    Ok(new_txn)
}

/// Request cancellation of the current processing batch.
pub fn cancel_processing() {
    CANCEL_FLAG.store(true, Ordering::Relaxed);
}

/// Extract a thumbnail frame from a video at ~30% duration for manual identification.
fn extract_thumbnail(video_path: &Path) {
    let ffmpeg = match subtitles::get_ffmpeg_path() {
        Some(p) => p,
        None => return,
    };

    let probe = subtitles::probe_file_metadata(video_path);
    let duration = probe.and_then(|p| p.duration_secs).unwrap_or(0.0);
    let seek_secs = if duration > 10.0 {
        (duration * 0.3) as u64
    } else {
        5
    };

    let thumb_path = video_path.with_extension("thumb.jpg");
    if thumb_path.exists() {
        return;
    }

    let result = std::process::Command::new(&ffmpeg)
        .args([
            "-ss", &seek_secs.to_string(),
            "-i", video_path.to_str().unwrap_or(""),
            "-vframes", "1",
            "-q:v", "2",
            thumb_path.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match result {
        Ok(status) if status.success() => {
            log::info!("[process] Extracted thumbnail: {}", thumb_path.display());
        }
        _ => {
            log::warn!("[process] Failed to extract thumbnail for {}", video_path.display());
        }
    }
}

fn clean_empty_dirs(dir: &Path, root: &Path) {
    // Never delete at or above the root boundary
    if dir == root || !dir.starts_with(root) {
        return;
    }
    // A directory is "empty" if it contains only platform junk files
    let is_empty = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .all(|e| crate::junk::is_platform_junk(&e.path())),
        Err(_) => false,
    };
    if is_empty {
        // Send to system trash so user can undo if needed
        if trash::delete(dir).is_ok() {
            log::info!("[cleanup] Trashed empty folder: {}", dir.display());
            // Successfully trashed, try parent
            if let Some(parent) = dir.parent() {
                clean_empty_dirs(parent, root);
            }
        }
    }
}

// Library browsing lives in crate::library.
// Re-exported here so frb_generated.rs paths continue to work.
pub use crate::library::{
    FormatInfo, GenreInfo, GenrePage, MediaDetail, MediaFile, MediaInfo,
};
pub use crate::library::{
    get_format_contents, get_genre_contents, get_library_contents, get_media_details,
    get_recently_added,
};
pub use crate::library::migrate::move_library;

/// Compare two files by hashing their first and last 64KB with SHA-256.
/// Returns true if both chunks match, meaning the files are almost certainly identical.
/// Much faster than full-file hash for multi-GB video files, while far safer than
/// file-size-only comparison (different encodes can have identical sizes).
pub(crate) fn partial_hash_match(a: &Path, b: &Path) -> bool {
    use sha2::{Digest, Sha256};
    const CHUNK: u64 = 65_536; // 64KB

    let hash_ends = |path: &Path| -> Option<([u8; 32], [u8; 32])> {
        let mut f = fs::File::open(path).ok()?;
        let len = f.metadata().ok()?.len();

        // Read first chunk
        let mut head = vec![0u8; CHUNK.min(len) as usize];
        f.read_exact(&mut head).ok()?;
        let head_hash: [u8; 32] = {
            let mut h = Sha256::new();
            h.update(&head);
            h.finalize().into()
        };

        // Read last chunk (may overlap with head for small files)
        let tail_start = len.saturating_sub(CHUNK);
        f.seek(SeekFrom::Start(tail_start)).ok()?;
        let mut tail = vec![0u8; (len - tail_start) as usize];
        f.read_exact(&mut tail).ok()?;
        let tail_hash: [u8; 32] = {
            let mut h = Sha256::new();
            h.update(&tail);
            h.finalize().into()
        };

        Some((head_hash, tail_hash))
    };

    match (hash_ends(a), hash_ends(b)) {
        (Some((ah, at)), Some((bh, bt))) => ah == bh && at == bt,
        _ => false,
    }
}

// Library rescan lives in crate::library::rescan.
// Re-exported here so frb_generated.rs paths continue to work.
pub use crate::library::rescan::{rescan_library, RescanProgress};
