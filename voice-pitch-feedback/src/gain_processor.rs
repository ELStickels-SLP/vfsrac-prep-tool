use neo_audio::prelude::*;
use neo_audio::processors::player::Sender;
use realtime_tools::{
    level_meter::{Level, LevelMeter},
    smooth_value::{Linear, SmoothValue},
};

use crate::UiMessage;

pub enum GainMessage {
    Gain(f32),
}

pub struct GainProcessor {
    gain: SmoothValue,
    meter: LevelMeter,
}

impl GainProcessor {
    pub fn new(ui_sender: Sender<UiMessage>) -> Self {
        Self {
            gain: SmoothValue::new(1.0, Linear::ease_in_out),
            meter: LevelMeter::new(Box::new(move |level: Level| {
                ui_sender.send(UiMessage::Level(level)).unwrap();
            })),
        }
    }
}

impl AudioProcessor for GainProcessor {
    type Message = GainMessage;

    fn prepare(&mut self, config: DeviceConfig) {
        self.gain.prepare(config.sample_rate, 100);
        self.meter
            .prepare(config.sample_rate, config.num_frames, 100);
        println!("Prepare is called with {:?}", config);
    }

    fn message_process(&mut self, message: GainMessage) {
        match message {
            GainMessage::Gain(gain) => self.gain.set_target_value(gain),
        }
    }

    fn process(
        &mut self,
        mut output: InterleavedAudioMut<'_, f32>,
        input: InterleavedAudio<'_, f32>,
    ) {
        if input.num_channels() > 0 {
            self.meter.process(input.channel_iter(0));
        }
        for (out_frame, in_frame) in output.frames_iter_mut().zip(input.frames_iter()) {
            let gain = self.gain.next_value();
            for (o, i) in out_frame.iter_mut().zip(in_frame.iter()) {
                *o = *i * gain;
            }
        }
    }
}
