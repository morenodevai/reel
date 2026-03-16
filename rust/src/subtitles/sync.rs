/// Subtitle timing synchronization — auto-adjust timestamps to match video audio.
///
/// Extracts audio via bundled ffmpeg, runs energy-based voice activity detection,
/// then cross-correlates speech timing with subtitle timing to find the best
/// global offset. Handles offsets up to ±5 minutes at 50ms resolution.
///
/// This replaces the functionality of autosubsync / ffsubsync for Reel's pipeline.

use once_cell::sync::Lazy;
use std::path::Path;

// === Configuration ===

/// Duration of audio to analyze (seconds).
const ANALYSIS_DURATION_SECS: u32 = 600;

/// Duration of analysis window in milliseconds.
const ANALYSIS_DURATION_MS: i64 = ANALYSIS_DURATION_SECS as i64 * 1000;

/// Cross-correlation resolution (milliseconds per bin).
const RESOLUTION_MS: i64 = 50;

/// Maximum offset to test (±5 minutes in bins).
const MAX_SHIFT_BINS: i64 = 300_000 / RESOLUTION_MS;

/// Minimum offset to apply (ignore tiny shifts).
const MIN_OFFSET_MS: i64 = 500;

/// Minimum confidence ratio (best_score / baseline_score) to apply shift.
const MIN_CONFIDENCE: f64 = 1.5;

/// VAD frame size in samples at 16kHz (30ms frames).
const FRAME_SAMPLES: usize = 480;

/// Minimum speech segment duration in milliseconds.
const MIN_SEGMENT_MS: i64 = 200;

/// Hangover duration — keep speech active for this long after energy drops (ms).
const HANGOVER_MS: i64 = 250;

/// Minimum RMS energy threshold — below this, audio is considered silent.
const MIN_ENERGY_THRESHOLD: f64 = 50.0;

/// Windows CREATE_NO_WINDOW flag for subprocess spawning.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// SRT timing regex: `HH:MM:SS,mmm --> HH:MM:SS,mmm`
static SRT_TIMING_RE: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(
        r"(\d{2}):(\d{2}):(\d{2}),(\d{3})\s*-->\s*(\d{2}):(\d{2}):(\d{2}),(\d{3})",
    )
    .unwrap()
});

/// Cached ffmpeg availability check (runs once per process).
static FFMPEG_AVAILABLE: Lazy<bool> = Lazy::new(|| {
    match super::get_ffmpeg_path() {
        Some(path) => {
            let mut cmd = std::process::Command::new(&path);
            cmd.arg("-version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            cmd.status().map(|s| s.success()).unwrap_or(false)
        }
        None => false,
    }
});

// === Public API ===

/// Auto-sync subtitle timing to the video audio.
/// Only processes .srt files — other formats are skipped gracefully.
pub(crate) fn sync_subtitle(video_path: &Path, srt_path: &Path) -> Result<(), String> {
    // Only sync .srt files — .ass/.sub have different timing formats
    let ext = srt_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "srt" {
        log::info!("[sync] Skipping non-SRT file: {}", srt_path.display());
        return Ok(());
    }

    if !*FFMPEG_AVAILABLE {
        log::warn!(
            "[sync] ffmpeg not available — cannot sync {}",
            srt_path.display()
        );
        return Ok(());
    }

    let video_name = video_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?");
    log::info!("[sync] Starting subtitle sync for {}", video_name);

    let temp_wav = srt_path.with_extension("_sync_tmp.wav");

    // Extract first 10 minutes of audio as 16kHz mono PCM
    if let Err(e) = extract_audio_wav(video_path, &temp_wav, ANALYSIS_DURATION_SECS) {
        cleanup_temp(&temp_wav);
        return Err(format!("Audio extraction failed: {}", e));
    }

    let audio_segments = match detect_speech_segments(&temp_wav) {
        Ok(s) => s,
        Err(e) => {
            cleanup_temp(&temp_wav);
            return Err(format!("Speech detection failed: {}", e));
        }
    };
    cleanup_temp(&temp_wav);

    if audio_segments.is_empty() {
        log::warn!(
            "[sync] No speech detected in audio — skipping sync for {}",
            video_name
        );
        return Ok(());
    }

    let sub_segments = match parse_srt_timing(srt_path) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("SRT parse failed: {}", e));
        }
    };

    if sub_segments.is_empty() {
        log::warn!(
            "[sync] No timing entries in SRT — skipping sync for {}",
            video_name
        );
        return Ok(());
    }

    log::info!(
        "[sync] {} audio speech segments, {} subtitle entries for {}",
        audio_segments.len(),
        sub_segments.len(),
        video_name
    );

    // Phase 1: Detect and correct framerate mismatch using ffprobe metadata.
    // Compares video duration with last subtitle timestamp to detect PAL↔NTSC conversion.
    let sub_segments = detect_and_fix_framerate(video_path, srt_path, sub_segments, video_name)?;

    let (offset_ms, score, baseline) = find_best_offset(&audio_segments, &sub_segments);

    // Already aligned — no shift needed
    if offset_ms == 0 {
        log::info!(
            "[sync] Already aligned for {} (score={})",
            video_name,
            score
        );
        return Ok(());
    }

    // Only apply if offset is meaningful (>500ms) and score is significantly above baseline
    let confidence = if baseline > 0 {
        score as f64 / baseline as f64
    } else {
        score as f64
    };

    if offset_ms.abs() > MIN_OFFSET_MS && confidence > MIN_CONFIDENCE {
        log::info!(
            "[sync] Shifting {} by {}ms (score={}, baseline={}, confidence={:.2})",
            video_name,
            offset_ms,
            score,
            baseline,
            confidence
        );
        shift_srt(srt_path, offset_ms)?;
    } else {
        log::info!(
            "[sync] No shift applied for {} (offset={}ms, score={}, baseline={}, confidence={:.2})",
            video_name,
            offset_ms,
            score,
            baseline,
            confidence
        );
    }

    Ok(())
}

