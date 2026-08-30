use anyhow::{bail, Context, Result};
use tokio::process::Command;
use tracing::warn;

/// Captures a single JPEG frame from the webcam by shelling out to `ffmpeg`.
///
/// Using ffmpeg instead of a native Rust camera crate keeps this dependency-light and
/// cross-platform: ffmpeg's `v4l2` / `avfoundation` / `dshow` input drivers cover
/// Linux, macOS, and Windows respectively. Install ffmpeg separately (e.g. `apt install
/// ffmpeg`, `brew install ffmpeg`, or download a build for Windows) and make sure it's
/// on PATH.
pub async fn capture_frame(camera_input: &str) -> Result<Vec<u8>> {
    let tmp = std::env::temp_dir().join(format!("human-detector-{}.jpg", std::process::id()));

    let status = ffmpeg_command(camera_input)
        .args(["-frames:v", "1", "-q:v", "2"])
        .arg(&tmp)
        .status()
        .await
        .context("failed to run ffmpeg — is it installed and on PATH?")?;

    if !status.success() {
        bail!(
            "ffmpeg exited with {status}; check CAMERA_INPUT (currently \"{camera_input}\") \
             matches an actual capture device"
        );
    }

    let bytes = tokio::fs::read(&tmp)
        .await
        .context("ffmpeg reported success but the output frame is missing")?;
    let _ = tokio::fs::remove_file(&tmp).await;
    Ok(bytes)
}

/// Lowest bitrate we'll drop to when backing off to hit a size target — below this the
/// clip stops being worth sending at all.
const MIN_VIDEO_BITRATE_KBPS: u64 = 100;
/// Highest bitrate we'll ever request — 640px-wide footage gains little past this, so a
/// generous size budget over a short clip shouldn't push quality (and file size) beyond it.
const MAX_VIDEO_BITRATE_KBPS: u64 = 2500;
/// How many encode attempts to make before giving up on hitting `max_bytes`.
const MAX_ENCODE_ATTEMPTS: u32 = 3;
/// Safety margin subtracted from `max_bytes` — MP4 container overhead and VBV bitrate
/// variance mean a clip targeted at exactly the cap can still land slightly over it.
const SIZE_SAFETY_MARGIN: f64 = 0.85;

/// Records a short H.264 MP4 clip from the webcam by shelling out to `ffmpeg`, sized to
/// try to fit under `max_bytes` (e.g. Discord's 8MB webhook attachment cap).
///
/// Rather than a fixed quality setting, the target bitrate is derived from
/// `max_bytes / duration_secs`, and ffmpeg is given a matching `-maxrate`/`-bufsize` so
/// the output is capped near that size instead of varying with scene content. If the
/// resulting file still comes out over `max_bytes` (VBV isn't a hard guarantee), it
/// retries at a lower bitrate up to `MAX_ENCODE_ATTEMPTS` times, then returns the
/// smallest attempt made (with a warning) even if it's still slightly over.
pub async fn capture_video(camera_input: &str, duration_secs: u64, max_bytes: u64) -> Result<Vec<u8>> {
    let mut bitrate_kbps = target_bitrate_kbps(max_bytes, duration_secs);
    let mut best: Option<Vec<u8>> = None;

    for attempt in 1..=MAX_ENCODE_ATTEMPTS {
        let bytes = encode_clip(camera_input, duration_secs, bitrate_kbps).await?;
        let size = bytes.len() as u64;

        if size <= max_bytes {
            return Ok(bytes);
        }

        warn!(
            "video clip attempt {attempt}/{MAX_ENCODE_ATTEMPTS} was {size} bytes, over the \
             {max_bytes} byte target at {bitrate_kbps}kbps"
        );

        let smaller_than_best = best.as_ref().map(|b| size < b.len() as u64).unwrap_or(true);
        if smaller_than_best {
            best = Some(bytes);
        }

        if bitrate_kbps <= MIN_VIDEO_BITRATE_KBPS {
            break;
        }
        bitrate_kbps = (bitrate_kbps * 2 / 3).max(MIN_VIDEO_BITRATE_KBPS);
    }

    let bytes = best.expect("at least one encode attempt always runs");
    warn!(
        "sending oversized video clip anyway ({} bytes > {max_bytes} byte target) — \
         consider a shorter VIDEO_DURATION_SECS",
        bytes.len()
    );
    Ok(bytes)
}

/// Computes a target video bitrate (in kbps) so that `duration_secs` of footage should
/// land under `max_bytes`, clamped to a sane quality range.
fn target_bitrate_kbps(max_bytes: u64, duration_secs: u64) -> u64 {
    let duration_secs = duration_secs.max(1);
    let target_bits = (max_bytes as f64) * SIZE_SAFETY_MARGIN * 8.0;
    let kbps = (target_bits / duration_secs as f64 / 1000.0) as u64;
    kbps.clamp(MIN_VIDEO_BITRATE_KBPS, MAX_VIDEO_BITRATE_KBPS)
}

/// Encodes one `duration_secs` clip at the given target bitrate.
async fn encode_clip(camera_input: &str, duration_secs: u64, bitrate_kbps: u64) -> Result<Vec<u8>> {
    let tmp = std::env::temp_dir().join(format!("human-detector-{}.mp4", std::process::id()));

    let bitrate = format!("{bitrate_kbps}k");
    let bufsize = format!("{}k", bitrate_kbps * 2);

    let status = ffmpeg_command(camera_input)
        .args([
            "-t",
            &duration_secs.to_string(),
            "-an", // no audio track
            "-vf",
            "scale=640:-2",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-b:v",
            &bitrate,
            "-maxrate",
            &bitrate,
            "-bufsize",
            &bufsize,
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(&tmp)
        .status()
        .await
        .context("failed to run ffmpeg — is it installed and on PATH?")?;

    if !status.success() {
        bail!(
            "ffmpeg exited with {status}; check CAMERA_INPUT (currently \"{camera_input}\") \
             matches an actual capture device"
        );
    }

    let bytes = tokio::fs::read(&tmp)
        .await
        .context("ffmpeg reported success but the output video is missing")?;
    let _ = tokio::fs::remove_file(&tmp).await;
    Ok(bytes)
}

/// Builds an ffmpeg `Command` with the platform-appropriate input driver and device
/// already set, ready for capture-specific args (`-frames:v 1 ...` or `-t N ...`) to be
/// appended.
fn ffmpeg_command(camera_input: &str) -> Command {
    let input_flag: &str = if cfg!(target_os = "macos") {
        "avfoundation"
    } else if cfg!(target_os = "windows") {
        "dshow"
    } else {
        "v4l2"
    };

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-f", input_flag, "-i", camera_input])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd
}
