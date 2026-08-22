use eframe::egui::remap;
use oxifft::{Complex, irfft, rfft};

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
    angle_buffer: &mut [f32]
) -> PitchShiftResult {
    let hz_ratio = (sample_rate as f32) / analysis_win_length as f32;

    // Forward real FFT: N samples -> N/2+1 complex bins
    let mut spectrum: Vec<oxifft::Complex<f32>> = rfft(&samples[..analysis_win_length]);

    let mut peak_freq = peak_frequency(&spectrum, hz_ratio);
    // If we get an overtone instead of the fundamental, downshift it till we're in human range
    while peak_freq > 500. {
        peak_freq /= 2.
    }


    let hop_ratio = (peak_freq + pitch_amount_hz) / peak_freq;
    let synth_len = (analysis_win_length as f32 * hop_ratio).round() as usize;

    // early out if we don't find anything useful
    if peak_freq < 50. || hop_ratio < 1. || synth_len < 40  {
        return PitchShiftResult {
            peak_freq: -1.,
            samples: samples.to_vec()
        };
    }
    
    spectrum_phase_match(&mut spectrum, angle_buffer, hop_ratio, false);
    // TOODO: Lowpass filter to remove artifacts 

    let target_len = synth_len / 2 + 1;
    spectrum = interleave_with_zero_bins(&spectrum, target_len);

    let output_samples = irfft(&spectrum, synth_len);
    // let output_samples = resample(&samples[..analysis_win_length], synth_len);


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

fn spectrum_phase_match(spectrum:  &mut [Complex<f32>],  angles: &mut[f32], hop_ratio: f32, first_time:bool) {

    let len = spectrum.len();
    let twopi = 2.0 * std::f32::consts::PI;
    for (i, (s, prev_angle)) in spectrum.iter_mut().zip(angles.iter_mut()).enumerate() {
        let t = twopi * i as f32 / len as f32 ;

        let s_mag = s.norm();
        let s_angle = s.im.atan2(s.re);

        let s_unwrap = {
            let wrapped = (s_angle - *prev_angle) - t;
            let unwrapped = wrapped - (wrapped / twopi).round() * twopi;
            (unwrapped + t) * hop_ratio
        };
        let final_angle = {
            if first_time {
                s_angle
            } else {
                s_angle + s_unwrap
            }
        };
        s.re = final_angle.cos() * s_mag;
        s.im = final_angle.sin() * s_mag;

        *prev_angle = final_angle;
    }
}

/// Grows `spectrum` to `target_len` bins by spreading zero bins evenly
/// between the original ones, instead of appending them all after the
/// highest original bin.
fn interleave_with_zero_bins(spectrum: &[Complex<f32>], target_len: usize) -> Vec<Complex<f32>> {
    let orig_len = spectrum.len();
    let mut result = Vec::with_capacity(target_len);

    let n_zero = target_len - orig_len;
    let zero_step = n_zero as f32 / orig_len as f32;
    let mut zero_acc = 0.0;

    for bin in spectrum {
        result.push(*bin);
        zero_acc += zero_step;
        while zero_acc >= 1.0 && result.len() < target_len {
            result.push(Complex::zero());
            zero_acc -= 1.0;
        }
    }
    while result.len() < target_len {
        result.push(Complex::zero());
    }

    result
}

/// A peak bin magnitude below this fraction of full scale is treated as
/// noise floor rather than a real tone. `rfft` is unnormalized, so the
/// magnitude is divided by `spectrum.len() - 1` (~ half the window
/// length) first to get a window-length-independent amplitude estimate.
const MIN_PEAK_AMPLITUDE: f32 = 0.01;

/// Finds the dominant frequency in `spectrum` (skipping the DC bin at index 0).
/// Returns -1 if the spectrum doesn't have enough energy for the peak to be
/// meaningful (e.g. silence or noise floor).
fn peak_frequency(spectrum: &[oxifft::Complex<f32>], hz_ratio: f32) -> f32 {
    let (peak_idx, peak_bin) = spectrum
        .iter()
        .enumerate()
        .skip(1)
        .max_by(|(_, a), (_, b)| {
            (&a.re).total_cmp(&b.re)
        })
        .unwrap();

    let amplitude = peak_bin.norm() / (spectrum.len() - 1) as f32;
    if amplitude < MIN_PEAK_AMPLITUDE {
        return -1.;
    }

    peak_idx as f32 * hz_ratio
}

#[cfg(test)]
mod tests;
