/// Library folder scanning — read format/genre/title directory trees.

use super::{FormatInfo, GenreInfo, GenrePage, MediaDetail, MediaFile, MediaInfo};
use crate::metadata;
use crate::renamer;
use crate::transaction;
use once_cell::sync::Lazy;
use std::fs;
use std::path::Path;

/// Get library contents: scan the library root for format folders.
pub fn get_library_contents(library_path: &str) -> Result<Vec<FormatInfo>, String> {
    let root = Path::new(library_path);
    if !root.exists() {
        return Err("Library path does not exist".to_string());
    }

    let tmdb_api_key = crate::config::load_config()
        .map(|c| c.tmdb_api_key)
        .unwrap_or_default();

    let mut formats = Vec::new();

    for default_name in crate::formats::default_format_names() {
        let format_path = root.join(default_name);
        let info = scan_format_dir(&format_path, default_name, &tmdb_api_key);
        formats.push(info);
    }

    for format_def in crate::formats::FORMATS.iter().filter(|f| f.auto_create) {
        let format_path = root.join(format_def.name);
        if format_path.exists() {
            let info = scan_format_dir(&format_path, format_def.name, &tmdb_api_key);
            if info.media_count > 0 {
                formats.push(info);
            }
        }
    }

    Ok(formats)
}

