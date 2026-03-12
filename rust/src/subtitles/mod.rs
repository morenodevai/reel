/// Subtitle discovery, downloading, and timing synchronization.

pub mod finder;
pub mod opensubs;
mod sync;

use once_cell::sync::Lazy;
use std::path::Path;

// Hash identification and probe metadata live in identify/.
// Re-exported here so existing callers continue to work.
pub use crate::identify::hash::{compute_os_hash, HashIdentification, identify_by_hash};
pub use crate::identify::probe::{probe_file_metadata, EmbeddedSubtitle, ProbeMetadata};

// Use shared::video::SUBTITLE_EXTENSIONS as single source of truth.
pub(crate) use crate::shared::video::SUBTITLE_EXTENSIONS;

static LANG_MAP: Lazy<Vec<(&str, &str)>> = Lazy::new(|| {
    vec![
        ("english", "eng"),
        ("eng", "eng"),
        ("en", "eng"),
        ("spanish", "spa"),
        ("spa", "spa"),
        ("es", "spa"),
        ("french", "fre"),
        ("fre", "fre"),
        ("fra", "fre"),
        ("fr", "fre"),
        ("german", "ger"),
        ("ger", "ger"),
        ("deu", "ger"),
        ("de", "ger"),
        ("japanese", "jpn"),
        ("jpn", "jpn"),
        ("ja", "jpn"),
        ("portuguese", "por"),
        ("por", "por"),
        ("pt", "por"),
        ("italian", "ita"),
        ("ita", "ita"),
        ("it", "ita"),
        ("russian", "rus"),
        ("rus", "rus"),
        ("ru", "rus"),
        ("chinese", "chi"),
        ("chi", "chi"),
        ("zh", "chi"),
        ("korean", "kor"),
        ("kor", "kor"),
        ("ko", "kor"),
        ("arabic", "ara"),
        ("ara", "ara"),
        ("ar", "ara"),
        ("hindi", "hin"),
        ("hin", "hin"),
        // NOTE: "hi" omitted — in subtitle filenames, .hi. means "hearing impaired" (SDH),
        // not Hindi. Use "hin" or "hindi" for Hindi detection.
        ("dutch", "dut"),
        ("dut", "dut"),
        ("nld", "dut"),
        ("nl", "dut"),
        ("greek", "gre"),
        ("gre", "gre"),
        ("ell", "gre"),
        ("el", "gre"),
        ("indonesian", "ind"),
        ("ind", "ind"),
        ("id", "ind"),
        ("persian", "per"),
        ("per", "per"),
        ("fas", "per"),
        ("fa", "per"),
        ("finnish", "fin"),
        ("fin", "fin"),
        ("fi", "fin"),
        ("swedish", "swe"),
        ("swe", "swe"),
        ("sv", "swe"),
        ("turkish", "tur"),
        ("tur", "tur"),
        ("tr", "tur"),
        ("polish", "pol"),
        ("pol", "pol"),
        ("pl", "pol"),
        ("romanian", "rum"),
        ("rum", "rum"),
        ("ron", "rum"),
        ("ro", "rum"),
        ("thai", "tha"),
        ("tha", "tha"),
        ("th", "tha"),
        ("vietnamese", "vie"),
        ("vie", "vie"),
        ("vi", "vie"),
        ("czech", "cze"),
        ("cze", "cze"),
        ("ces", "cze"),
        ("cs", "cze"),
        ("hungarian", "hun"),
        ("hun", "hun"),
        ("hu", "hun"),
        ("norwegian", "nor"),
        ("nor", "nor"),
        ("no", "nor"),
        ("danish", "dan"),
        ("dan", "dan"),
        ("da", "dan"),
        ("hebrew", "heb"),
        ("heb", "heb"),
        ("he", "heb"),
        ("malay", "may"),
        ("may", "may"),
        ("msa", "may"),
        ("ms", "may"),
        ("ukrainian", "ukr"),
        ("ukr", "ukr"),
        ("uk", "ukr"),
        ("croatian", "hrv"),
        ("hrv", "hrv"),
        ("hr", "hrv"),
        ("bulgarian", "bul"),
        ("bul", "bul"),
        ("bg", "bul"),
    ]
});

/// Normalize a language code to ISO 639-2/B. Handles ISO 639-2/T variants
/// (e.g. "fra"→"fre", "deu"→"ger") and ISO 639-1 codes (e.g. "en"→"eng").
/// Returns the input unchanged if no mapping is found.
pub fn normalize_lang(code: &str) -> String {
    let lower = code.to_lowercase();
    for (pattern, normalized) in LANG_MAP.iter() {
        if lower == *pattern {
            return normalized.to_string();
        }
    }
    lower
}

/// Detect the language of a subtitle file from its filename or path.
pub fn detect_language(path: &str) -> String {
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let parts: Vec<&str> = filename
        .split(|c: char| c == '.' || c == '_' || c == ' ' || c == '-')
        .collect();

    for part in &parts {
        for (pattern, code) in LANG_MAP.iter() {
            if *part == *pattern {
                return code.to_string();
            }
        }
    }

    let parent_names: Vec<String> = Path::new(path)
        .ancestors()
        .skip(1)
        .take(3)
        .filter_map(|p| p.file_name())
        .filter_map(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .collect();

    for parent in &parent_names {
        for (pattern, code) in LANG_MAP.iter() {
            if parent == *pattern {
                return code.to_string();
            }
        }
    }

    "und".to_string()
}

/// Check if a subtitle filename indicates forced subtitles.
pub fn is_forced(path: &str) -> bool {
    path.to_lowercase().contains("forced")
}

/// Check if a subtitle filename indicates SDH (Subtitles for Deaf/Hard of Hearing).
pub fn is_sdh(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("sdh") || lower.contains(".hi.")
}

/// Convert any language code/name to 2-letter ISO 639-1 for OpenSubtitles API.
pub(crate) fn to_os_lang(lang: &str) -> String {
    match lang.to_lowercase().as_str() {
        "eng" | "english" | "en" => "en",
        "spa" | "spanish" | "es" => "es",
        "fre" | "fra" | "french" | "fr" => "fr",
        "ger" | "deu" | "german" | "de" => "de",
        "jpn" | "japanese" | "ja" => "ja",
        "por" | "portuguese" | "pt" => "pt",
        "ita" | "italian" | "it" => "it",
        "rus" | "russian" | "ru" => "ru",
        "chi" | "chinese" | "zh" => "zh",
        "kor" | "korean" | "ko" => "ko",
        "ara" | "arabic" | "ar" => "ar",
        "hin" | "hindi" | "hi" => "hi",
        other => return other.to_string(),
    }
    .to_string()
}

/// Get path to the bundled ffmpeg binary in the app's config directory.
pub fn get_ffmpeg_path() -> Option<std::path::PathBuf> {
    let bin_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let path = crate::config::config_dir()
        .join("bin")
        .join(bin_name);

    if path.exists() { Some(path) } else { None }
}

// Public API — delegates to submodules.
pub use finder::{find_subtitles_dedicated, find_subtitles_local, matches_video_stem};
pub use opensubs::{download_subtitle, search_and_download};