// === Implementation ===

fn cleanup_temp(path: &Path) {
    if let Err(e) = std::fs::remove_file(path) {
        log::debug!(
            "[sync] Failed to clean up temp file {}: {}",
            path.display(),
            e
        );
    }
}

fn extract_audio_wav(
    video_path: &Path,
    wav_path: &Path,
    duration_secs: u32,
) -> Result<(), String> {
    let ffmpeg =
        super::get_ffmpeg_path().ok_or_else(|| "Bundled ffmpeg not found".to_string())?;
    let video_str = video_path
        .to_str()
        .ok_or_else(|| format!("Non-UTF-8 video path: {}", video_path.display()))?;
    let wav_str = wav_path
        .to_str()
        .ok_or_else(|| format!("Non-UTF-8 WAV path: {}", wav_path.display()))?;
    let mut cmd = std::process::Command::new(&ffmpeg);
    cmd.args([
        "-y",
        "-i",
        video_str,
        "-vn",
        "-acodec",
        "pcm_s16le",
        "-ar",
        "16000",
        "-ac",
        "1",
        "-t",
        &duration_secs.to_string(),
        wav_str,
    ])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd
        .status()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !status.success() {
        return Err("ffmpeg exited with error".to_string());
    }
    Ok(())
}

/// Energy-based voice activity detection with hangover.
///
/// Uses adaptive threshold from energy distribution (25th/75th percentile gap
/// as the speech/noise boundary). Includes hangover to merge nearby speech
/// segments that would otherwise be split by brief pauses.
fn detect_speech_segments(wav_path: &Path) -> Result<Vec<(i64, i64)>, String> {
    let data = std::fs::read(wav_path).map_err(|e| format!("Failed to read WAV: {}", e))?;

    let pcm_offset =
        find_wav_data_offset(&data).ok_or("WAV file missing data chunk".to_string())?;
    if pcm_offset >= data.len() {
        return Err("WAV file too small".to_string());
    }
    let pcm = &data[pcm_offset..];

    let frame_bytes = FRAME_SAMPLES * 2;
    let num_frames = pcm.len() / frame_bytes;

    if num_frames == 0 {
        return Ok(Vec::new());
    }

    // Compute RMS energy per 30ms frame
    let mut energies = Vec::with_capacity(num_frames);
    for i in 0..num_frames {
        let start = i * frame_bytes;
        let mut sum_sq: f64 = 0.0;
        for j in 0..FRAME_SAMPLES {
            let offset = start + j * 2;
            if offset + 1 < pcm.len() {
                let sample = i16::from_le_bytes([pcm[offset], pcm[offset + 1]]) as f64;
                sum_sq += sample * sample;
            }
        }
        energies.push((sum_sq / FRAME_SAMPLES as f64).sqrt());
    }

    // Adaptive threshold from energy distribution
    let mut sorted = energies.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p25 = sorted[sorted.len() / 4];
    let p75 = sorted[sorted.len() * 3 / 4];

    let threshold = if p75 > p25 * 1.5 {
        p25 + (p75 - p25) * 0.5
    } else {
        p75 * 1.5
    };

    // Audio too quiet for meaningful VAD
    if threshold < MIN_ENERGY_THRESHOLD {
        log::debug!(
            "[sync] Audio energy too low for VAD (threshold={:.0})",
            threshold
        );
        return Ok(Vec::new());
    }

    // Segment detection with hangover
    let frame_ms = 30i64;
    let hangover_frames = (HANGOVER_MS / frame_ms) as usize;
    let mut segments = Vec::new();
    let mut in_speech = false;
    let mut seg_start = 0i64;
    let mut hangover_remaining = 0usize;

    for (i, &energy) in energies.iter().enumerate() {
        let t = i as i64 * frame_ms;
        if energy > threshold {
            if !in_speech {
                seg_start = t;
                in_speech = true;
            }
            hangover_remaining = hangover_frames;
        } else if in_speech {
            if hangover_remaining > 0 {
                hangover_remaining -= 1;
            } else {
                if t - seg_start >= MIN_SEGMENT_MS {
                    segments.push((seg_start, t));
                }
                in_speech = false;
            }
        }
    }
    if in_speech {
        let end = num_frames as i64 * frame_ms;
        if end - seg_start >= MIN_SEGMENT_MS {
            segments.push((seg_start, end));
        }
    }

    log::debug!(
        "[sync] VAD: {} frames, threshold={:.0}, p25={:.0}, p75={:.0}, {} speech segments",
        num_frames,
        threshold,
        p25,
        p75,
        segments.len()
    );

    Ok(segments)
}

