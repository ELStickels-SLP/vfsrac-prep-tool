#!/usr/bin/env bash
# Builds and stages the Linux release binary.
# Usage: build-linux.sh <target-triple> <artifact-name>
set -euo pipefail

target="$1"
artifact="$2"

# Install deps
# PortAudio for neo-audio, GTK3/X11/xkbcommon for eframe.
sudo apt-get update
sudo apt-get install -y \
  portaudio19-dev \
  libgtk-3-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxkbcommon-dev \
  libssl-dev

# Build
VFSRAC_VERSION="$GITHUB_REF_NAME" cargo build --release --target "$target" -p voice-pitch-feedback

# Stage binary
mkdir -p dist
cp "target/$target/release/voice-pitch-feedback" "dist/$artifact"
