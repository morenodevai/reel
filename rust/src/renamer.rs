use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

static YEAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[\.\s\-\(]*((?:19|20)\d{2})[\.\s\-\)]*").unwrap());

static EPISODE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)S(\d{1,2})[\s\._-]*E(\d{1,3})(?:[\s\._-]*E(\d{1,3}))?").unwrap()
});

// Matches "1x01", "01x05", "2x12" format (common in torrents and Jellyfin libraries)
static EPISODE_NX_NN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(\d{1,2})x(\d{1,3})\b").unwrap()
});

static ANIME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[([^\]]+)\]\s*(.+?)\s*-\s*(\d+)").unwrap());

// Matches "Title - Episode N", "Title - EP N", "Title - Ep. N" (case insensitive)
static EPISODE_WORD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(.+?)\s*-\s*(?:Episode|EP\.?)\s*(\d+)").unwrap()
});

// Matches "Title - OVA", "Title - OVA 2", "Title - OVA 12" (Original Video Animation)
static OVA_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^(.+?)\s*-\s*OVA\s*(\d+)?").unwrap());

static RESOLUTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(1080[pi]|720[pi]|2160[pi]|4[Kk]|UHD)\b").unwrap());

static SOURCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(BluRay|BDRip|BRRip|WEB[-\.]?DL|WEB[-\.]?Rip|WEBRip|HDTV|HDRip|DVDRip|DVDScr|AMZN|NF|DSNP|HMAX|ATVP)\b").unwrap()
});

static CODEC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(x264|x265|H\.?264|H\.?265|HEVC|AVC|AAC|DTS|AC3|FLAC|Atmos|TrueHD|DD5\.1|DD\+?5\.?1|10bit)\b").unwrap()
});

static GROUP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:\[([^\]]+)\]|-([A-Za-z0-9]+))$").unwrap());

static BRACKET_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[.*?\]").unwrap());

static MULTI_SPACE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s+").unwrap());

/// Matches common extras/specials markers in filenames.
pub static EXTRAS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(creditless|NCOP|NCED|NC[\s._-]?OP|NC[\s._-]?ED|clean[\s._-]?(OP|ED)|textless|extras?|bonus|promo)\b").unwrap()
});

#[derive(Debug, Clone)]
pub struct ParsedFilename {
    pub title: String,
    pub year: Option<u16>,
    pub season: Option<u16>,
    pub episode: Option<u16>,
    pub episode_end: Option<u16>,
    pub is_anime_format: bool,
    pub is_extra: bool,
    pub release_group: Option<String>,
}

/// Parse a media filename into its components.
pub fn parse_filename(filename: &str) -> ParsedFilename {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    let is_extra = EXTRAS_RE.is_match(stem);

    // Try anime format first: [Group] Title - Episode (Quality)
    if let Some(caps) = ANIME_RE.captures(stem) {
        let group = caps.get(1).map(|m| m.as_str().to_string());
        let title = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let episode: Option<u16> = caps.get(3).and_then(|m| m.as_str().parse().ok());

        return ParsedFilename {
            title: clean_title(&title),
            year: None,
            season: Some(1), // Anime without explicit season = Season 1
            episode,
            episode_end: None,
            is_anime_format: true,
            is_extra,
            release_group: group,
        };
    }

    // Try "Title - Episode N" / "Title - EP N" format (common for anime without group tags)
    let stem_clean = stem.replace('.', " ").replace('_', " ");
    if let Some(caps) = EPISODE_WORD_RE.captures(&stem_clean) {
        let title = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let episode: Option<u16> = caps.get(2).and_then(|m| m.as_str().parse().ok());

        let release_group = GROUP_RE
            .captures(stem)
            .and_then(|caps| caps.get(1).or(caps.get(2)))
            .map(|m| m.as_str().to_string());

        return ParsedFilename {
            title: clean_title(&title),
            year: None,
            season: Some(1), // Anime without explicit season = Season 1
            episode,
            episode_end: None,
            is_anime_format: false,
            is_extra,
            release_group,
        };
    }

    // Try "Title - OVA" or "Title - OVA 2" format (Original Video Animation)
    if let Some(caps) = OVA_RE.captures(&stem_clean) {
        let title = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let ova_num: Option<u16> = caps.get(2).and_then(|m| m.as_str().parse().ok());

        let release_group = GROUP_RE
            .captures(stem)
            .and_then(|caps| caps.get(1).or(caps.get(2)))
            .map(|m| m.as_str().to_string());

        return ParsedFilename {
            title: clean_title(&title),
            year: None,
            season: Some(0), // OVA = Season 0 (Specials)
            episode: Some(ova_num.unwrap_or(0)), // OVA 2 → episode 2, bare OVA → episode 0
            episode_end: None,
            is_anime_format: false,
            is_extra,
            release_group,
        };
    }

    let clean = stem.replace('.', " ").replace('_', " ");

    // Extract season/episode (SxxExx first, then NxNN fallback)
    let (season, episode, episode_end) = if let Some(caps) = EPISODE_RE.captures(&clean) {
        let s: Option<u16> = caps.get(1).and_then(|m| m.as_str().parse().ok());
        let e: Option<u16> = caps.get(2).and_then(|m| m.as_str().parse().ok());
        let e2: Option<u16> = caps.get(3).and_then(|m| m.as_str().parse().ok());
        (s, e, e2)
    } else if let Some(caps) = EPISODE_NX_NN_RE.captures(&clean) {
        let s: Option<u16> = caps.get(1).and_then(|m| m.as_str().parse().ok());
        let e: Option<u16> = caps.get(2).and_then(|m| m.as_str().parse().ok());
        (s, e, None)
    } else {
        (None, None, None)
    };

    // Extract year
    let year: Option<u16> = YEAR_RE
        .captures(&clean)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse().ok());

    // Extract title: everything before year or first quality/source/episode indicator
    let title = extract_title(&clean, year);

    // Extract release group
    let release_group = GROUP_RE
        .captures(stem)
        .and_then(|caps| caps.get(1).or(caps.get(2)))
        .map(|m| m.as_str().to_string());

    ParsedFilename {
        title: clean_title(&title),
        year,
        season,
        episode,
        episode_end,
        is_anime_format: false,
        is_extra,
        release_group,
    }
}

