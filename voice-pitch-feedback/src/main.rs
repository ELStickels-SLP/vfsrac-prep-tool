#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::usize;

use eframe::egui;
use level_meter::level_meter;
use neo_audio::{
    prelude::*,
    processors::player::{bounded, Receiver, Sender},
};
use realtime_tools::smooth_value::{Easing, Linear, SmoothValue};

#[cfg(windows)]
use neo_audio::backends::rtaudio_backend::RtAudioBackend as AudioBackendImpl;
#[cfg(not(windows))]
use neo_audio::backends::portaudio_backend::PortAudioBackend as AudioBackendImpl;

mod level_meter;
mod pitch_processor;

use pitch_processor::PitchProcessor;

static ANALYSIS_WIN_LENGTH: usize = 1500;

// Set by git tags or "local" if not set
static VERSION: &str = match option_env!("VFSRAC_VERSION") {
    Some(v) => v,
    None => "local",
};

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "neo-audio egui example",
        native_options,
        Box::new(|cc| Ok(Box::new(NeoAudioEguiExample::new(cc)))),
    )
    .unwrap();
}

struct NeoAudioEguiExample {
    neo_audio: NeoAudio<AudioBackendImpl>,
    sender: Option<Sender<pitch_processor::PitchMessage>>,
    audio_running: bool,
    config: DeviceConfig,
    ui_sender: Sender<UiMessage>,
    ui_receiver: Receiver<UiMessage>,
    input_level: SmoothValue,
    windows_processed: u32,
    pitch_amount: f32,
}

impl NeoAudioEguiExample {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        let neo_audio = NeoAudio::<AudioBackendImpl>::new().unwrap();
        let backend = neo_audio.backend();
        let (ui_sender, ui_receiver) = bounded(1024);
        let mut input_level = SmoothValue::new(-60.0, Linear::ease_in_out);
        input_level.prepare(60, 100);
        Self {
            audio_running: false,
            sender: None,
            config: backend.config(),
            neo_audio,
            ui_sender,
            ui_receiver,
            input_level,
            windows_processed: 0,
            pitch_amount: 70.0,
        }
    }
}

impl eframe::App for NeoAudioEguiExample {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(2.0);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("neo-audio egui example!");
            ui.label(format!("Version: {VERSION}"));

            let backend = self.neo_audio.backend();

            // API
            egui::ComboBox::from_label("Api")
                .selected_text(backend.api())
                .show_ui(ui, |ui| {
                    for api in backend.available_apis() {
                        ui.selectable_value(&mut self.config.api, api.clone(), api);
                    }
                });

            // Output Device
            egui::ComboBox::from_label("Output Device")
                .selected_text(format!(
                    "{:?}",
                    backend.output_device().unwrap_or("None".to_string())
                ))
                .show_ui(ui, |ui| {
                    for device in backend.available_output_devices() {
                        ui.selectable_value(
                            &mut self.config.output_device,
                            Device::Name(device.clone()),
                            device,
                        );
                    }
                });

            // Input Device
            egui::ComboBox::from_label("Input Device")
                .selected_text(format!(
                    "{:?}",
                    backend.input_device().unwrap_or("None".to_string())
                ))
                .show_ui(ui, |ui| {
                    for device in backend.available_input_devices() {
                        ui.selectable_value(
                            &mut self.config.input_device,
                            Device::Name(device.clone()),
                            device,
                        );
                    }
                });

            // Sample Rate
            egui::ComboBox::from_label("Sample Rate")
                .selected_text(format!("{}", backend.sample_rate()))
                .show_ui(ui, |ui| {
                    for sr in backend.available_sample_rates() {
                        ui.selectable_value(&mut self.config.sample_rate, sr, sr.to_string());
                    }
                });

            // Num Frames
            egui::ComboBox::from_label("Num Frames")
                .selected_text(format!("{}", backend.num_frames()))
                .show_ui(ui, |ui| {
                    for frames in backend.available_num_frames().iter() {
                        ui.selectable_value(
                            &mut self.config.num_frames,
                            *frames,
                            frames.to_string(),
                        );
                    }
                });

            if self.config != backend.config() {
                if self.audio_running {
                    self.neo_audio.stop_audio().unwrap();
                    self.audio_running = false;
                }
                // update config and receive actually applied config
                self.config = self
                    .neo_audio
                    .backend_mut()
                    .set_config(&self.config)
                    .unwrap();
            }

            #[allow(clippy::collapsible_else_if)]
            if self.audio_running {
                if ui.button("Stop").clicked() {
                    self.neo_audio.stop_audio().unwrap();
                    self.audio_running = false;
                    self.windows_processed = 0;
                }
            } else {
                if ui.button("Start").clicked() {
                    self.sender = Some(
                        self.neo_audio
                            .start_audio(PitchProcessor::new(
                                self.config.sample_rate,
                                ANALYSIS_WIN_LENGTH,
                                self.ui_sender.clone(),
                            ))
                            .unwrap(),
                    );
                    self.audio_running = true;
                    self.windows_processed = 0;
                    if let Some(sender) = &self.sender {
                        sender
                            .send(pitch_processor::PitchMessage::Pitch(self.pitch_amount))
                            .unwrap();
                    }
                }
            }

            let pitch_slider = ui.add(
                egui::Slider::new(&mut self.pitch_amount, 0.0..=100.0)
                    .text("Pitch Amount (Hz)"),
            );
            if pitch_slider.changed() {
                if let Some(sender) = &self.sender {
                    sender
                        .send(pitch_processor::PitchMessage::Pitch(self.pitch_amount))
                        .unwrap();
                }
            }

            ui.label("Pitch shift: 170Hz → 250Hz (1.47x)");
            ui.label("Processing audio with SLA pitch shifter...");
            ui.label(format!("Windows processed: {}", self.windows_processed));

            // update percentage and debug info
            if self.audio_running {
                for _ in 0..self.ui_receiver.len() {
                    match self.ui_receiver.try_recv() {
                        Ok(message) => match message {
                            UiMessage::Level(level) => {
                                self.input_level.set_target_value(level);
                            }
                            UiMessage::WindowProcessed => break,
                        },
                        _ => break,
                    }
                }
                ui.ctx().request_repaint();
            } else {
                self.input_level.set_current_and_target_value(-60.0);
            }

            let cur_level = self.input_level.next_value();
            ui.add(level_meter(0.0..=300.0, cur_level));
            ui.label(format!("Level: {}hz", cur_level));
        });
    }
}

enum UiMessage {
    Level(f32),
    WindowProcessed,
}
