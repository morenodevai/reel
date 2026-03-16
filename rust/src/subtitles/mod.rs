/// Subtitle discovery, downloading, and timing synchronization.

pub mod finder;
pub mod opensubs;
mod sync;

use std::path::Path;

// Hash identification and probe metadata live in identify/.
// Re-exported here so existing callers continue to work.
pub use crate::identify::hash::{compute_os_hash, HashIdentification, identify_by_hash};
pub use crate::identify::probe::{probe_file_metadata, EmbeddedSubtitle, ProbeMetadata};

// Use shared::video::SUBTITLE_EXTENSIONS as single source of truth.
pub(crate) use crate::shared::video::SUBTITLE_EXTENSIONS;

/// Single source of truth for language data. Used by `normalize_lang`, `detect_language`,
/// and `to_os_lang` — adding a language here covers all three functions.
struct Language {
    /// Recognized forms: full name, ISO 639-2/B, ISO 639-2/T, ISO 639-1.
    /// NOTE: "hi" excluded from Hindi — .hi. in subtitle filenames means SDH, not Hindi.
    aliases: &'static [&'static str],
    /// ISO 639-2/B (3-letter, used for normalization and detection)
    code_3: &'static str,
    /// ISO 639-1 (2-letter, used for OpenSubtitles API)
    code_2: &'static str,
}

static LANGUAGES: &[Language] = &[
    Language { aliases: &["english", "eng", "en"],          code_3: "eng", code_2: "en" },
    Language { aliases: &["spanish", "spa", "es"],          code_3: "spa", code_2: "es" },
    Language { aliases: &["french", "fre", "fra", "fr"],    code_3: "fre", code_2: "fr" },
    Language { aliases: &["german", "ger", "deu", "de"],    code_3: "ger", code_2: "de" },
    Language { aliases: &["japanese", "jpn", "ja"],          code_3: "jpn", code_2: "ja" },
    Language { aliases: &["portuguese", "por", "pt"],        code_3: "por", code_2: "pt" },
    Language { aliases: &["italian", "ita", "it"],           code_3: "ita", code_2: "it" },
    Language { aliases: &["russian", "rus", "ru"],           code_3: "rus", code_2: "ru" },
    Language { aliases: &["chinese", "chi", "zh"],           code_3: "chi", code_2: "zh" },
    Language { aliases: &["korean", "kor", "ko"],            code_3: "kor", code_2: "ko" },
    Language { aliases: &["arabic", "ara", "ar"],            code_3: "ara", code_2: "ar" },
    Language { aliases: &["hindi", "hin"],                   code_3: "hin", code_2: "hi" },
    Language { aliases: &["dutch", "dut", "nld", "nl"],      code_3: "dut", code_2: "nl" },
    Language { aliases: &["greek", "gre", "ell", "el"],      code_3: "gre", code_2: "el" },
    Language { aliases: &["indonesian", "ind", "id"],        code_3: "ind", code_2: "id" },
    Language { aliases: &["persian", "per", "fas", "fa"],    code_3: "per", code_2: "fa" },
    Language { aliases: &["finnish", "fin", "fi"],            code_3: "fin", code_2: "fi" },
    Language { aliases: &["swedish", "swe", "sv"],            code_3: "swe", code_2: "sv" },
    Language { aliases: &["turkish", "tur", "tr"],            code_3: "tur", code_2: "tr" },
    Language { aliases: &["polish", "pol", "pl"],             code_3: "pol", code_2: "pl" },
    Language { aliases: &["romanian", "rum", "ron", "ro"],    code_3: "rum", code_2: "ro" },
    Language { aliases: &["thai", "tha", "th"],               code_3: "tha", code_2: "th" },
    Language { aliases: &["vietnamese", "vie", "vi"],          code_3: "vie", code_2: "vi" },
    Language { aliases: &["czech", "cze", "ces", "cs"],       code_3: "cze", code_2: "cs" },
    Language { aliases: &["hungarian", "hun", "hu"],           code_3: "hun", code_2: "hu" },
    Language { aliases: &["norwegian", "nor", "no"],           code_3: "nor", code_2: "no" },
    Language { aliases: &["danish", "dan", "da"],              code_3: "dan", code_2: "da" },
    Language { aliases: &["hebrew", "heb", "he"],              code_3: "heb", code_2: "he" },
    Language { aliases: &["malay", "may", "msa", "ms"],       code_3: "may", code_2: "ms" },
    Language { aliases: &["ukrainian", "ukr", "uk"],           code_3: "ukr", code_2: "uk" },
    Language { aliases: &["croatian", "hrv", "hr"],            code_3: "hrv", code_2: "hr" },
    Language { aliases: &["bulgarian", "bul", "bg"],           code_3: "bul", code_2: "bg" },
];

/// Normalize a language code to ISO 639-2/B. Handles ISO 639-2/T variants
/// (e.g. "fra"→"fre", "deu"→"ger") and ISO 639-1 codes (e.g. "en"→"eng").
/// Returns the input unchanged if no mapping is found.
pub fn normalize_lang(code: &str) -> String {
    let lower = code.to_lowercase();
    for lang in LANGUAGES {
        if lang.aliases.contains(&lower.as_str()) {
            return lang.code_3.to_string();
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
        for lang in LANGUAGES {
            if lang.aliases.contains(part) {
                return lang.code_3.to_string();
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
        for lang in LANGUAGES {
            if lang.aliases.contains(&parent.as_str()) {
                return lang.code_3.to_string();
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
/// Unknown codes pass through unchanged (already 2-letter, or unrecognized).
pub(crate) fn to_os_lang(lang: &str) -> String {
    let lower = lang.to_lowercase();
    for l in LANGUAGES {
        if l.aliases.contains(&lower.as_str()) {
            return l.code_2.to_string();
        }
    }
    lower
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
