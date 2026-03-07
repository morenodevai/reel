/// Playback FFI API -- watch progress CRUD and external subtitle discovery.

use crate::subtitles;
use crate::transaction;
use std::path::Path;

/// An external subtitle file discovered next to a video.
pub struct ExternalSubtitle {
    pub path: String,
    pub language: String,
    pub format: String,
}

/// Save/update watch progress for a file.
pub fn save_watch_progress(
    file_path: String,
    position: f64,
    duration: f64,
    title: Option<String>,
    poster_url: Option<String>,
    media_path: Option<String>,
    season: Option<u32>,
    episode: Option<u32>,
    episode_title: Option<String>,
    media_type: Option<String>,
) -> Result<(), String> {
    Ok(transaction::update_watch_progress(
        &file_path,
        position,
        duration,
        title.as_deref(),
        poster_url.as_deref(),
        media_path.as_deref(),
        season,
        episode,
        episode_title.as_deref(),
        media_type.as_deref(),
    )?)
}

/// Load watch progress for a single file.
pub fn load_watch_progress(file_path: String) -> Result<Option<transaction::WatchProgress>, String> {
    Ok(transaction::get_watch_progress(&file_path)?)
}

/// Load all watch progress entries for a media (by media_path).
pub fn load_all_progress(media_path: String) -> Result<Vec<transaction::WatchProgress>, String> {
    Ok(transaction::get_all_progress_for_media(&media_path)?)
}

/// Mark a file as fully watched.
pub fn set_file_completed(file_path: String) -> Result<(), String> {
    Ok(transaction::mark_file_completed(&file_path)?)
}

/// Mark a file as unwatched (remove progress).
pub fn set_file_unwatched(file_path: String) -> Result<(), String> {
    Ok(transaction::mark_file_unwatched(&file_path)?)
}

/// Discover external subtitle files next to a video file.
/// Scans the video's parent directory and subdirectories for .srt, .ass, .ssa, .sub, .vtt files.
pub fn discover_external_subtitles(video_path: String) -> Result<Vec<ExternalSubtitle>, String> {
    let path = Path::new(&video_path);
    let dir = path
        .parent()
        .ok_or_else(|| "No parent directory".to_string())?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let sub_paths = subtitles::find_subtitles(
        dir.to_str().unwrap_or(""),
        stem,
    );

    let mut results = Vec::new();
    for sub_path in sub_paths {
        let ext = Path::new(&sub_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let lang_code = subtitles::detect_language(&sub_path);
        let language = lang_code_to_name(&lang_code);

        results.push(ExternalSubtitle {
            path: sub_path,
            language,
            format: ext,
        });
    }

    Ok(results)
}

/// Convert a 3-letter language code to a human-readable name.
fn lang_code_to_name(code: &str) -> String {
    match code {
        "eng" => "English",
        "spa" => "Spanish",
        "fre" => "French",
        "ger" => "German",
        "jpn" => "Japanese",
        "por" => "Portuguese",
        "ita" => "Italian",
        "rus" => "Russian",
        "chi" => "Chinese",
        "kor" => "Korean",
        "ara" => "Arabic",
        "hin" => "Hindi",
        "dut" => "Dutch",
        "gre" => "Greek",
        "ind" => "Indonesian",
        "per" => "Persian",
        "fin" => "Finnish",
        "swe" => "Swedish",
        "tur" => "Turkish",
        "pol" => "Polish",
        "rum" => "Romanian",
        "tha" => "Thai",
        "vie" => "Vietnamese",
        "cze" => "Czech",
        "hun" => "Hungarian",
        "nor" => "Norwegian",
        "dan" => "Danish",
        "heb" => "Hebrew",
        "may" => "Malay",
        "ukr" => "Ukrainian",
        "hrv" => "Croatian",
        "bul" => "Bulgarian",
        "und" => "Unknown",
        other => other,
    }
    .to_string()
}
