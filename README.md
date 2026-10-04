# vfsrac-prep-tool
Rust tool to provide pitch raised auditory biofeedback as part of pre-operative voice therapy


# Participant Instructions

Head over to the (Releases)[https://github.com/ELStickels-SLP/vfsrac-prep-tool/releases] and click the download that matches your operating system.  

Most of the settings can be left alone, but if something doesn't sound right you can open the "Advanced settings". 

### Windows: "Windows protected your PC" warning

The released `.exe` isn't code-signed yet, so Windows SmartScreen will show
an "unrecognized publisher" warning when you run it. This is expected for
now — click **More info**, then **Run anyway** to launch it. 


# Developer information 


### Offline CLI

`voice-pitch-offline` pitch-shifts a WAV file on disk instead of running
live through an audio device. It's not the default `cargo run` target, so
build/run it with `-p`:

```
cargo run --release -p voice-pitch-offline -- <input.wav> <output.wav> --pitch-hz <hz>
```

Run `cargo run -p voice-pitch-offline -- --help` for the full option list
(including `--window` to change the analysis window length).

### Windows prerequisites

- [CMake](https://cmake.org/download/) on `PATH`
- MSVC toolchain (Visual Studio Build Tools, "Desktop development with C++"),
  matching the `x86_64-pc-windows-msvc` Rust target

### Non-Windows prerequisites

Install PortAudio's development headers/libraries via your platform's package
manager (e.g. `apt install portaudio19-dev` on Debian/Ubuntu,
`brew install portaudio` on macOS) before building.

