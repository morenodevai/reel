/// Subtitle timing synchronization — auto-adjust timestamps to match video audio.
///
/// Uses the bundled ffmpeg binary for audio extraction, then energy-based VAD
/// to find speech, then cross-correlates speech timing with subtitle timing
/// to find the best offset. Silently skips if ffmpeg is unavailable.

use once_cell::sync::Lazy;
use std::path::Path;

/// Auto-sync subtitle timing to the video audio.
pub(super) fn sync_subtitle(video_path: &Path, srt_path: &Path) -> Result<(), String> {
    if !is_ffmpeg_available() {
        return Ok(());
    }

    let temp_wav = srt_path.with_extension("_sync_tmp.wav");
    if let Err(e) = extract_audio_wav(video_path, &temp_wav, 600) {
        std::fs::remove_file(&temp_wav).ok();
        return Err(format!("Audio extraction failed: {}", e));
    }

    let audio_segments = match detect_speech_segments(&temp_wav) {
        Ok(s) => s,
        Err(e) => {
            std::fs::remove_file(&temp_wav).ok();
            return Err(format!("Speech detection failed: {}", e));
        }
    };

    let sub_segments = match parse_srt_timing(srt_path) {
        Ok(s) => s,
        Err(e) => {
            std::fs::remove_file(&temp_wav).ok();
            return Err(format!("SRT parse failed: {}", e));
        }
    };

    let offset_ms = find_best_offset(&audio_segments, &sub_segments);

    if offset_ms.abs() > 200 {
        shift_srt(srt_path, offset_ms)?;
    }

    std::fs::remove_file(&temp_wav).ok();
    Ok(())
}

fn is_ffmpeg_available() -> bool {
    match super::get_ffmpeg_path() {
        Some(path) => std::process::Command::new(&path)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false),
        None => false,
    }
}

fn extract_audio_wav(
    video_path: &Path,
    wav_path: &Path,
    duration_secs: u32,
) -> Result<(), String> {
    let ffmpeg = super::get_ffmpeg_path()
        .ok_or_else(|| "Bundled ffmpeg not found".to_string())?;
    let status = std::process::Command::new(&ffmpeg)
        .args([
            "-y",
            "-i",
            video_path.to_str().unwrap_or(""),
            "-vn",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-t",
            &duration_secs.to_string(),
            wav_path.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !status.success() {
        return Err("ffmpeg exited with error".to_string());
    }
    Ok(())
}

fn detect_speech_segments(wav_path: &Path) -> Result<Vec<(i64, i64)>, String> {
    let data = std::fs::read(wav_path).map_err(|e| format!("Failed to read WAV: {}", e))?;

    let pcm_offset = find_wav_data_offset(&data).unwrap_or(44);
    if pcm_offset >= data.len() {
        return Err("WAV file too small".to_string());
    }
    let pcm = &data[pcm_offset..];

    let frame_samples = 480usize;
    let frame_bytes = frame_samples * 2;
    let num_frames = pcm.len() / frame_bytes;

    if num_frames == 0 {
        return Ok(Vec::new());
    }

    let mut energies = Vec::with_capacity(num_frames);
    for i in 0..num_frames {
        let start = i * frame_bytes;
        let mut sum_sq: f64 = 0.0;
        for j in 0..frame_samples {
            let offset = start + j * 2;
            if offset + 1 < pcm.len() {
                let sample = i16::from_le_bytes([pcm[offset], pcm[offset + 1]]) as f64;
                sum_sq += sample * sample;
            }
        }
        energies.push((sum_sq / frame_samples as f64).sqrt());
    }

    let mut sorted_energies = energies.clone();
    sorted_energies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted_energies[sorted_energies.len() / 2];
    let threshold = median * 2.0;

    let frame_ms = 30i64;
    let mut segments = Vec::new();
    let mut in_speech = false;
    let mut seg_start = 0i64;

    for (i, &energy) in energies.iter().enumerate() {
        let t = i as i64 * frame_ms;
        if energy > threshold {
            if !in_speech {
                seg_start = t;
                in_speech = true;
            }
        } else if in_speech {
            if t - seg_start >= 300 {
                segments.push((seg_start, t));
            }
            in_speech = false;
        }
    }
    if in_speech {
        let end = num_frames as i64 * frame_ms;
        if end - seg_start >= 300 {
            segments.push((seg_start, end));
        }
    }

    Ok(segments)
}

fn find_wav_data_offset(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(4) {
        if &data[i..i + 4] == b"data" {
            return Some(i + 8);
        }
    }
    None
}

fn parse_srt_timing(srt_path: &Path) -> Result<Vec<(i64, i64)>, String> {
    let content =
        std::fs::read_to_string(srt_path).map_err(|e| format!("Failed to read SRT: {}", e))?;

    static TIME_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(
            r"(\d{2}):(\d{2}):(\d{2}),(\d{3})\s*-->\s*(\d{2}):(\d{2}):(\d{2}),(\d{3})",
        )
        .unwrap()
    });

    let mut segments = Vec::new();
    for caps in TIME_RE.captures_iter(&content) {
        let start_ms = srt_time_to_ms(&caps[1], &caps[2], &caps[3], &caps[4]);
        let end_ms = srt_time_to_ms(&caps[5], &caps[6], &caps[7], &caps[8]);
        segments.push((start_ms, end_ms));
    }

    Ok(segments)
}

