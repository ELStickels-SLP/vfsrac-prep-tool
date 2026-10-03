# vfsrac-prep-tool

Rust tool providing pitch-raised auditory biofeedback for pre-operative voice therapy.

## Workspace

`default-members` is set to `voice-pitch-feedback`, so `cargo run --release`
builds and runs just that app. Passing `voice-pitch-feedback` as a trailing
arg does nothing useful — `cargo run`'s positional args go to the program
itself, not to package selection.

The phase-vocoder `PitchShifter` (`crates/pitch-shift/src/phase_vocoder.rs`)
and the shared settings (window/FFT length options, default pitches,
`synthesis_length`) live in the `crates/pitch-shift` library, shared by:

- **`voice-pitch-feedback`** — the realtime GUI app.
- **`voice-pitch-offline`** — a CLI that pitch-shifts a WAV file on disk
  with the same buffering and latency as the app (`cargo run -p
  voice-pitch-offline -- <input.wav> <output.wav> [--pitch-hz <hz>]
  [--target-pitch <hz>] [--window <n>] [--fft-length <n>]`). Each app
  dropdown/slider has a matching flag, validated against the same option
  lists. It isn't in `default-members` since it isn't the app's `cargo
  run` target; build/run it with `-p voice-pitch-offline` explicitly.

The older `shift_pitch_window` in `crates/pitch-shift/src/lib.rs` is no
longer used by either app; only its unit tests exercise it.

`pitch-shift` has no GUI dependencies (it reimplements the one `egui::remap`
helper it needed) so it's safe for the CLI to depend on without pulling in
`eframe`.

## Architecture

See [architecture.md](architecture.md) for crate roles, the realtime data
flow, and known gotchas.

## Checks

On Windows, run `cargo clippy -p voice-pitch-feedback -p voice-pitch-offline
-p pitch-shift -p realtime-tools --all-targets` and `cargo test -p pitch-shift
-p realtime-tools`. `--workspace` fails there (`neo-audio` defaults to
`portaudio`). Keep clippy warning-free.

## Audio backend selection

`voice-pitch-feedback` selects its `neo-audio` backend per target platform
(see `voice-pitch-feedback/Cargo.toml`):

- **Windows**: `rtaudio-backend` (via the `rtaudio` crate)
- **Other platforms**: `portaudio-backend` (via the `portaudio` crate)

## Windows build details

The `rtaudio-sys` crate vendors RtAudio's C++ source and builds it
automatically during `cargo build` using CMake — no manual native build or
package manager (vcpkg/choco) install is required beyond CMake and the MSVC
toolchain on `PATH`.

The workspace root `Cargo.toml` pins `rtaudio-sys` out of the `opt-level = 2`
dev-profile override applied to all other dependencies. That override causes
the `cmake` crate to build RtAudio as `RelWithDebInfo` instead of `Debug`,
which omits the `d` debug-postfix that `rtaudio-sys`'s build script
hardcodes looking for on Windows (`rtaudiod.lib`), and the build fails to
link. Don't remove that override.

## Non-Windows prerequisites

Install PortAudio's development headers/libraries via your platform's
package manager (e.g. `apt install portaudio19-dev` on Debian/Ubuntu,
`brew install portaudio` on macOS) before building.

## Release workflow

[.github/workflows/release.yml](../.github/workflows/release.yml) triggers
on `v*.*.*` tags and builds Linux, Windows, and macOS artifacts. Each
platform's build (and macOS signing) logic lives in a standalone script
under [.github/scripts/](../.github/scripts/), not inline in the workflow
YAML:

- `build-linux.sh`, `build-windows.sh` — install deps, build, stage the
  binary into `dist/`.
- `build-macos.sh` — installs deps, sets the bundle version from the tag,
  builds the `.app` via `cargo bundle`, fixes up dylib paths, and adds the
  microphone usage description.
- `sign-macos.sh` — imports the Developer ID cert, code-signs, and
  notarizes the `.app`. No-op until the signing secrets exist (see
  [macos-code-signing-plan.md](macos-code-signing-plan.md)).

When changing build or signing behavior, edit the script, not the
workflow — the workflow should only pass `matrix.target`/`matrix.artifact`
and wire up secrets as env vars.

## Style guide

See [style-guide.md](style-guide.md) for code comment conventions and
interaction language. In short: keep inline comments minimal and in
Simplified Technical English, and put implementation-detail write-ups
(the "why", not the "what") in `.claude/` docs instead.
