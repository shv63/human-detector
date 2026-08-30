# human-detector

Watches a webcam and pings a Discord webhook whenever a human shows up in frame,
using an NVIDIA NIM vision-language model to do the "is there a person here?" check.

## How it works

1. Every `POLL_INTERVAL_SECS`, it grabs one JPEG frame from your webcam (via `ffmpeg`).
2. It sends that frame to a NIM vision-language model (default:
   `meta/llama-3.2-11b-vision-instruct`) with the question "is there a human in this
   image?", using NIM's OpenAI-compatible chat-completions API.
3. If the model says yes, it posts to your Discord webhook — with either the triggering
   photo or a short video clip attached (your choice, see `NOTIFY_MEDIA` below) — but
   not more than once per `COOLDOWN_SECS`, so it won't spam you.

## Prerequisites

- **Rust** (a recent stable toolchain — `rustup` is the easiest way to get one:
  https://rustup.rs)
- **ffmpeg** on PATH — used to grab webcam frames:
  - Debian/Ubuntu: `sudo apt install ffmpeg`
  - macOS: `brew install ffmpeg`
  - Windows: https://ffmpeg.org/download.html
- An **NVIDIA NIM API key** — sign up at https://build.nvidia.com and generate one
  (there's a free credit tier). Any hosted VLM NIM works; you can also point this at
  a self-hosted NIM container instead (set `NIM_API_URL` to its local endpoint).
- A **Discord webhook URL** — in Discord: Server Settings → Integrations → Webhooks →
  New Webhook → Copy Webhook URL.

## Setup

### Option A: installer script

Linux/macOS:

```bash
chmod +x install.sh
./install.sh
```

Windows (PowerShell):

```powershell
.\install.ps1
```

Either script installs a C compiler, OpenSSL dev headers, ffmpeg, and Rust if any are
missing, creates `.env` from `.env.example`, and builds the release binary. On Linux it
also offers to set up a `systemd --user` service so it keeps running in the background
and starts on login. You still need to edit `.env` afterwards to add your `NIM_API_KEY`
and `DISCORD_WEBHOOK_URL`.

### Option B: manual

```bash
cp .env.example .env
# edit .env and fill in NIM_API_KEY and DISCORD_WEBHOOK_URL
cargo build --release
```

## Usage

Test everything end-to-end against a single image first, no webcam or camera
permissions required:

```bash
cargo run --release -- --image path/to/some_photo.jpg
```

Then run the continuous webcam monitor:

```bash
cargo run --release
```

Logging is via `tracing`; set `RUST_LOG=debug` for more detail.

## Configuration (environment variables / `.env`)

| Variable              | Required | Default                                                    | Notes |
|-----------------------|----------|-------------------------------------------------------------|-------|
| `NIM_API_KEY`         | yes      | —                                                             | From build.nvidia.com |
| `DISCORD_WEBHOOK_URL` | yes      | —                                                             | Discord webhook to post to |
| `NIM_API_URL`         | no       | `https://integrate.api.nvidia.com/v1/chat/completions`       | Point at a self-hosted NIM container instead if you run one |
| `NIM_MODEL`           | no       | `meta/llama-3.2-11b-vision-instruct`                          | Any NIM VLM that accepts `image_url` content |
| `POLL_INTERVAL_SECS`  | no       | `10`                                                          | How often to capture + check a frame |
| `COOLDOWN_SECS`       | no       | `300`                                                         | Minimum gap between two Discord notifications |
| `CAMERA_INPUT`        | no       | OS-specific (see `.env.example`)                              | The device string passed to ffmpeg |
| `NOTIFY_MEDIA`        | no       | `photo`                                                       | `photo` attaches the triggering frame; `video` records and attaches a short clip |
| `VIDEO_DURATION_SECS` | no       | `5`                                                            | Length of the clip when `NOTIFY_MEDIA=video` |
| `MAX_ATTACHMENT_MB`   | no       | `8`                                                            | Target size cap for video clips; raise to 50/100 if your Discord server is boosted |
| `ATTACH_MEDIA`        | no       | `true`                                                        | Set to `false` for a text-only notification with no attachment |

Note on video mode: the clip is recorded *after* detection fires, so it's a short
post-trigger clip (whatever the subject does in the next `VIDEO_DURATION_SECS`
seconds) rather than a pre-trigger buffer of what led up to it. The encoder targets a
bitrate computed from `MAX_ATTACHMENT_MB` and `VIDEO_DURATION_SECS`, and automatically
retries at a lower bitrate (up to a couple of times) if a clip still comes out over
that size — so a longer duration trades off against per-frame quality rather than
risking an oversized upload.

## Notes and caveats

- **Cost**: every poll is one NIM API call. A hosted VLM call isn't free forever —
  check current pricing/credits on build.nvidia.com, and pick a `POLL_INTERVAL_SECS`
  that matches your budget and how latency-sensitive the detection needs to be.
- **Accuracy**: this asks a general-purpose vision-language model a yes/no question,
  it isn't a dedicated, fine-tuned person-detector. It's generally solid for "is a
  person clearly in frame" but can miss partial occlusions or misfire on human-shaped
  objects (mannequins, posters, etc.). For stricter accuracy you could add a
  traditional CV pre-filter (e.g. motion detection) so the NIM call only fires on
  frames that already look interesting.
- **Privacy**: frames are sent to NVIDIA's API (or your own self-hosted NIM
  container, if you configure one) and, when detected, to Discord's CDN as an
  attachment. Make sure that's acceptable for wherever the camera is pointed.
