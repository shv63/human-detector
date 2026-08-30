use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;

use crate::config::Config;

/// Asks a NIM vision-language model whether a person appears in `image_bytes` (JPEG/PNG).
///
/// NIM's VLM endpoints follow the OpenAI chat-completions schema: the image is passed as
/// an `image_url` content part, and a `data:` URI is used instead of a public URL so the
/// frame never has to be hosted anywhere. See:
/// https://docs.nvidia.com/nim/vision-language-models/latest/api-reference.html
pub async fn detect_human(client: &reqwest::Client, cfg: &Config, image_bytes: &[u8]) -> Result<bool> {
    let b64 = STANDARD.encode(image_bytes);
    let data_uri = format!("data:image/jpeg;base64,{b64}");

    let body = json!({
        "model": cfg.nim_model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Look carefully at this image. Is there at least one human \
                                  being visible in it (any part of a person counts — face, \
                                  body, hand, etc.)? Reply with exactly one word: YES or NO."
                    },
                    {
                        "type": "image_url",
                        "image_url": { "url": data_uri }
                    }
                ]
            }
        ],
        "max_tokens": 5,
        "temperature": 0.0
    });

    let resp = client
        .post(&cfg.nim_api_url)
        .bearer_auth(&cfg.nim_api_key)
        .json(&body)
        .send()
        .await
        .context("request to NIM API failed")?;

    let status = resp.status();
    let text = resp.text().await.context("failed to read NIM API response body")?;

    if !status.is_success() {
        bail!("NIM API returned {status}: {text}");
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("could not parse NIM response as JSON: {text}"))?;

    let answer = parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();

    if answer.is_empty() {
        bail!("NIM response had no message content: {text}");
    }

    Ok(answer.contains("yes"))
}
