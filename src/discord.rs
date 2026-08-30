use anyhow::{bail, Context, Result};
use reqwest::multipart::{Form, Part};
use serde_json::json;

/// Sends a Discord webhook notification. If `image_bytes` is `Some`, the frame that
/// triggered the detection is attached to the message.
pub async fn notify(
    client: &reqwest::Client,
    webhook_url: &str,
    message: &str,
    image_bytes: Option<&[u8]>,
) -> Result<()> {
    let resp = if let Some(bytes) = image_bytes {
        let payload_json = json!({ "content": message }).to_string();
        let part = Part::bytes(bytes.to_vec())
            .file_name("detection.jpg")
            .mime_str("image/jpeg")
            .context("failed to build multipart image part")?;
        let form = Form::new()
            .text("payload_json", payload_json)
            .part("file", part);

        client
            .post(webhook_url)
            .multipart(form)
            .send()
            .await
            .context("failed to send Discord webhook (with attachment)")?
    } else {
        client
            .post(webhook_url)
            .json(&json!({ "content": message }))
            .send()
            .await
            .context("failed to send Discord webhook")?
    };

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("Discord webhook returned {status}: {text}");
    }

    Ok(())
}