/// Walk RIFF chunk structure to find the PCM data offset.
fn find_wav_data_offset(data: &[u8]) -> Option<usize> {
    // Verify RIFF/WAVE header
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12; // skip past "RIFF" + size + "WAVE"
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;
        if chunk_id == b"data" {
            return Some(pos + 8);
        }
        pos += 8 + chunk_size;
        // Chunks are word-aligned
        if chunk_size % 2 != 0 {
            pos += 1;
        }
    }
    None
}

fn parse_srt_timing(srt_path: &Path) -> Result<Vec<(i64, i64)>, String> {
    let content =
        std::fs::read_to_string(srt_path).map_err(|e| format!("Failed to read SRT: {}", e))?;

    let mut segments = Vec::new();
    for caps in SRT_TIMING_RE.captures_iter(&content) {
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

/// Known framerate conversion ratios (PAL ↔ NTSC).
const FRAMERATE_RATIOS: &[(f64, &str)] = &[
    (25.0 / 23.976, "PAL→NTSC (25→23.976)"),
    (23.976 / 25.0, "NTSC→PAL (23.976→25)"),
    (24.0 / 23.976, "24→23.976"),
    (23.976 / 24.0, "23.976→24"),
    (25.0 / 24.0, "PAL→Film (25→24)"),
    (24.0 / 25.0, "Film→PAL (24→25)"),
];

/// Tolerance for framerate ratio matching (0.2%).
const FRAMERATE_TOLERANCE: f64 = 0.002;

/// Detect framerate mismatch between video and subtitle using ffprobe duration.
/// If a known framerate conversion is detected, rescales all subtitle timestamps
/// and rewrites the SRT file. Returns the (possibly adjusted) subtitle segments.
fn detect_and_fix_framerate(
    video_path: &Path,
    srt_path: &Path,
    sub_segments: Vec<(i64, i64)>,
    video_name: &str,
) -> Result<Vec<(i64, i64)>, String> {
    // Get video duration from ffprobe
    let video_duration_ms = match crate::identify::probe::probe_file_metadata(video_path) {
        Some(meta) => match meta.duration_secs {
            Some(d) => (d * 1000.0) as i64,
            None => {
                log::debug!("[sync] No duration from ffprobe for {}", video_name);
                return Ok(sub_segments);
            }
        },
        None => {
            log::debug!("[sync] ffprobe failed for {}", video_name);
            return Ok(sub_segments);
        }
    };

    // Get last subtitle timestamp
    let last_sub_end = match sub_segments.iter().map(|&(_, end)| end).max() {
        Some(end) if end > 60_000 => end, // need at least 1 minute of subs
        _ => return Ok(sub_segments),
    };

    // Compare ratio of last sub timestamp to video duration
    let ratio = last_sub_end as f64 / video_duration_ms as f64;

    // Check against known framerate conversion ratios
    for &(target_ratio, description) in FRAMERATE_RATIOS {
        if (ratio - target_ratio).abs() < FRAMERATE_TOLERANCE {
            let scale = 1.0 / target_ratio; // inverse to correct the mismatch

            log::info!(
                "[sync] Framerate mismatch detected for {}: {} (ratio={:.4}, scaling by {:.6})",
                video_name,
                description,
                ratio,
                scale
            );

            // Rescale all subtitle timestamps
            let rescaled: Vec<(i64, i64)> = sub_segments
                .iter()
                .map(|&(start, end)| {
                    ((start as f64 * scale) as i64, (end as f64 * scale) as i64)
                })
                .collect();

            // Rewrite the SRT file with corrected timing
            rescale_srt(srt_path, scale)?;

            return Ok(rescaled);
        }
    }

    log::debug!(
        "[sync] No framerate mismatch for {} (video={}ms, last_sub={}ms, ratio={:.4})",
        video_name,
        video_duration_ms,
        last_sub_end,
        ratio
    );
    Ok(sub_segments)
}

/// Rescale all SRT timestamps by a factor (for framerate correction).
fn rescale_srt(srt_path: &Path, scale: f64) -> Result<(), String> {
    let content =
        std::fs::read_to_string(srt_path).map_err(|e| format!("Failed to read SRT: {}", e))?;

    let adjusted = SRT_TIMING_RE.replace_all(&content, |caps: &regex::Captures| {
        let start = (srt_time_to_ms(&caps[1], &caps[2], &caps[3], &caps[4]) as f64 * scale) as i64;
        let end = (srt_time_to_ms(&caps[5], &caps[6], &caps[7], &caps[8]) as f64 * scale) as i64;
        format!(
            "{} --> {}",
            ms_to_srt_time(start.max(0)),
            ms_to_srt_time(end.max(0))
        )
    });

    std::fs::write(srt_path, adjusted.as_bytes())
        .map_err(|e| format!("Failed to write rescaled SRT: {}", e))
}

/// Cross-correlate audio speech timeline with subtitle timeline to find best offset.
///
/// Returns (offset_ms, best_score, baseline_score).
/// - offset_ms: how many ms to shift subtitles (positive = subs arrive later)
/// - best_score: correlation at best offset
/// - baseline_score: correlation at zero offset (for confidence calculation)
///
/// Uses sparse iteration on audio speech bins for performance.
fn find_best_offset(audio: &[(i64, i64)], subs: &[(i64, i64)]) -> (i64, i64, i64) {
    if audio.is_empty() || subs.is_empty() {
        return (0, 0, 0);
    }

    let bins = (ANALYSIS_DURATION_MS / RESOLUTION_MS) as usize;

    let mut audio_timeline = vec![false; bins];
    for &(start, end) in audio {
        let s = (start / RESOLUTION_MS).max(0) as usize;
        let e = (end / RESOLUTION_MS).min(bins as i64) as usize;
        for i in s..e.min(bins) {
            audio_timeline[i] = true;
        }
    }

    let mut sub_timeline = vec![false; bins];
    for &(start, end) in subs {
        let s = (start / RESOLUTION_MS).max(0) as usize;
        let e = (end / RESOLUTION_MS).min(bins as i64) as usize;
        for i in s..e.min(bins) {
            sub_timeline[i] = true;
        }
    }

    // Sparse iteration: only check bins where audio has speech
    let audio_ones: Vec<usize> = audio_timeline
        .iter()
        .enumerate()
        .filter(|(_, &v)| v)
        .map(|(i, _)| i)
        .collect();

    let mut best_score = 0i64;
    let mut best_offset = 0i64;
    let mut baseline_score = 0i64;

    for shift in -MAX_SHIFT_BINS..=MAX_SHIFT_BINS {
        let mut score = 0i64;
        for &i in &audio_ones {
            let shifted = i as i64 + shift;
            if shifted >= 0
                && (shifted as usize) < bins
                && sub_timeline[shifted as usize]
            {
                score += 1;
            }
        }
        if shift == 0 {
            baseline_score = score;
        }
        if score > best_score {
            best_score = score;
            best_offset = shift;
        }
    }

    // Negate: a positive best_offset means "sub bin i+N aligns with audio bin i",
    // i.e. subs are N bins ahead. To fix, subtract N from sub timestamps.
    (-best_offset * RESOLUTION_MS, best_score, baseline_score)
}

fn shift_srt(srt_path: &Path, offset_ms: i64) -> Result<(), String> {
    let content =
        std::fs::read_to_string(srt_path).map_err(|e| format!("Failed to read SRT: {}", e))?;

    let adjusted = SRT_TIMING_RE.replace_all(&content, |caps: &regex::Captures| {
        let start = (srt_time_to_ms(&caps[1], &caps[2], &caps[3], &caps[4]) + offset_ms).max(0);
        let end = (srt_time_to_ms(&caps[5], &caps[6], &caps[7], &caps[8]) + offset_ms).max(0);
        format!("{} --> {}", ms_to_srt_time(start), ms_to_srt_time(end))
    });

    std::fs::write(srt_path, adjusted.as_bytes())
        .map_err(|e| format!("Failed to write adjusted SRT: {}", e))
}
