use std::collections::VecDeque;

use neo_audio::prelude::*;
use neo_audio::processors::player::Sender;
use realtime_tools::smooth_value::{Easing, Linear, SmoothValue};

use oxifft::{irfft, rfft};

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
    // input_write_idx: usize,
    // output_read_idx: usize,
    // output_write_idx: usize,
    ui_sender: Sender<UiMessage>,
    process_call_count: u32,
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
            ui_sender,
            process_call_count: 0,
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
    // TODO: Dampen pitches larger than human speech
    fn pitch_weight(a: &&oxifft::Complex<f32>, hz_ratio: f32) -> f32 {
        a.norm_sqr() * hz_ratio
    }
    fn process_window(&mut self) {
        // // tODO: real ring buffer
        let samples = self.input_buffer.make_contiguous();
        let hz_ratio = (self.sample_rate as f32) / self.analysis_win_length as f32;

        // Forward real FFT: N samples -> N/2+1 complex bins
        let mut spectrum: Vec<oxifft::Complex<f32>> = rfft(&samples[..self.analysis_win_length]);

        // Find the peak bin (skip DC at index 0)
        let peak_idx = spectrum
            .iter()
            .enumerate()
            .skip(1)
            .max_by(|(_, a), (_, b)| {
                Self::pitch_weight(a, hz_ratio)
                    .partial_cmp(&Self::pitch_weight(b, hz_ratio))
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap();

        let mut peak_freq = peak_idx as f32 * hz_ratio;
        // If we get an overtone instead of the fundamental, downshift it till we're in human range
        while peak_freq > 500. {
            peak_freq /= 2.
        }
        // Send the pitch to the screen
        self.ui_sender.send(UiMessage::Level(peak_freq)).unwrap();

        // Raise the pitch by stretching the spectrum: bin k in the output
        // takes its content from bin k / ratio in the input, moving energy
        // to higher frequencies for ratio > 1.
        let pitch_amount_hz = self.pitch_amount.next_value();
        let ratio = if peak_freq > 0.0 {
            (peak_freq + pitch_amount_hz) / peak_freq
        } else {
            1.0
        };
        let mut shifted_spectrum = vec![oxifft::Complex::new(0.0, 0.0); spectrum.len()];
        for (k, bin) in shifted_spectrum.iter_mut().enumerate() {
            let src_idx = (k as f32 / ratio).round() as usize;
            if src_idx < spectrum.len() {
                *bin = spectrum[src_idx];
            }
        }

        // Render back to the output buffer
        spectrum = shifted_spectrum;
        let shifted = irfft(&spectrum, self.analysis_win_length);

        for s in shifted {
            self.output_buffer.push_back(s);
            self.input_buffer.pop_front();
        }
    }
}
