use anyhow::{bail, Context, Result};
use std::env;

/// What kind of media to attach to the Discord notification when a human is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyMedia {
    /// Attach the still frame that triggered the detection.
    Photo,
    /// Record a short video clip (via ffmpeg) after detection and attach that instead.
    Video,
}

/// Runtime configuration, loaded from environment variables (optionally via a .env file).
#[derive(Debug, Clone)]
pub struct Config {
    /// NVIDIA NIM API key (starts with "nvapi-"). Get one at https://build.nvidia.com
    pub nim_api_key: String,
    /// Chat-completions endpoint. Default is NVIDIA's hosted NIM endpoint; point this at
    /// a self-hosted NIM container (e.g. http://localhost:8000/v1/chat/completions) instead.
    pub nim_api_url: String,
    /// Vision-language model to use. Must be a NIM VLM that accepts image_url content.
    pub nim_model: String,
    /// Discord webhook URL to notify.
    pub discord_webhook_url: String,
    /// How often to capture and check a frame, in seconds.
    pub poll_interval_secs: u64,
    /// Minimum time between two Discord notifications, in seconds (avoids spam).
    pub cooldown_secs: u64,
    /// ffmpeg video input device / index (platform specific, see README).
    pub camera_input: String,
    /// Whether to attach any media at all to the Discord notification.
    pub attach_media: bool,
    /// Photo or video, when attach_media is true.
    pub notify_media: NotifyMedia,
    /// Length of the video clip to record, in seconds, when notify_media is Video.
    pub video_duration_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Silently ignore a missing .env file — env vars set another way are fine too.
        let _ = dotenvy::dotenv();

        let notify_media = match env::var("NOTIFY_MEDIA") {
            Ok(v) => match v.to_ascii_lowercase().as_str() {
                "photo" | "image" => NotifyMedia::Photo,
                "video" => NotifyMedia::Video,
                other => bail!(r#"NOTIFY_MEDIA must be "photo" or "video", got "{other}""#),
            },
            Err(_) => NotifyMedia::Photo,
        };

        Ok(Config {
            nim_api_key: env::var("NIM_API_KEY")
                .context("NIM_API_KEY is not set (get a key at https://build.nvidia.com)")?,
            nim_api_url: env::var("NIM_API_URL").unwrap_or_else(|_| {
                "https://integrate.api.nvidia.com/v1/chat/completions".to_string()
            }),
            nim_model: env::var("NIM_MODEL")
                .unwrap_or_else(|_| "meta/llama-3.2-11b-vision-instruct".to_string()),
            discord_webhook_url: env::var("DISCORD_WEBHOOK_URL")
                .context("DISCORD_WEBHOOK_URL is not set")?,
            poll_interval_secs: env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            cooldown_secs: env::var("COOLDOWN_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            camera_input: env::var("CAMERA_INPUT").unwrap_or_else(|_| default_camera_input()),
            attach_media: env::var("ATTACH_MEDIA")
                .or_else(|_| env::var("ATTACH_IMAGE")) // backwards-compatible old name
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
            notify_media,
            video_duration_secs: env::var("VIDEO_DURATION_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
        })
    }
}

/// A reasonable default ffmpeg input for grabbing a single webcam frame, per OS.
fn default_camera_input() -> String {
    if cfg!(target_os = "macos") {
        "0".to_string() // avfoundation device index
    } else if cfg!(target_os = "windows") {
        "video=Integrated Camera".to_string() // dshow device name, adjust as needed
    } else {
        "/dev/video0".to_string() // v4l2 device path
    }
}
