use anyhow::{bail, Context, Result};
use reqwest::multipart::{Form, Part};
use serde_json::json;

/// A file to attach to a Discord webhook message.
pub struct Attachment<'a> {
    pub bytes: &'a [u8],
    pub filename: &'a str,
    pub mime_type: &'a str,
}

/// Sends a Discord webhook notification. If `attachment` is `Some`, the file is attached
/// to the message (e.g. the detection photo, or a short video clip).
pub async fn notify(
    client: &reqwest::Client,
    webhook_url: &str,
    message: &str,
    attachment: Option<Attachment<'_>>,
) -> Result<()> {
    let resp = if let Some(a) = attachment {
        let payload_json = json!({ "content": message }).to_string();
        let part = Part::bytes(a.bytes.to_vec())
            .file_name(a.filename.to_string())
            .mime_str(a.mime_type)
            .context("failed to build multipart attachment part")?;
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
