# vfsrac-prep-tool

Rust tool providing pitch-raised auditory biofeedback for pre-operative voice therapy.

## Workspace

`default-members` is set to `voice-pitch-feedback`, so `cargo run --release`
builds and runs just that app. Passing `voice-pitch-feedback` as a trailing
arg does nothing useful — `cargo run`'s positional args go to the program
itself, not to package selection.

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
