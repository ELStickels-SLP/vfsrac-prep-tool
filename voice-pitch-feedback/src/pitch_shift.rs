use oxifft::{irfft, rfft};

pub(crate) struct PitchShiftResult {
    pub(crate) peak_freq: f32,
    pub(crate) samples: Vec<f32>,
}

/// Analyzes one window of `samples`, finds the fundamental frequency, and
/// rebuilds the window with the spectrum stretched so the fundamental
/// moves up (or down) by `pitch_amount_hz`.
pub(crate) fn shift_pitch_window(
    samples: &[f32],
    sample_rate: u32,
    analysis_win_length: usize,
    pitch_amount_hz: f32,
) -> PitchShiftResult {
    let hz_ratio = (sample_rate as f32) / analysis_win_length as f32;

    // Forward real FFT: N samples -> N/2+1 complex bins
    let mut spectrum: Vec<oxifft::Complex<f32>> = rfft(&samples[..analysis_win_length]);

    let mut peak_freq = peak_frequency(&spectrum, hz_ratio);
    // If we get an overtone instead of the fundamental, downshift it till we're in human range
    while peak_freq > 500. {
        peak_freq /= 2.
    }

    // Raise the pitch by stretching the spectrum: bin k in the output
    // takes its content from bin k / ratio in the input, moving energy
    // to higher frequencies for ratio > 1.
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
    let samples = irfft(&spectrum, analysis_win_length);

    PitchShiftResult { peak_freq, samples }
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
