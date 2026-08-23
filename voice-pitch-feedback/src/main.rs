#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::usize;

use eframe::egui::{self, Response};
use egui_plot::{FilledArea, HLine, Line, Plot, PlotBounds};
use level_meter::level_meter;
use neo_audio::{
    prelude::*,
    processors::player::{bounded, Receiver, Sender},
};
use realtime_tools::smooth_value::{Easing, Linear, SmoothValue};

#[cfg(not(windows))]
use neo_audio::backends::portaudio_backend::PortAudioBackend as AudioBackendImpl;
#[cfg(windows)]
use neo_audio::backends::rtaudio_backend::RtAudioBackend as AudioBackendImpl;

mod level_meter;
mod pitch_processor;

use pitch_processor::PitchProcessor;

static ANALYSIS_WIN_LENGTH_OPTIONS: [usize; 6] = [250, 500, 1000, 1500, 2000, 3000];
static DEFAULT_ANALYSIS_WIN_LENGTH: usize = 1500;
static PITCHLINE_SAMPLES: usize = 1500;
const PITCH_HISTOGRAM_INTERVAL: f64 = 1.0 / 30.0;

// Set by git tags or "local" if not set
static VERSION: &str = match option_env!("VFSRAC_VERSION") {
    Some(v) => v,
    None => "local",
};

// Middle value rather than the largest sample rate: the largest tends to be
// a backend-reported edge value that underperforms on real hardware.
fn middle(values: &[u32]) -> Option<u32> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted.get(sorted.len() / 2).copied()
}



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
    analysis_win_length: usize,
    applied_analysis_win_length: usize,
    ui_sender: Sender<UiMessage>,
    ui_receiver: Receiver<UiMessage>,
    pitch_level: SmoothValue,
    windows_processed: u32,
    pitch_amount: f32,
    pitch_histogram: Vec<f32>,
    pitch_histogram_pos: usize,
    last_histogram_update: f64,
}

impl NeoAudioEguiExample {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        let mut neo_audio = NeoAudio::<AudioBackendImpl>::new().unwrap();
        let (ui_sender, ui_receiver) = bounded(1024);
        let mut pitch_level = SmoothValue::new(-60.0, Linear::ease_in_out);
        pitch_level.prepare(60, 100);

        let backend = neo_audio.backend();
        let mut config = backend.config();
        if let Some(sample_rate) = middle(&backend.available_sample_rates()) {
            config.sample_rate = sample_rate;
        }
        if let Some(num_frames) = middle(&backend.available_num_frames()) {
            config.num_frames = num_frames;
        }
        let config = neo_audio.backend_mut().set_config(&config).unwrap();

        Self {
            audio_running: false,
            sender: None,
            config,
            analysis_win_length: DEFAULT_ANALYSIS_WIN_LENGTH,
            applied_analysis_win_length: DEFAULT_ANALYSIS_WIN_LENGTH,
            neo_audio,
            ui_sender,
            ui_receiver,
            pitch_level,
            windows_processed: 0,
            pitch_amount: 70.0,
            pitch_histogram: vec![-1.; 600],
            pitch_histogram_pos: 0,
            last_histogram_update: 0.0,
        }
    }
    fn show_plot(&self, ui: &mut egui::Ui, current_pitch: f32) -> Response {
        let last_x = (self.pitch_histogram.len().saturating_sub(1)) as f64;
        let filled_area = FilledArea::new(
            "human pitch range",
            &[0.0, last_x],
            &[100.0, 100.0],
            &[200.0, 200.0],
        )
        .fill_color(egui::Color32::from_rgba_unmultiplied(100, 200, 100, 50));

        // -1 marks samples that haven't been written yet; break the line there.
        let mut segments: Vec<Vec<[f64; 2]>> = Vec::new();
        let mut segment: Vec<[f64; 2]> = Vec::new();
        for (i, &level) in self.pitch_histogram.iter().enumerate() {
            if level < 0.0 {
                if segment.len() > 1 {
                    segments.push(std::mem::take(&mut segment));
                } else {
                    segment.clear();
                }
            } else {
                segment.push([i as f64, level as f64]);
            }
        }
        if segment.len() > 1 {
            segments.push(segment);
        }

        Plot::new("Pitch range")
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max([0.0, 0.0], [600.0, 300.0]));
                plot_ui.add(filled_area);
                plot_ui.hline(HLine::new("current", current_pitch));
                for points in segments {
                    plot_ui.line(Line::new("pitch", points));
                }
            })
            .response
    }
}

impl eframe::App for NeoAudioEguiExample {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().set_pixels_per_point(2.0);
        egui::CentralPanel::default().show(ui, |ui| {
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

            ui.horizontal(|ui| {
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

                // Analysis Window Length
                egui::ComboBox::from_label("Analysis Window Length")
                    .selected_text(self.analysis_win_length.to_string())
                    .show_ui(ui, |ui| {
                        for win_length in ANALYSIS_WIN_LENGTH_OPTIONS {
                            ui.selectable_value(
                                &mut self.analysis_win_length,
                                win_length,
                                win_length.to_string(),
                            );
                        }
                    });
            });

            let pitch_slider = ui.add(
                egui::Slider::new(&mut self.pitch_amount, 0.0..=100.0).text("Pitch Amount (Hz)"),
            );
            if pitch_slider.changed() {
                if let Some(sender) = &self.sender {
                    sender
                        .send(pitch_processor::PitchMessage::Pitch(self.pitch_amount))
                        .unwrap();
                }
            }

            if self.config != backend.config()
                || self.analysis_win_length != self.applied_analysis_win_length
            {
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
                self.applied_analysis_win_length = self.analysis_win_length;
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
                                self.analysis_win_length,
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

            // update percentage and debug info
            if self.audio_running {
                for _ in 0..self.ui_receiver.len() {
                    match self.ui_receiver.try_recv() {
                        Ok(message) => match message {
                            UiMessage::Level(level) => {
                                if level.signum() != self.pitch_level.cur_level().signum() {
                                    self.pitch_level.set_current_and_target_value(level);
                                } else {
                                    self.pitch_level.set_target_value(level);
                                }
                            }
                            UiMessage::WindowProcessed => {
                                self.windows_processed += 1;
                            }
                        },
                        _ => break,
                    }
                }
                ui.ctx().request_repaint();
            } else {
                self.pitch_level.set_current_and_target_value(-60.0);
            }

            let cur_level = self.pitch_level.next_value();

            let now = ui.ctx().time();
            if self.audio_running && now - self.last_histogram_update >= PITCH_HISTOGRAM_INTERVAL {
                self.last_histogram_update = now;
                let pos = self.pitch_histogram_pos;
                self.pitch_histogram[pos] = cur_level;
                self.pitch_histogram_pos = (pos + 1) % self.pitch_histogram.len();
            }
            ui.horizontal(|ui| {
                ui.add(level_meter(0.0..=300.0, cur_level));
                ui.label(format!("Level: {}hz", cur_level));
            });

            self.show_plot(ui, cur_level);
        });
    }
}

enum UiMessage {
    Level(f32),
    WindowProcessed,
}
