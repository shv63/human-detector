mod camera;
mod config;
mod discord;
mod nim;

use anyhow::Result;
use chrono::Local;
use clap::Parser;
use config::Config;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Watches a webcam (or a single image file) for a human being, using an NVIDIA NIM
/// vision-language model, and pings a Discord webhook when one is spotted.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Run once against a specific image file instead of the webcam loop. Useful for
    /// testing your NIM key, model choice, and Discord webhook without a camera.
    #[arg(long)]
    image: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    let cfg = Config::from_env()?;
    let client = reqwest::Client::new();

    if let Some(path) = args.image {
        run_once(&client, &cfg, &path).await?;
        return Ok(());
    }

    run_loop(&client, &cfg).await
}

/// Single-shot mode: check one image file and exit. Does not apply the cooldown.
async fn run_once(client: &reqwest::Client, cfg: &Config, path: &str) -> Result<()> {
    info!("checking {path}");
    let bytes = tokio::fs::read(path).await?;
    let found = nim::detect_human(client, cfg, &bytes).await?;
    if found {
        info!("human detected — sending Discord notification");
        let message = format!(
            "🚨 Human detected at {}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        let attachment = if cfg.attach_image { Some(bytes.as_slice()) } else { None };
        discord::notify(client, &cfg.discord_webhook_url, &message, attachment).await?;
    } else {
        info!("no human detected");
    }
    Ok(())
}

/// Continuous mode: capture from the webcam every `poll_interval_secs`, and notify on
/// detection at most once per `cooldown_secs`.
async fn run_loop(client: &reqwest::Client, cfg: &Config) -> Result<()> {
    info!(
        "starting monitor: polling every {}s, cooldown {}s, camera input \"{}\"",
        cfg.poll_interval_secs, cfg.cooldown_secs, cfg.camera_input
    );

    let mut last_notified: Option<Instant> = None;

    loop {
        match tick(client, cfg, &mut last_notified).await {
            Ok(()) => {}
            Err(e) => error!("tick failed: {e:#}"),
        }
        tokio::time::sleep(Duration::from_secs(cfg.poll_interval_secs)).await;
    }
}

async fn tick(client: &reqwest::Client, cfg: &Config, last_notified: &mut Option<Instant>) -> Result<()> {
    let frame = camera::capture_frame(&cfg.camera_input).await?;
    let found = nim::detect_human(client, cfg, &frame).await?;

    if !found {
        info!("no human detected");
        return Ok(());
    }

    let on_cooldown = last_notified
        .map(|t| t.elapsed() < Duration::from_secs(cfg.cooldown_secs))
        .unwrap_or(false);

    if on_cooldown {
        warn!("human detected but still within cooldown — skipping notification");
        return Ok(());
    }

    info!("human detected — sending Discord notification");
    let message = format!(
        "🚨 Human detected at {}",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    let attachment = if cfg.attach_image { Some(frame.as_slice()) } else { None };
    discord::notify(client, &cfg.discord_webhook_url, &message, attachment).await?;
    *last_notified = Some(Instant::now());

    Ok(())
}
