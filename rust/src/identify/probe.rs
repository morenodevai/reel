/// ffprobe integration — extract container metadata from video files.

use std::path::Path;

/// Metadata extracted from a media container via ffprobe.
pub struct ProbeMetadata {
    pub title: Option<String>,
    pub year: Option<u16>,
    pub audio_languages: Vec<String>,
    pub has_japanese_audio: bool,
    pub duration_secs: Option<f64>,
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
    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            if stream.get("codec_type").and_then(|v| v.as_str()) != Some("audio") {
                continue;
            }
            if let Some(lang) = stream
                .get("tags")
                .and_then(|t| {
                    t.get("language")
                        .or_else(|| t.get("LANGUAGE"))
                        .or_else(|| t.get("Language"))
                })
                .and_then(|v| v.as_str())
            {
                let lang_lower = lang.to_lowercase();
                if lang_lower != "und" && lang_lower != "unk" {
                    audio_languages.push(lang_lower);
                }
            }
        }
    }

    let has_japanese_audio = audio_languages
        .iter()
        .any(|l| l == "jpn" || l == "ja" || l == "japanese" || l == "jp");

    if title.is_none() && year.is_none() && audio_languages.is_empty() {
        return None;
    }

    Some(ProbeMetadata {
        title,
        year,
        audio_languages,
        has_japanese_audio,
        duration_secs,
    })
}