fn extract_title(clean: &str, year: Option<u16>) -> String {
    // If we have an episode marker (SxxExx), title is everything before it
    if let Some(m) = EPISODE_RE.find(clean) {
        let before = clean[..m.start()].trim();
        if !before.is_empty() {
            return before.to_string();
        }
    }

    // If we have an episode marker (NxNN), title is everything before it
    if let Some(m) = EPISODE_NX_NN_RE.find(clean) {
        let before = clean[..m.start()].trim();
        if !before.is_empty() {
            return before.to_string();
        }
    }

    // If we have a year, title is everything before it
    if let Some(y) = year {
        let year_str = y.to_string();
        if let Some(pos) = clean.find(&year_str) {
            let before = clean[..pos].trim().trim_end_matches(|c: char| c == '(' || c == '[').trim();
            if !before.is_empty() {
                return before.to_string();
            }
        }
    }

    // Fallback: everything before first quality/source indicator
    let indicators = [&RESOLUTION_RE, &SOURCE_RE, &CODEC_RE];
    let mut earliest = clean.len();
    for re in &indicators {
        if let Some(m) = re.find(clean) {
            earliest = earliest.min(m.start());
        }
    }

    clean[..earliest].trim().to_string()
}

fn clean_title(title: &str) -> String {
    let mut cleaned = title.to_string();
    // Remove resolution, source, codec, group tags
    cleaned = RESOLUTION_RE.replace_all(&cleaned, "").to_string();
    cleaned = SOURCE_RE.replace_all(&cleaned, "").to_string();
    cleaned = CODEC_RE.replace_all(&cleaned, "").to_string();
    cleaned = GROUP_RE.replace_all(&cleaned, "").to_string();

    // Remove common bracket content
    cleaned = BRACKET_RE.replace_all(&cleaned, "").to_string();

    // Clean up whitespace
    cleaned = MULTI_SPACE_RE.replace_all(cleaned.trim(), " ").to_string();

    // Remove trailing hyphens/dots/brackets
    cleaned = cleaned.trim_end_matches(|c: char| c == '-' || c == ' ' || c == '(' || c == '[').trim().to_string();

    cleaned
}

/// Sanitize a string for use as a filesystem path component.
pub fn sanitize_for_fs(name: &str) -> String {
    name.replace(':', " -")
        .replace('?', "")
        .replace('*', "")
        .replace('<', "")
        .replace('>', "")
        .replace('|', "")
        .replace('"', "")
        .replace('/', "")
        .replace('\\', "")
}

/// Generate the title folder name: "Title (Year) [tmdbid-123]"
pub fn title_folder(title: &str, year: Option<u16>, tmdb_id: Option<u64>) -> String {
    let mut name = match year {
        Some(y) => format!("{} ({})", title, y),
        None => title.to_string(),
    };
    if let Some(id) = tmdb_id {
        name.push_str(&format!(" [tmdbid-{}]", id));
    }
    sanitize_for_fs(&name)
}

/// Generate the video filename for a movie: "Title (Year).ext"
pub fn movie_filename(title: &str, year: Option<u16>, ext: &str) -> String {
    let name = match year {
        Some(y) => format!("{} ({})", title, y),
        None => title.to_string(),
    };
    format!("{}.{}", sanitize_for_fs(&name), ext)
}

/// Generate the video filename for a show episode: "Show - S01E05 - Episode Title.ext"
pub fn episode_filename(
    show_title: &str,
    season: u16,
    episode: u16,
    episode_end: Option<u16>,
    episode_title: Option<&str>,
    ext: &str,
) -> String {
    let ep_part = match episode_end {
        Some(end) => format!("S{:02}E{:02}-E{:02}", season, episode, end),
        None => format!("S{:02}E{:02}", season, episode),
    };

    let name = match episode_title {
        Some(et) if !et.is_empty() => {
            format!("{} - {} - {}", show_title, ep_part, sanitize_for_fs(et))
        }
        _ => format!("{} - {}", show_title, ep_part),
    };

    format!("{}.{}", sanitize_for_fs(&name), ext)
}

/// Generate the subtitle filename: "video_name.lang.ext"
/// Preserves the original subtitle extension (srt, ass, ssa, sub, vtt).
pub fn subtitle_filename(video_stem: &str, lang: &str, forced: bool, sdh: bool, ext: &str) -> String {
    let mut name = format!("{}.{}", video_stem, lang);
    if forced {
        name.push_str(".forced");
    }
    if sdh {
        name.push_str(".sdh");
    }
    let sub_ext = if ext.is_empty() { "srt" } else { ext };
    name.push('.');
    name.push_str(sub_ext);
    name
}

/// Get the file extension from a path (without the dot).
pub fn get_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv")
        .to_string()
}

// Video file detection lives in shared::video. Re-exported here
// so existing `crate::renamer::is_video_file` imports continue to work.
pub use crate::shared::video::{is_video_extension, is_video_file};
