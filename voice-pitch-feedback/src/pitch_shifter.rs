use oxifft::{Complex, rfft, irfft};
use std::f32::consts::PI;

/// Manages state for phase vocoder pitch shifting
pub struct PitchShifter {
    pub n_anal: usize,
    pub n_synth: usize,
    pub n_fft: usize,
    pub unwrapdata: Vec<f32>,
    pub phi_prev: Vec<f32>,
    pub phi_syn: Vec<f32>,
    pub prev_synth: Vec<f32>,
    pub first_time: bool,
    pub sample_rate: usize,
    pub min_bins:usize,
}

impl PitchShifter {
    pub fn new(n_anal: usize, n_synth: usize, n_fft: usize, sample_rate:usize) -> Self {
        let bins = n_fft / 2 + 1;
        // unwrapdata = 2*pi*k*n_anal / n_fft (expected phase advance over one analysis hop)
        let mut unwrapdata = vec![0.0; bins];
        for k in 0..bins {
            unwrapdata[k] = 2.0 * PI * k as f32 * n_anal as f32 / n_fft as f32;
        }
        PitchShifter {
            min_bins: bins,
            n_anal,
            n_synth,
            n_fft,
            unwrapdata,
            phi_prev: vec![0.0; bins],
            phi_syn: vec![0.0; bins],
            prev_synth: vec![0.0; n_fft],
            first_time: true,
            sample_rate
        }
    }

    // in: fftlen + analysislen
    pub fn process(&mut self, s: &[f32]) -> PitchShiftResult {
        // FFT (rfft)
        let sf = rfft(s);
        let bins = sf.len();
        let peak_freq = peak_frequency(&sf, self.sample_rate as f32 / self.n_fft as f32);

        // Phase
        let mut phi = vec![0.0; bins];
        for i in 0..bins {
            phi[i] = sf[i].im.atan2(sf[i].re);
        }

        // Phase unwrapping and accumulation
        let mut phi_unwrap = vec![0.0; bins];
        for i in 0..bins {
            let mut dphi = phi[i] - self.phi_prev[i] - self.unwrapdata[i];
            // Wrap to [-pi, pi]
            dphi = dphi - (dphi / (2.0 * PI)).round() * 2.0 * PI; 
            phi_unwrap[i] = (dphi + self.unwrapdata[i]) * (self.n_synth as f32 / self.n_anal as f32);
        }

        if self.first_time {
            self.phi_syn.clone_from_slice(&phi);
            self.prev_synth.fill(0.0);
            self.first_time = false;
        } else {
            for i in 0..bins {
                self.phi_syn[i] += phi_unwrap[i];
            }
        }

        // Build synthesis spectrum with wrapped phase and original magnitude
        let mut ibuf = vec![Complex::<f32>::zero(); bins];
        for i in 0..bins {
            let mag = sf[i].norm();
            let phase = self.phi_syn[i];
            ibuf[i] = Complex::from_polar(mag, phase);
        }

        // IFFT
        let synth = irfft(&ibuf, self.n_fft);

        // Overlap-add
        // Tail from previous synthesis window
        let prev_tail = &self.prev_synth[self.n_anal..];
        let out_len = self.n_synth;
        let mut obuf = vec![0.0f32; out_len];
        for i in 0..prev_tail.len().min(out_len) {
            obuf[i] = prev_tail[i];
        }
        // Add head of current synth
        for i in 0..obuf.len() {
            if i < synth.len() {
                obuf[i] += synth[i];
            }
        }
        // Next prev_synth for next frame is current synth
        self.prev_synth.clone_from_slice(&synth[..self.n_fft]);

        // Resample obuf of length n_synth back to n_anal via linear interpolation
        let mut out = vec![0.0; self.n_anal];
        let hop = self.n_synth as f32 / self.n_anal as f32;
        for i in 0..self.n_anal {
            let idx = i as f32 * hop;
            let idx_floor = idx.floor() as usize;
            let frac = idx - idx.floor();
            let a = obuf.get(idx_floor).copied().unwrap_or(0.0);
            let b = obuf.get(idx_floor + 1).copied().unwrap_or(0.0);
            out[i] = a * (1.0 - frac) + b * frac;
        }
        // Save phi for next iteration
        self.phi_prev.clone_from_slice(&phi);

        PitchShiftResult {
            samples: out,
            peak_freq,
        }
    }
}

pub struct PitchShiftResult {
    pub samples: Vec<f32>,
    pub peak_freq: f32,
}


/// A peak bin magnitude below this fraction of full scale is treated as
/// noise floor rather than a real tone. `rfft` is unnormalized, so the
/// magnitude is divided by `spectrum.len() - 1` (~ half the window
/// length) first to get a window-length-independent amplitude estimate.
const MIN_PEAK_AMPLITUDE: f32 = 0.001;

/// Finds the dominant frequency in `spectrum` (skipping the DC bin at index 0).
/// Returns -1 if the spectrum doesn't have enough energy for the peak to be
/// meaningful (e.g. silence or noise floor).
fn peak_frequency(spectrum: &[oxifft::Complex<f32>], hz_ratio: f32) -> f32 {
    let (peak_idx, peak_bin) = spectrum
        .iter()
        .enumerate()
        .skip(1)
        .max_by(|(_, a), (_, b)| a.norm().total_cmp(&b.norm()))
        .unwrap();

    let amplitude = peak_bin.norm() / (spectrum.len() - 1) as f32;
    if amplitude < MIN_PEAK_AMPLITUDE {
        return -1.;
    }
    

    let peak_freq = peak_idx as f32 * hz_ratio;
    if peak_freq > 300. {
        peak_freq / 2.
    } else {
        peak_freq
    }
}