fn srt_time_to_ms(h: &str, m: &str, s: &str, ms: &str) -> i64 {
    let h: i64 = h.parse().unwrap_or(0);
    let m: i64 = m.parse().unwrap_or(0);
    let s: i64 = s.parse().unwrap_or(0);
    let ms: i64 = ms.parse().unwrap_or(0);
    h * 3_600_000 + m * 60_000 + s * 1_000 + ms
}

fn ms_to_srt_time(ms: i64) -> String {
    let ms = ms.max(0);
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let frac = ms % 1_000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, frac)
}

fn find_best_offset(audio: &[(i64, i64)], subs: &[(i64, i64)]) -> i64 {
    if audio.is_empty() || subs.is_empty() {
        return 0;
    }

    let resolution = 100i64;
    let duration = 600_000i64;
    let bins = (duration / resolution) as usize;

    let mut audio_timeline = vec![false; bins];
    for &(start, end) in audio {
        let s = (start / resolution).max(0) as usize;
        let e = (end / resolution).min(bins as i64) as usize;
        for i in s..e.min(bins) {
            audio_timeline[i] = true;
        }
    }

    let mut sub_timeline = vec![false; bins];
    for &(start, end) in subs {
        let s = (start / resolution).max(0) as usize;
        let e = (end / resolution).min(bins as i64) as usize;
        for i in s..e.min(bins) {
            sub_timeline[i] = true;
        }
    }

    let max_shift = 600i64;
    let mut best_score = 0i64;
    let mut best_offset = 0i64;

    for shift in -max_shift..=max_shift {
        let mut score = 0i64;
        for i in 0..bins {
            let shifted = i as i64 + shift;
            if shifted >= 0 && (shifted as usize) < bins {
                if audio_timeline[i] && sub_timeline[shifted as usize] {
                    score += 1;
                }
            }
        }
        if score > best_score {
            best_score = score;
            best_offset = shift;
        }
    }

    best_offset * resolution
}

fn shift_srt(srt_path: &Path, offset_ms: i64) -> Result<(), String> {
    let content =
        std::fs::read_to_string(srt_path).map_err(|e| format!("Failed to read SRT: {}", e))?;

    static TIME_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(
            r"(\d{2}):(\d{2}):(\d{2}),(\d{3})\s*-->\s*(\d{2}):(\d{2}):(\d{2}),(\d{3})",
        )
        .unwrap()
    });

    let adjusted = TIME_RE.replace_all(&content, |caps: &regex::Captures| {
        let start = srt_time_to_ms(&caps[1], &caps[2], &caps[3], &caps[4]) + offset_ms;
        let end = srt_time_to_ms(&caps[5], &caps[6], &caps[7], &caps[8]) + offset_ms;
        format!("{} --> {}", ms_to_srt_time(start), ms_to_srt_time(end))
    });

    std::fs::write(srt_path, adjusted.as_bytes())
        .map_err(|e| format!("Failed to write adjusted SRT: {}", e))
}
