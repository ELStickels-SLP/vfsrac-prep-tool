use eframe::egui::{lerp, remap};
use oxifft::rfft;
use oxifft::signal::resample;

pub(crate) struct PitchShiftResult {
    pub(crate) peak_freq: f32,
    pub(crate) samples: Vec<f32>,
}

/// Analyzes one window of `samples`, finds the fundamental frequency, and
/// Creates a new window of samples extended in time to shift the pitch by `pitch_amount_hz`.
pub(crate) fn shift_pitch_window(
    samples: &[f32],
    sample_rate: u32,
    analysis_win_length: usize,
    pitch_amount_hz: f32,
) -> PitchShiftResult {
    let hz_ratio = (sample_rate as f32) / analysis_win_length as f32;

    // Forward real FFT: N samples -> N/2+1 complex bins
    let spectrum: Vec<oxifft::Complex<f32>> = rfft(&samples[..analysis_win_length]);

    let mut peak_freq = peak_frequency(&spectrum, hz_ratio);
    // If we get an overtone instead of the fundamental, downshift it till we're in human range
    while peak_freq > 500. {
        peak_freq /= 2.
    }

    // TODO: Phase shifting to avoid artifacts

    let hop_ratio = (peak_freq + pitch_amount_hz) / peak_freq;
    let synth_len = (analysis_win_length as f32 * hop_ratio).round() as usize;

    let output_samples = resample(&samples[..analysis_win_length], synth_len);

    let mut output = Vec::<f32>::with_capacity(analysis_win_length);

    // Interpolate samples to squeeze back into output
    for i in 0..analysis_win_length {
        let t = i as f32 / analysis_win_length as f32;
        let sample_idx = remap(t, 0.0..=1.0, 0.0..=synth_len as f32);
        let frac = sample_idx % 1.0;
        let whole = sample_idx.floor() as usize;

        let a = output_samples[whole];
        let b = output_samples[(whole + 1).min(output_samples.len() - 1)];
        let sample = a * (1.0 - frac) + b * frac;

        output.push(sample);
    }

    PitchShiftResult {
        peak_freq,
        samples: output,
    }
}

// TODO: Dampen pitches larger than human speech
fn pitch_weight(a: &&oxifft::Complex<f32>, hz_ratio: f32) -> f32 {
    a.re * hz_ratio
}

/// Finds the dominant frequency in `spectrum` (skipping the DC bin at index 0).
fn peak_frequency(spectrum: &[oxifft::Complex<f32>], hz_ratio: f32) -> f32 {
    let peak_idx = spectrum
        .iter()
        .enumerate()
        .skip(1)
        .max_by(|(_, a), (_, b)| {
            pitch_weight(a, hz_ratio)
                .partial_cmp(&pitch_weight(b, hz_ratio))
                .unwrap()
        })
        .map(|(i, _)| i)
        .unwrap();

    peak_idx as f32 * hz_ratio
}

#[cfg(test)]
mod tests;
