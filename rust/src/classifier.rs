use crate::ai;
use crate::formats;
use crate::metadata;
use crate::renamer;

/// Classification result combining AI + metadata signals.
#[derive(Debug, Clone)]
pub struct ClassificationOutput {
    pub format: String,
    pub genre: String,
    pub confidence: f32,
}

/// Determine the format and genre for a media file.
pub fn classify(
    filename: &str,
    parsed: &renamer::ParsedFilename,
    tmdb_data: Option<&metadata::MetadataResult>,
    has_japanese_audio: bool,
) -> ClassificationOutput {
    // Get AI signal if available
    let ai_result = if ai::is_classifier_ready() {
        ai::classify_filename(filename).ok()
    } else {
        None
    };

    let has_episode = parsed.season.is_some() || parsed.episode.is_some();

    // Determine format using decision matrix
    let format = determine_format(has_episode, parsed.is_anime_format, &ai_result, tmdb_data, has_japanese_audio);

    // Determine genre from TMDb data
    let genre = determine_genre(tmdb_data, &format);

    let confidence = ai_result.map(|r| r.confidence).unwrap_or(0.6);

    ClassificationOutput {
        format,
        genre,
        confidence,
    }
}

fn determine_format(
    has_episode: bool,
    is_anime_format: bool,
    ai_result: &Option<ai::ClassificationResult>,
    tmdb_data: Option<&metadata::MetadataResult>,
    has_japanese_audio: bool,
) -> String {
    let is_animation = tmdb_data
        .map(|t| metadata::has_animation_genre(&t.genre_ids))
        .unwrap_or(false);
    let is_japanese = tmdb_data
        .map(|t| metadata::is_japanese_origin(&t.origin_country))
        .unwrap_or(false);
    let ai_says_anime = ai_result
        .as_ref()
        .map(|r| r.media_type == "anime" || r.media_type == "anime_movie")
        .unwrap_or(false);

    if has_episode {
        // TV-like content
        if (is_animation && (is_japanese || has_japanese_audio)) || is_anime_format || ai_says_anime {
            "Anime".to_string()
        } else if is_animation {
            "Animated Shows".to_string()
        } else {
            "Shows".to_string()
        }
    } else {
        // Movie-like content
        let ai_says_anime_movie = ai_result
            .as_ref()
            .map(|r| r.media_type == "anime_movie")
            .unwrap_or(false);

        if (is_animation && (is_japanese || has_japanese_audio)) || ai_says_anime_movie {
            "Anime Movies".to_string()
        } else if is_animation {
            "Animated Movies".to_string()
        } else if tmdb_data
            .map(|t| t.genre_ids.contains(&99))
            .unwrap_or(false)
        {
            "Documentary".to_string()
        } else {
            "Movies".to_string()
        }
    }
}

fn determine_genre(tmdb_data: Option<&metadata::MetadataResult>, format: &str) -> String {
    let genre_ids = match tmdb_data {
        Some(t) => &t.genre_ids,
        None => return "Uncategorized".to_string(),
    };

    // Try each genre ID until we find one that maps to this format
    for &gid in genre_ids {
        // Skip Animation genre ID (handled at format level)
        if gid == 16 {
            continue;
        }
        if let Some(genre_name) = formats::map_tmdb_genre(gid, format) {
            return genre_name.to_string();
        }
    }

    // If we have TMDb data but no genre mapped, use "General" instead of "Uncategorized"
    // This happens when e.g. a Documentary's only TMDb genre is 99 (Documentary) —
    // the format-level genre doesn't repeat inside the format's genre list.
    if !genre_ids.is_empty() {
        return "General".to_string();
    }

    "Uncategorized".to_string()
}
