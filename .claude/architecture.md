# Architecture

## Crates

| Crate | Role |
|---|---|
| `crates/neo-audio` | Audio I/O abstraction. `AudioProcessor` trait (`prepare`, `message_process`, `process`) plus backends in `src/backends/` (`rtaudio`, `portaudio`, `cpal`; `webaudio` is an empty stub). Backend is a cargo feature. |
| `crates/realtime-tools` | Real-time-safe helpers: `InterleavedAudio(Mut)` views, atomic `parameters`, `SmoothValue`, `level_meter`. |
| `crates/pitch-shift` | `shift_pitch_window` — FFT peak detection and resynthesis (uses `oxifft`, see [oxifft-notes.md](oxifft-notes.md)). No GUI deps. |
| `voice-pitch-feedback` | Realtime egui app (default member). |
| `voice-pitch-offline` | CLI: pitch-shift a WAV file. |

## Realtime app data flow

```
mic -> backend callback -> PitchProcessor::process
         input_buffer (VecDeque) -> PitchShifter::process (per fft_length window)
         output_buffer (VecDeque) -> speakers
                    |
                    +-- UiMessage (crossbeam bounded(1024)) --> egui update loop
egui --> Sender<PitchMessage> --> message_process (audio thread)
```

- `GainProcessor` and `PitchProcessor` are `AudioProcessor`s; the UI talks to
  them only through message senders.
- The audio thread must not block or allocate in steady state. Do
  allocation in `prepare`. (`PitchShifter::process` currently allocates
  `Vec`s per window, and `ui_sender.send(..).unwrap()` can panic if the UI
  queue closes. Both are known debts.)
- Settings are persisted via `PersistedSettings` in `main.rs`.
- The UI keeps `applied_*` copies of `analysis_win_length`, `pitch_amount`
  and `target_pitch`. Changing them rebuilds the processor.

## Known state / gotchas

- Phase matching (`spectrum_phase_match` in `pitch-shift`) is commented out
  on purpose ("workable build"). `_angle_buffer` and `_first_time` are
  unused until it returns.
- `voice-pitch-feedback/src/pitch_shifter.rs` is a separate copy of the
  algorithm, not the shared `pitch-shift` crate. The two can drift.
  Consolidating them is the intended direction.
- `PitchMessage::Pitch` is never sent and `PitchShifter::min_bins` is never
  read. They are placeholders.
- `cargo clippy --workspace` fails on Windows because `neo-audio` defaults
  to `portaudio`. Use
  `cargo clippy -p voice-pitch-feedback -p voice-pitch-offline -p pitch-shift -p realtime-tools --all-targets`.
