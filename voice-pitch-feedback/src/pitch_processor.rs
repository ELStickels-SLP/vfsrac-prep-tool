use std::collections::VecDeque;

use neo_audio::prelude::*;
use neo_audio::processors::player::Sender;
use realtime_tools::smooth_value::{Easing, Linear, SmoothValue};

use crate::pitch_shifter::{PitchShifter, PitchShiftResult};
use crate::UiMessage;

pub enum PitchMessage {
    Pitch(f32),
}

pub struct PitchProcessor {
    pitch_amount:f32,
    sample_rate: u32,
    // // SLA algorithm parameters
    analysis_length: usize,
    synthesis_length: usize,
    fft_length: usize,
    pitch_shifter: PitchShifter,

    
    // // Streaming buffers
    input_buffer: VecDeque<f32>,
    output_buffer: VecDeque<f32>,
    ui_sender: Sender<UiMessage>,
}

impl PitchProcessor {
    pub fn new(sample_rate: u32, analysis_length: usize, fft_length: usize, ui_sender: Sender<UiMessage>, target_pitch:f32, pitch_amount:f32) -> Self {
        let synthesis_length =
            (analysis_length as f32 * (target_pitch + pitch_amount) / target_pitch).round() as usize;
        

        Self {
            pitch_amount,
            analysis_length: 100,
            sample_rate,
            synthesis_length: 170,
            fft_length: 2048,
            pitch_shifter: PitchShifter::new(100, 170, 2048, sample_rate as usize),

            input_buffer: VecDeque::<f32>::new(),
            output_buffer: VecDeque::<f32>::new(),
            ui_sender,
        }
    }
}

impl AudioProcessor for PitchProcessor {
    type Message = PitchMessage;

    fn prepare(&mut self, config: DeviceConfig) {
        self.output_buffer.reserve(self.fft_length);
        self.input_buffer.reserve(self.fft_length);

        for _ in 0..(self.analysis_length) {
            self.output_buffer.push_back(0.0);
        }

        println!("PitchProcessor prepare called with {:?}", config);
    }

    fn message_process(&mut self, _message: PitchMessage) {}

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

        // TODO: Keep 
        while self.input_buffer.len() >= self.fft_length {
            self.process_window();
        }
    }
}

impl PitchProcessor {
    fn process_window(&mut self) {
        let samples = &self.input_buffer.make_contiguous()[..self.fft_length];
        // let pitch_amount_hz = self.pitch_amount.next_value();

        let PitchShiftResult { samples: out, peak_freq } = self.pitch_shifter.process(samples);

        // Send the pitch to the screen
        self.ui_sender.send(UiMessage::Level(peak_freq)).unwrap();
        self.ui_sender.send(UiMessage::WindowProcessed).unwrap();

        // Pop analysis_len out
        for s in out.iter() {
            self.output_buffer.push_back(*s);
            self.input_buffer.pop_front();
        }

        if (self.pitch_shifter.first_time) {
            println!("First window succeeded")
        }
        self.pitch_shifter.first_time = false;
    }
}
