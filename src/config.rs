use anyhow::{Context, Result};
use std::env;

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
    /// Whether to attach the captured frame to the Discord notification.
    pub attach_image: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Silently ignore a missing .env file — env vars set another way are fine too.
        let _ = dotenvy::dotenv();

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
            attach_image: env::var("ATTACH_IMAGE")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
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