fn scan_format_dir(format_path: &Path, name: &str, tmdb_api_key: &str) -> FormatInfo {
    let mut genre_count = 0u32;
    let mut media_count = 0u32;
    let mut poster_samples = Vec::new();
    let media_type = match name {
        "Shows" | "Anime" | "Animated Shows" => "tv",
        _ => "movie",
    };

    if format_path.exists() {
        if let Ok(entries) = fs::read_dir(format_path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    genre_count += 1;
                    if let Ok(genre_entries) = fs::read_dir(entry.path()) {
                        for ge in genre_entries.flatten() {
                            if ge.path().is_dir() {
                                media_count += 1;
                                if poster_samples.len() < 4 {
                                    let dir_name = ge.file_name().to_string_lossy().to_string();
                                    let (_, _, tmdb_id) = parse_title_folder(&dir_name);
                                    let poster = if let Some(id) = tmdb_id {
                                        if !tmdb_api_key.is_empty() {
                                            metadata::get_poster_by_tmdb_id(id, tmdb_api_key, media_type)
                                        } else {
                                            None
                                        }
                                        .or_else(|| find_poster_in_dir(&ge.path()))
                                    } else {
                                        find_poster_in_dir(&ge.path())
                                    };
                                    if let Some(p) = poster {
                                        poster_samples.push(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    FormatInfo {
        name: name.to_string(),
        path: format_path.to_string_lossy().to_string(),
        genre_count,
        media_count,
        poster_samples,
    }
}

/// Get format contents: scan a format folder for genre subfolders.
pub fn get_format_contents(format_path: &str) -> Result<Vec<GenreInfo>, String> {
    let path = Path::new(format_path);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let tmdb_api_key = crate::config::load_config()
        .map(|c| c.tmdb_api_key)
        .unwrap_or_default();

    let mut genres = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let genre_name = entry.file_name().to_string_lossy().to_string();

            let mut media_count = 0u32;
            let mut media_samples = Vec::new();

            if let Ok(genre_entries) = fs::read_dir(entry.path()) {
                let mut title_dirs: Vec<_> = genre_entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .filter(|e| !auto_clean_if_only_junk(&e.path()))
                    .collect();

                media_count = title_dirs.len() as u32;

                title_dirs.sort_by(|a, b| {
                    let (ta, ya, _) = parse_title_folder(&a.file_name().to_string_lossy());
                    let (tb, yb, _) = parse_title_folder(&b.file_name().to_string_lossy());
                    franchise_key(&ta)
                        .cmp(&franchise_key(&tb))
                        .then_with(|| ya.cmp(&yb))
                        .then_with(|| ta.to_lowercase().cmp(&tb.to_lowercase()))
                });

                for ge in title_dirs.iter().take(10) {
                    let info = media_info_from_dir(&ge.path(), &genre_name, &tmdb_api_key);
                    media_samples.push(info);
                }
            }

            if media_count > 0 {
                genres.push(GenreInfo {
                    name: genre_name,
                    path: entry.path().to_string_lossy().to_string(),
                    media_count,
                    media_samples,
                });
            }
        }
    }

    genres.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(genres)
}

/// Get genre contents with pagination.
pub fn get_genre_contents(
    genre_path: &str,
    offset: u32,
    limit: u32,
) -> Result<GenrePage, String> {
    let path = Path::new(genre_path);
    if !path.exists() {
        return Ok(GenrePage {
            items: Vec::new(),
            total: 0,
            has_more: false,
        });
    }

    let tmdb_api_key = crate::config::load_config()
        .map(|c| c.tmdb_api_key)
        .unwrap_or_default();

    let genre_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let mut all_dirs: Vec<_> = fs::read_dir(path)
        .map_err(|e| format!("Failed to read genre directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| !auto_clean_if_only_junk(&e.path()))
        .collect();

    all_dirs.sort_by(|a, b| {
        let (ta, ya, _) = parse_title_folder(&a.file_name().to_string_lossy());
        let (tb, yb, _) = parse_title_folder(&b.file_name().to_string_lossy());
        franchise_key(&ta)
            .cmp(&franchise_key(&tb))
            .then_with(|| ya.cmp(&yb))
            .then_with(|| ta.to_lowercase().cmp(&tb.to_lowercase()))
    });

    let total = all_dirs.len() as u32;
    let items: Vec<MediaInfo> = all_dirs
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .map(|entry| media_info_from_dir(&entry.path(), &genre_name, &tmdb_api_key))
        .collect();

    let has_more = (offset + limit) < total;

    Ok(GenrePage {
        items,
        total,
        has_more,
    })
}

/// Get detailed info for a single media title directory.
pub fn get_media_details(media_path: &str) -> Result<MediaDetail, String> {
    let dir = Path::new(media_path);
    if !dir.exists() || !dir.is_dir() {
        return Err("Media path does not exist or is not a directory".to_string());
    }

    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");
    let (title, year, tmdb_id) = parse_title_folder(dir_name);

    let genre = dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let format = dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Movies")
        .to_string();

    let media_type_str = match format.as_str() {
        "Shows" | "Anime" | "Animated Shows" => "tv",
        _ => "movie",
    };

    let has_season_dirs = fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().is_dir() && e.file_name().to_string_lossy().starts_with("Season"))
        })
        .unwrap_or(false);

    let is_tv = media_type_str == "tv" || has_season_dirs;

    let tmdb_api_key = crate::config::load_config()
        .map(|c| c.tmdb_api_key)
        .unwrap_or_default();

    let poster_url = if let Some(id) = tmdb_id {
        if !tmdb_api_key.is_empty() {
            metadata::get_poster_by_tmdb_id(id, &tmdb_api_key, media_type_str)
        } else {
            None
        }
        .or_else(|| find_poster_in_dir(dir))
    } else {
        find_poster_in_dir(dir)
    };

    let mut files = Vec::new();

    if is_tv {
        let mut season_dirs: Vec<_> = fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().is_dir() && e.file_name().to_string_lossy().starts_with("Season")
            })
            .collect();
        season_dirs.sort_by_key(|e| e.file_name().to_string_lossy().to_string());

        for season_dir in &season_dirs {
            if let Ok(entries) = fs::read_dir(season_dir.path()) {
                let mut episode_files: Vec<_> = entries
                    .flatten()
                    .filter(|e| e.path().is_file() && renamer::is_video_file(&e.path()))
                    .collect();
                episode_files.sort_by_key(|e| e.file_name().to_string_lossy().to_string());

                for ep in episode_files {
                    let ep_path = ep.path();
                    let ep_filename = ep.file_name().to_string_lossy().to_string();
                    let parsed = renamer::parse_filename(&ep_filename);
                    let size = fs::metadata(&ep_path).map(|m| m.len()).unwrap_or(0);

                    let ep_stem = ep_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    let ep_dir = ep_path.parent().unwrap_or(Path::new("."));
                    let has_subs = has_subtitle_file(ep_dir, ep_stem);

                    let ep_title = parsed
                        .episode_end
                        .is_none()
                        .then(|| {
                            transaction::get_episode_title_for_path(
                                ep_path.to_str().unwrap_or(""),
                            )
                        })
                        .flatten()
                        .or(parsed
                            .episode_end
                            .is_none()
                            .then(|| parse_episode_title_from_filename(&ep_filename))
                            .flatten());

                    files.push(MediaFile {
                        path: ep_path.to_string_lossy().to_string(),
                        filename: ep_filename,
                        season: parsed.season,
                        episode: parsed.episode,
                        episode_title: ep_title,
                        size_bytes: size,
                        has_subtitles: has_subs,
                    });
                }
            }
        }
    } else {
        if let Ok(entries) = fs::read_dir(dir) {
            let mut video_files: Vec<_> = entries
                .flatten()
                .filter(|e| e.path().is_file() && renamer::is_video_file(&e.path()))
                .collect();
            video_files.sort_by_key(|e| e.file_name().to_string_lossy().to_string());

            for vf in video_files {
                let vf_path = vf.path();
                let vf_filename = vf.file_name().to_string_lossy().to_string();
                let size = fs::metadata(&vf_path).map(|m| m.len()).unwrap_or(0);

                let vf_stem = vf_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let has_subs = has_subtitle_file(dir, vf_stem);

                files.push(MediaFile {
                    path: vf_path.to_string_lossy().to_string(),
                    filename: vf_filename,
                    season: None,
                    episode: None,
                    episode_title: None,
                    size_bytes: size,
                    has_subtitles: has_subs,
                });
            }
        }
    }

    let season_count = if is_tv {
        files
            .iter()
            .filter_map(|f| f.season)
            .collect::<std::collections::HashSet<_>>()
            .len() as u16
    } else {
        0
    };
    let episode_count = files.len() as u16;

    Ok(MediaDetail {
        title,
        year,
        path: media_path.to_string(),
        poster_url,
        tmdb_id,
        format,
        genre,
        media_type: if is_tv { "tv" } else { "movie" }.to_string(),
        files,
        season_count,
        episode_count,
    })
}

/// Get recently added items across all formats/genres, sorted by directory mtime.
pub fn get_recently_added(library_path: &str, limit: u32) -> Result<Vec<MediaInfo>, String> {
    let root = Path::new(library_path);
    if !root.exists() {
        return Ok(Vec::new());
    }

    let tmdb_api_key = crate::config::load_config()
        .map(|c| c.tmdb_api_key)
        .unwrap_or_default();

    let mut entries: Vec<(std::path::PathBuf, std::time::SystemTime, String)> = Vec::new();

    let format_dirs =
        fs::read_dir(root).map_err(|e| format!("Failed to read library: {}", e))?;
    for format_entry in format_dirs.flatten() {
        if !format_entry.path().is_dir() {
            continue;
        }
        let format_name = format_entry.file_name().to_string_lossy().to_string();
        if !crate::formats::is_valid_format(&format_name) {
            continue;
        }
        if let Ok(genre_dirs) = fs::read_dir(format_entry.path()) {
            for genre_entry in genre_dirs.flatten() {
                if !genre_entry.path().is_dir() {
                    continue;
                }
                let genre_name = genre_entry.file_name().to_string_lossy().to_string();

                if let Ok(title_dirs) = fs::read_dir(genre_entry.path()) {
                    for title_entry in title_dirs.flatten() {
                        if !title_entry.path().is_dir() {
                            continue;
                        }
                        if !has_video_files_recursive(&title_entry.path()) {
                            auto_clean_if_only_junk(&title_entry.path());
                            continue;
                        }
                        let mtime = title_entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::UNIX_EPOCH);
                        entries.push((title_entry.path(), mtime, genre_name.clone()));
                    }
                }
            }
        }
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1));

    let items: Vec<MediaInfo> = entries
        .into_iter()
        .take(limit as usize)
        .map(|(path, _, genre)| media_info_from_dir(&path, &genre, &tmdb_api_key))
        .collect();

    Ok(items)
}

// === Internal helpers ===

fn media_info_from_dir(title_dir: &Path, genre: &str, tmdb_api_key: &str) -> MediaInfo {
    let dir_name = title_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");

    let (title, year, tmdb_id) = parse_title_folder(dir_name);

    let format = title_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Movies")
        .to_string();

    let media_type = match format.as_str() {
        "Shows" | "Anime" | "Animated Shows" => "tv",
        _ => "movie",
    };

    let poster_url = if let Some(id) = tmdb_id {
        if !tmdb_api_key.is_empty() {
            metadata::get_poster_by_tmdb_id(id, tmdb_api_key, media_type)
        } else {
            None
        }
        .or_else(|| find_poster_in_dir(title_dir))
    } else {
        find_poster_in_dir(title_dir)
    };

    MediaInfo {
        title,
        year,
        path: title_dir.to_string_lossy().to_string(),
        poster_url,
        tmdb_id,
        format,
        genre: genre.to_string(),
    }
}

/// First 2 words of a title, lowercased. Groups franchises together.
fn franchise_key(title: &str) -> String {
    title
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Parse "Title (Year) [tmdbid-123]" folder name into components.
pub(crate) fn parse_title_folder(name: &str) -> (String, Option<u16>, Option<u64>) {
    static YEAR_RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"\((\d{4})\)").unwrap());
    static TMDB_RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"\[tmdbid-(\d+)\]").unwrap());

    let year: Option<u16> = YEAR_RE
        .captures(name)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok());

    let tmdb_id: Option<u64> = TMDB_RE
        .captures(name)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok());

    let title = name
        .split('(')
        .next()
        .unwrap_or(name)
        .trim()
        .to_string();

    (title, year, tmdb_id)
}

