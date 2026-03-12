/// ffprobe integration — extract container metadata from video files.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Text-based subtitle codecs that can be rendered directly by players.
/// Image-based codecs (hdmv_pgs_subtitle, dvd_subtitle) are bitmap overlays — they exist
/// but can't be extracted to .srt, so they don't replace a downloadable text sub.
const TEXT_SUB_CODECS: &[&str] = &[
    "subrip", "ass", "ssa", "webvtt", "mov_text",
    "text", "subviewer", "subviewer1", "microdvd",
    "jacosub", "sami", "realtext", "stl", "pjs",
];

/// A subtitle stream embedded in the media container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedSubtitle {
    /// ISO 639-2/B lowercase (e.g. "eng", "spa"), or "und" if untagged.
    pub language: String,
    /// ffprobe codec_name (e.g. "subrip", "ass", "hdmv_pgs_subtitle").
    pub codec: String,
    /// True for text-based codecs that players can render as searchable text.
    /// False for image-based codecs (PGS, VobSub) that are bitmap overlays.
    pub is_text_based: bool,
    /// True if the stream's disposition has forced=1. Forced subs only cover
    /// foreign-language dialogue, not full subtitles — they should NOT prevent
    /// downloading full subs for a language.
    pub is_forced: bool,
}

/// Metadata extracted from a media container via ffprobe.
pub struct ProbeMetadata {
    pub title: Option<String>,
    pub year: Option<u16>,
    pub audio_languages: Vec<String>,
    pub has_japanese_audio: bool,
    pub duration_secs: Option<f64>,
    pub subtitle_streams: Vec<EmbeddedSubtitle>,
}

/// Get path to the bundled ffprobe binary in the app's config directory.
pub fn get_ffprobe_path() -> Option<std::path::PathBuf> {
    let bin_name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    let path = crate::config::config_dir()
        .join("bin")
        .join(bin_name);

    if path.exists() { Some(path) } else { None }
}

/// Probe a video file's container metadata using ffprobe.
/// Returns None if ffprobe is unavailable, execution fails, or no useful tags are present.
pub fn probe_file_metadata(video_path: &Path) -> Option<ProbeMetadata> {
    let ffprobe = get_ffprobe_path()?;
    let path_str = video_path.to_str()?;

    let mut cmd = std::process::Command::new(&ffprobe);
    cmd.args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path_str,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd.output()
        .map_err(|e| log::debug!("[probe] ffprobe execution failed for {}: {}", video_path.display(), e))
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| log::debug!("[probe] Failed to parse ffprobe output for {}: {}", video_path.display(), e))
        .ok()?;

    let format_tags = json.get("format").and_then(|f| f.get("tags"));

    let title = format_tags
        .and_then(|tags| {
            tags.get("title")
                .or_else(|| tags.get("TITLE"))
                .or_else(|| tags.get("Title"))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let year = format_tags
        .and_then(|tags| {
            tags.get("DATE_RELEASED")
                .or_else(|| tags.get("date"))
                .or_else(|| tags.get("DATE"))
                .or_else(|| tags.get("year"))
                .or_else(|| tags.get("YEAR"))
                .or_else(|| tags.get("creation_time"))
        })
        .and_then(|v| v.as_str())
        .and_then(|s| {
            s.parse::<u16>().ok().or_else(|| {
                if s.len() >= 4 {
                    s[..4].parse::<u16>().ok().filter(|&y| (1900..=2100).contains(&y))
                } else {
                    None
                }
            })
        });

    let duration_secs = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok());

    let mut audio_languages = Vec::new();
    let mut subtitle_streams = Vec::new();

    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            let codec_type = stream.get("codec_type").and_then(|v| v.as_str());

            match codec_type {
                Some("audio") => {
                    if let Some(lang) = stream_language(stream) {
                        audio_languages.push(lang);
                    }
                }
                Some("subtitle") => {
                    let codec = stream
                        .get("codec_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_lowercase();

                    // Untagged/unknown streams get "und" — they'll never match a config
                    // language, so they won't incorrectly prevent subtitle downloads.
                    let language = stream_language(stream).unwrap_or_else(|| "und".to_string());

                    let is_forced = stream
                        .get("disposition")
                        .and_then(|d| d.get("forced"))
                        .and_then(|v| v.as_i64())
                        == Some(1);

                    let is_text_based = TEXT_SUB_CODECS.contains(&codec.as_str());

                    subtitle_streams.push(EmbeddedSubtitle {
                        language,
                        codec,
                        is_text_based,
                        is_forced,
                    });
                }
                _ => {}
            }
        }
    }

    let has_japanese_audio = audio_languages.iter().any(|l| l == "jpn");

    if title.is_none() && year.is_none() && audio_languages.is_empty() && subtitle_streams.is_empty() {
        return None;
    }

    Some(ProbeMetadata {
        title,
        year,
        audio_languages,
        has_japanese_audio,
        duration_secs,
        subtitle_streams,
    })
}

/// Extract and normalize a language tag from a stream, checking common casing variants.
/// Normalizes ISO 639-2/T codes to 639-2/B (e.g. "fra"→"fre", "deu"→"ger") so that
/// comparisons against config language codes (which use 639-2/B) work correctly.
/// Returns None for "und"/"unk" (unknown/undefined).
fn stream_language(stream: &serde_json::Value) -> Option<String> {
    let lang = stream
        .get("tags")
        .and_then(|t| {
            t.get("language")
                .or_else(|| t.get("LANGUAGE"))
                .or_else(|| t.get("Language"))
        })
        .and_then(|v| v.as_str())?;

    let lang_lower = lang.to_lowercase();
    if lang_lower == "und" || lang_lower == "unk" {
        return None;
    }
    Some(crate::subtitles::normalize_lang(&lang_lower))
}
