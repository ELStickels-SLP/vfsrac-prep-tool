# vfsrac-prep-tool
Rust tool to provide pitch raised auditory biofeedback as part of pre-operative voice therapy

to run, use 
`cargo run --release`

(the workspace's `default-members` is set to `voice-pitch-feedback`, so this
builds and runs just the app; passing `voice-pitch-feedback` as a trailing
argument, as in an earlier version of this doc, does nothing useful — `cargo
run`'s positional args are passed to the program itself, not used to select
a package)

## Building

`voice-pitch-feedback` selects its `neo-audio` backend per target platform (see
`voice-pitch-feedback/Cargo.toml`):

- **Windows**: `rtaudio-backend` (via the `rtaudio` crate)
- **Other platforms**: `portaudio-backend` (via the `portaudio` crate)

### Windows prerequisites

The `rtaudio-sys` crate vendors RtAudio's C++ source and builds it automatically
during `cargo build` using CMake — no manual native build or package manager
(vcpkg/choco) install is required. You just need:

- [CMake](https://cmake.org/download/) on `PATH`
- The MSVC toolchain (Visual Studio Build Tools with the "Desktop development
  with C++" workload), matching the `x86_64-pc-windows-msvc` Rust target

With those installed, `cargo build` / `cargo run` will compile RtAudio and
link it automatically.

Note: the workspace root `Cargo.toml` pins `rtaudio-sys` out of the
`opt-level = 2` dev-profile override applied to all other dependencies. That
override causes the `cmake` crate to build RtAudio as `RelWithDebInfo` instead
of `Debug`, which omits the `d` debug-postfix that `rtaudio-sys`'s build
script hardcodes looking for on Windows (`rtaudiod.lib`), and the build fails
to link. Don't remove that override.

### Non-Windows prerequisites

Install PortAudio's development headers/libraries via your platform's package
manager (e.g. `apt install portaudio19-dev` on Debian/Ubuntu,
`brew install portaudio` on macOS) before building.
