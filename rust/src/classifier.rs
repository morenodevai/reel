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
///
/// `current_format` is an optional hint from the rescan path — the format
/// directory the file currently lives in.  When a fresh classification would
/// downgrade a specific format (e.g. Anime → Shows) without a strong signal,
/// the current format is preserved to avoid re-organization regressions.
pub fn classify(
    filename: &str,
    parsed: &renamer::ParsedFilename,
    tmdb_data: Option<&metadata::MetadataResult>,
    has_japanese_audio: bool,
    current_format: Option<&str>,
) -> ClassificationOutput {
    // Get AI signal if available
    let ai_result = if ai::is_classifier_ready() {
        ai::classify_filename(filename).ok()
    } else {
        None
    };

    let has_episode = parsed.season.is_some() || parsed.episode.is_some();

    // Determine format using decision matrix
    let mut format = determine_format(has_episode, parsed.is_anime_format, &ai_result, tmdb_data, has_japanese_audio);

    // Preserve current format when reclassification would downgrade without
    // a strong counter-signal (e.g. Anime → Shows with no positive Shows evidence).
    if let Some(cur) = current_format {
        if cur != format && is_format_downgrade(cur, &format) {
            log::info!(
                "[classify] Preserving current format '{}' (new '{}' would be a downgrade for '{}')",
                cur, format, filename
            );
            format = cur.to_string();
        }
    }

    // Determine genre from TMDb data
    let genre = determine_genre(tmdb_data, &format);

    // Confidence reflects actual signal strength, not a flat fallback.
    // AI provides its own confidence (0.30–0.85).  Without AI, base
    // confidence on whether TMDb returned data and whether the genre
    // resolved to something useful.
    let confidence = if let Some(ref ai) = ai_result {
        ai.confidence
    } else {
        match (tmdb_data.is_some(), genre.as_str()) {
            (true, g) if g != "Uncategorized" && g != "General" => 0.70,
            (true, _) => 0.50,   // TMDb matched but genre didn't map
            (false, _) => 0.30,  // no TMDb, no AI — pure heuristics
        }
    };

    ClassificationOutput {
        format,
        genre,
        confidence,
    }
}

/// A "downgrade" is when a more specific format (Anime, Anime Movies) would
/// be replaced by a less specific one (Shows, Movies, Animated Shows/Movies).
/// This happens during rescan when the renamed filename loses the original
/// anime bracket format and the AI model isn't loaded.
fn is_format_downgrade(current: &str, proposed: &str) -> bool {
    matches!(
        (current, proposed),
        ("Anime", "Shows")
            | ("Anime", "Animated Shows")
            | ("Anime Movies", "Movies")
            | ("Anime Movies", "Animated Movies")
    )
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

    // TMDb "original_language" == "ja" is a strong anime signal even when
    // origin_country is missing or set to a licensor's country.
    let tmdb_is_japanese_lang = tmdb_data
        .map(|t| metadata::is_japanese_language(t.original_language.as_deref()))
        .unwrap_or(false);

    if has_episode {
        // TV-like content
        if (is_animation && (is_japanese || tmdb_is_japanese_lang || has_japanese_audio))
            || is_anime_format
            || ai_says_anime
        {
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

        if (is_animation && (is_japanese || tmdb_is_japanese_lang || has_japanese_audio))
            || ai_says_anime_movie
        {
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

    // Iterate the format's genre list in priority order (defined in formats.rs)
    // rather than TMDb's genre_ids order.  This guarantees the same franchise
    // always gets the same genre regardless of how TMDb orders its genre_ids
    // (e.g. Kung Fu Panda 3 has [28,10751] while 1/2/4 have [10751,28]).
    let non_animation_ids: Vec<u32> = genre_ids.iter().copied().filter(|&g| g != 16).collect();
    if let Some(format_def) = formats::FORMATS.iter().find(|f| f.name == format) {
        for &format_genre in format_def.genres {
            if non_animation_ids.iter().any(|&gid| {
                formats::map_tmdb_genre(gid, format) == Some(format_genre)
            }) {
                return format_genre.to_string();
            }
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
