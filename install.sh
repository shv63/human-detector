#!/usr/bin/env bash
#
# install.sh — sets up human-detector on Linux or macOS.
#
# What it does:
#   1. Installs a C compiler/linker if missing (needed to build openssl-sys).
#   2. Installs ffmpeg if it's not already on PATH (apt/dnf/pacman/brew).
#   3. Installs Rust via rustup if `cargo` isn't already on PATH.
#   4. Copies .env.example to .env (if .env doesn't exist yet) and reminds you to fill it in.
#   5. Builds the release binary.
#   6. On Linux, optionally installs a systemd --user service so it runs continuously
#      in the background and starts on login.
#
# Usage:
#   ./install.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

log()  { printf '\033[1;32m==>\033[0m %s\n' "$1"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$1"; }
die()  { printf '\033[1;31mError:\033[0m %s\n' "$1" >&2; exit 1; }

OS="$(uname -s)"

# ---------------------------------------------------------------------------
# 1. C toolchain (needed to build openssl-sys and friends)
# ---------------------------------------------------------------------------
if command -v cc >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1; then
    log "C compiler/linker already installed"
else
    log "No C compiler found (needed to build openssl-sys) — installing..."
    case "$OS" in
        Linux)
            if command -v apt-get >/dev/null 2>&1; then
                sudo apt-get update -y
                sudo apt-get install -y build-essential pkg-config
            elif command -v dnf >/dev/null 2>&1; then
                sudo dnf groupinstall -y "Development Tools"
                sudo dnf install -y pkgconf-pkg-config
            elif command -v pacman >/dev/null 2>&1; then
                sudo pacman -Sy --noconfirm base-devel pkgconf
            else
                die "Couldn't detect a supported package manager (apt/dnf/pacman). Install a C compiler (gcc/clang) manually, then re-run this script."
            fi
            ;;
        Darwin)
            log "Triggering Xcode Command Line Tools install (a system dialog may appear)..."
            xcode-select --install 2>/dev/null || true
            until command -v cc >/dev/null 2>&1; do
                warn "Waiting for Xcode Command Line Tools install to finish..."
                sleep 5
            done
            ;;
        *)
            die "Unsupported OS ($OS) for automatic C toolchain install. Install a C compiler manually, then re-run this script."
            ;;
    esac
fi

# ---------------------------------------------------------------------------
# 2. ffmpeg
# ---------------------------------------------------------------------------
if command -v ffmpeg >/dev/null 2>&1; then
    log "ffmpeg already installed ($(command -v ffmpeg))"
else
    log "ffmpeg not found — installing..."
    case "$OS" in
        Linux)
            if command -v apt-get >/dev/null 2>&1; then
                sudo apt-get update -y
                sudo apt-get install -y ffmpeg
            elif command -v dnf >/dev/null 2>&1; then
                sudo dnf install -y ffmpeg
            elif command -v pacman >/dev/null 2>&1; then
                sudo pacman -Sy --noconfirm ffmpeg
            else
                die "Couldn't detect a supported package manager (apt/dnf/pacman). Install ffmpeg manually, then re-run this script."
            fi
            ;;
        Darwin)
            if ! command -v brew >/dev/null 2>&1; then
                die "Homebrew not found. Install it from https://brew.sh, then re-run this script (or run 'brew install ffmpeg' yourself)."
            fi
            brew install ffmpeg
            ;;
        *)
            die "Unsupported OS ($OS) for automatic ffmpeg install. Install ffmpeg manually (https://ffmpeg.org/download.html), then re-run this script. On Windows, use install.ps1 instead."
            ;;
    esac
fi

# ---------------------------------------------------------------------------
# 3. Rust toolchain
# ---------------------------------------------------------------------------
if command -v cargo >/dev/null 2>&1; then
    log "Rust already installed ($(cargo --version))"
else
    log "Rust not found — installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
fi

# ---------------------------------------------------------------------------
# 4. .env
# ---------------------------------------------------------------------------
if [ -f .env ]; then
    log ".env already exists — leaving it as is"
else
    cp .env.example .env
    warn "Created .env from .env.example — edit it and fill in NIM_API_KEY and DISCORD_WEBHOOK_URL before running."
fi

# ---------------------------------------------------------------------------
# 5. Build
# ---------------------------------------------------------------------------
log "Building release binary (this can take a couple of minutes the first time)..."
cargo build --release

BIN_PATH="$SCRIPT_DIR/target/release/human-detector"
log "Built: $BIN_PATH"

# ---------------------------------------------------------------------------
# 6. Optional systemd --user service (Linux only)
# ---------------------------------------------------------------------------
if [ "$OS" = "Linux" ] && command -v systemctl >/dev/null 2>&1; then
    read -r -p "Install a systemd --user service so this runs continuously in the background? [y/N] " REPLY
    if [[ "$REPLY" =~ ^[Yy]$ ]]; then
        UNIT_DIR="$HOME/.config/systemd/user"
        mkdir -p "$UNIT_DIR"
        UNIT_FILE="$UNIT_DIR/human-detector.service"

        cat > "$UNIT_FILE" <<EOF
[Unit]
Description=human-detector — NIM-based webcam human detector with Discord alerts

[Service]
Type=simple
WorkingDirectory=$SCRIPT_DIR
EnvironmentFile=$SCRIPT_DIR/.env
ExecStart=$BIN_PATH
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

        systemctl --user daemon-reload
        systemctl --user enable --now human-detector.service
        log "Installed and started: systemctl --user status human-detector"
        log "Logs: journalctl --user -u human-detector -f"
    fi
fi

echo
log "Setup complete."
echo "  - Edit .env if you haven't already (NIM_API_KEY, DISCORD_WEBHOOK_URL)."
echo "  - Test against a single image:  $BIN_PATH --image path/to/photo.jpg"
echo "  - Run the live monitor:         $BIN_PATH"
