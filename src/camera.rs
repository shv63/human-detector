use anyhow::{bail, Context, Result};
use tokio::process::Command;

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

/// Records a short H.264 MP4 clip from the webcam by shelling out to `ffmpeg`.
///
/// The clip is kept small (scaled down, no audio, a fairly aggressive CRF) since Discord
/// webhook attachments are capped at 8MB on non-boosted servers — a few seconds of video
/// fits comfortably, but a long `duration_secs` on a high-res camera may not.
pub async fn capture_video(camera_input: &str, duration_secs: u64) -> Result<Vec<u8>> {
    let tmp = std::env::temp_dir().join(format!("human-detector-{}.mp4", std::process::id()));

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
            "-crf",
            "28",
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
