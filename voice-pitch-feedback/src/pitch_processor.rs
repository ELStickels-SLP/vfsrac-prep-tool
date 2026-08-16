use std::collections::VecDeque;

use neo_audio::prelude::*;
use neo_audio::processors::player::Sender;
use realtime_tools::smooth_value::{Easing, Linear, SmoothValue};

use crate::pitch_shift::{shift_pitch_window, PitchShiftResult};
use crate::UiMessage;

pub enum PitchMessage {
    Pitch(f32),
}

pub struct PitchProcessor {
    pitch_amount: SmoothValue,
    sample_rate: u32,
    // // SLA algorithm parameters
    analysis_win_length: usize,
    // anls_hop_len: usize,
    // synth_hop_len: usize,
    // synth_win_len: usize,
    // win_f: Vec<f32>,

    // // Streaming buffers
    input_buffer: VecDeque<f32>,
    output_buffer: VecDeque<f32>,
    angle_buffer: Vec<f32>,
    ui_sender: Sender<UiMessage>,
}

impl PitchProcessor {
    pub fn new(sample_rate: u32, buffer_size: usize, ui_sender: Sender<UiMessage>) -> Self {
        let analysis_win_length = buffer_size;

        Self {
            pitch_amount: SmoothValue::new(50.0, Linear::ease_in_out),
            analysis_win_length,
            sample_rate,
            // anls_hop_len,
            // synth_hop_len,
            // synth_win_len,
            // win_f,
            input_buffer: VecDeque::<f32>::new(),
            output_buffer: VecDeque::<f32>::new(),
            angle_buffer: vec![0.0; buffer_size],
            ui_sender,
        }
    }
}

impl AudioProcessor for PitchProcessor {
    type Message = PitchMessage;

    fn prepare(&mut self, config: DeviceConfig) {
        self.output_buffer.reserve(self.analysis_win_length * 2);
        self.input_buffer.reserve(self.analysis_win_length * 2);

        for _ in 0..(self.analysis_win_length) {
            self.output_buffer.push_back(0.0);
        }

        println!("PitchProcessor prepare called with {:?}", config);
    }

    fn message_process(&mut self, message: PitchMessage) {
        match message {
            PitchMessage::Pitch(pitch) => self.pitch_amount.set_target_value(pitch),
        }
    }

    fn process(
        &mut self,
        mut output: InterleavedAudioMut<'_, f32>,
        input: InterleavedAudio<'_, f32>,
    ) {
        // self.process_call_count += 1;

        for (o, i) in output.channel_iter_mut(0).zip(input.channel_iter(0)) {
            self.input_buffer.push_back(*i);
            *o = self.output_buffer.pop_front().unwrap_or(0.0)
        }

        if self.input_buffer.len() >= self.analysis_win_length {
            self.process_window();
        }
    }
}

impl PitchProcessor {
    fn process_window(&mut self) {
        // // tODO: real ring buffer
        let samples = self.input_buffer.make_contiguous();
        let pitch_amount_hz = self.pitch_amount.next_value();

        let PitchShiftResult {
            peak_freq,
            samples: shifted,
        } = shift_pitch_window(
            &samples[..self.analysis_win_length],
            self.sample_rate,
            self.analysis_win_length,
            pitch_amount_hz,
            &mut self.angle_buffer[..self.analysis_win_length],
        );

        // Send the pitch to the screen
        self.ui_sender.send(UiMessage::Level(peak_freq)).unwrap();
        self.ui_sender.send(UiMessage::WindowProcessed).unwrap();

        for s in shifted {
            self.output_buffer.push_back(s);
            self.input_buffer.pop_front();
        }
    }
}
