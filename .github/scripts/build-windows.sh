#!/usr/bin/env bash
# Builds and stages the Windows release binary.
# Usage: build-windows.sh <target-triple> <artifact-name>
#
# Assumes the MSVC dev environment (cl.exe/nmake.exe on PATH) has already
# been set up, e.g. via ilammy/msvc-dev-cmd in the calling workflow.
set -euo pipefail

target="$1"
artifact="$2"

# cmake-rs auto-detects a "Visual Studio NN" generator via its own registry
# scan and panics on VS versions it doesn't recognize yet (e.g. VS 2026
# "18.0" alongside 2022 on windows-latest runners). NMake Makefiles just
# needs cl.exe/nmake.exe on PATH and skips that lookup entirely.
export CMAKE_GENERATOR="NMake Makefiles"

# With an explicit generator, cmake-rs skips the flag-forcing it normally
# does for the Visual Studio generator, so RtAudio's own CMakeLists.txt
# rewrites CMake's default Debug flags ("/MDd") to "/MTd" (static debug
# CRT). That mismatches Rust's dynamic release CRT and fails to link
# (unresolved _CrtDbgReport, _malloc_dbg, etc). This toolchain file forces
# "/MD" instead. See .github/cmake/windows-crt.cmake for details.
export CMAKE_TOOLCHAIN_FILE="$GITHUB_WORKSPACE/.github/cmake/windows-crt.cmake"

# Statically link the CRT so the shipped .exe has no dependency on the
# VC++ Redistributable (MSVCP140.dll/VCRUNTIME140.dll) being installed on
# the target machine. Must match the static release CRT that
# windows-crt.cmake forces RtAudio to use above.
export RUSTFLAGS="-C target-feature=+crt-static"

export VFSRAC_VERSION="$GITHUB_REF_NAME"

# Git for Windows ships its own link.exe (a symlink shim) in usr/bin. On
# GitHub's windows-latest runner, running this script via `shell: bash`
# puts that dir ahead of the MSVC link.exe that ilammy/msvc-dev-cmd added
# to PATH, so rustc picks up Git's link.exe instead and fails to link.
export PATH="$(echo "$PATH" | sed -e 's|:/c/Program Files/Git/usr/bin||' -e 's|/c/Program Files/Git/usr/bin:||')"

# Build
cargo build --release --target "$target" -p voice-pitch-feedback

# Stage binary
mkdir -p dist
cp "target/$target/release/voice-pitch-feedback.exe" "dist/$artifact"
