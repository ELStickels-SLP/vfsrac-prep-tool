#!/usr/bin/env bash
# Installs deps, sets the bundle version, and builds+preps the .app bundle.
# Usage: build-macos.sh <target-triple>
#
# Leaves the built .app under target/<target>/release/bundle/osx/ for a
# later signing/packaging step. Assumes the Rust toolchain is already
# installed.
set -euo pipefail

target="$1"

# Text shown in the macOS microphone-access (TCC) permission prompt.
# Required regardless of code signing, since the app captures live audio.
mic_usage_description="Used to analyze and pitch-shift your voice for biofeedback practice."

# Install deps
brew install portaudio dylibbundler
cargo install cargo-bundle --locked

# Set bundle version from tag
version="${GITHUB_REF_NAME#v}"
awk -v ver="$version" '
  /^\[workspace\.package\]/ { in_block=1; print; next }
  /^\[/ { in_block=0 }
  in_block && /^version = / && !done { print "version = \"" ver "\""; done=1; next }
  { print }
' Cargo.toml > Cargo.toml.tmp
mv Cargo.toml.tmp Cargo.toml

# Build .app bundle
(
  cd voice-pitch-feedback
  VFSRAC_VERSION="$GITHUB_REF_NAME" cargo bundle --release --target "$target"
)

app=$(echo "target/$target/release/bundle/osx"/*.app)

# Fix up dylib paths
mkdir -p "$app/Contents/Frameworks"
dylibbundler -od -b \
  -x "$app/Contents/MacOS/voice-pitch-feedback" \
  -d "$app/Contents/Frameworks" \
  -p "@executable_path/../Frameworks/"

# Add microphone usage description
/usr/libexec/PlistBuddy -c \
  "Add :NSMicrophoneUsageDescription string '$mic_usage_description'" \
  "$app/Contents/Info.plist"
