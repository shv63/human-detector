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

    let input_flag: &str = if cfg!(target_os = "macos") {
        "avfoundation"
    } else if cfg!(target_os = "windows") {
        "dshow"
    } else {
        "v4l2"
    };

    let status = Command::new("ffmpeg")
        .args([
            "-y", // overwrite output without prompting
            "-f",
            input_flag,
            "-i",
            camera_input,
            "-frames:v",
            "1",
            "-q:v",
            "2",
        ])
        .arg(&tmp)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
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