fn find_poster_in_dir(dir: &Path) -> Option<String> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if renamer::is_video_file(&entry.path()) {
                if let Some(poster) =
                    transaction::get_poster_for_path(entry.path().to_str().unwrap_or(""))
                {
                    return Some(poster);
                }
            }
        }
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                    for se in sub_entries.flatten() {
                        if renamer::is_video_file(&se.path()) {
                            if let Some(poster) = transaction::get_poster_for_path(
                                se.path().to_str().unwrap_or(""),
                            ) {
                                return Some(poster);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check if a directory tree has any video files.
pub(crate) fn has_video_files_recursive(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && renamer::is_video_file(&path) {
                return true;
            }
            if path.is_dir() && has_video_files_recursive(&path) {
                return true;
            }
        }
    }
    false
}

/// Check if a directory ONLY contains junk files and remove it if so.
pub(crate) fn auto_clean_if_only_junk(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    if has_video_files_recursive(dir) {
        return false;
    }
    if dir_contains_only_junk(dir) {
        remove_junk_tree(dir);
        return true;
    }
    false
}

fn dir_contains_only_junk(dir: &Path) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !dir_contains_only_junk(&path) {
                return false;
            }
        } else {
            let name = entry.file_name().to_string_lossy().to_string();
            if !matches!(name.as_str(), ".DS_Store" | "Thumbs.db" | "desktop.ini") {
                return false;
            }
        }
    }
    true
}

fn remove_junk_tree(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_junk_tree(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
    }
    let _ = fs::remove_dir(dir);
}

/// Check if any subtitle file exists for the given video stem.
fn has_subtitle_file(dir: &Path, video_stem: &str) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with(&video_stem.to_lowercase()) {
                let ext = Path::new(&*name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if matches!(ext, "srt" | "ass" | "ssa" | "sub" | "vtt") {
                    return true;
                }
            }
        }
    }
    false
}

/// Parse episode title from Jellyfin-style filename: "Title - S01E01 - Episode Title.ext"
fn parse_episode_title_from_filename(filename: &str) -> Option<String> {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    static RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"(?i)-\s*S\d+E\d+(?:E\d+)?\s*-\s*(.+)$").unwrap()
    });
    RE.captures(stem)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}
