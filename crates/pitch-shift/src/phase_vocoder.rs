use oxifft::{irfft, rfft, Complex};
use std::f32::consts::PI;

use crate::PitchShiftResult;

/// Manages state for phase vocoder pitch shifting
pub struct PitchShifter {
    pub n_anal: usize,
    pub n_synth: usize,
    pub n_fft: usize,
    pub unwrapdata: Vec<f32>,
    pub phi_prev: Vec<f32>,
    pub phi_syn: Vec<f32>,
    window: Vec<f32>,
    ola: Vec<f32>,
    ola_norm: Vec<f32>,
    pub first_time: bool,
    pub sample_rate: usize,
}

impl PitchShifter {
    pub fn new(n_anal: usize, n_synth: usize, n_fft: usize, sample_rate: usize) -> Self {
        let bins = n_fft / 2 + 1;
        // unwrapdata = 2*pi*k*n_anal / n_fft (expected phase advance over one analysis hop)
        let mut unwrapdata = vec![0.0; bins];
        for (k, u) in unwrapdata.iter_mut().enumerate() {
            *u = 2.0 * PI * k as f32 * n_anal as f32 / n_fft as f32;
        }
        // Periodic Hann
        let window = (0..n_fft)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / n_fft as f32).cos())
            .collect();
        PitchShifter {
            n_anal,
            n_synth,
            n_fft,
            unwrapdata,
            phi_prev: vec![0.0; bins],
            phi_syn: vec![0.0; bins],
            window,
            ola: vec![0.0; n_fft],
            ola_norm: vec![0.0; n_fft],
            first_time: true,
            sample_rate,
        }
    }

    // in: fftlen + analysislen
    pub fn process(&mut self, s: &[f32]) -> PitchShiftResult {
        let windowed: Vec<f32> = s.iter().zip(&self.window).map(|(x, w)| x * w).collect();
        let sf = rfft(&windowed);
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
            phi_unwrap[i] =
                (dphi + self.unwrapdata[i]) * (self.n_synth as f32 / self.n_anal as f32);
        }

        if self.first_time {
            self.phi_syn.clone_from_slice(&phi);
            self.first_time = false;
        } else {
            let mag: Vec<f32> = sf.iter().map(|c| c.norm()).collect();
            lock_phases(&mut self.phi_syn, &phi, &phi_unwrap, &mag);
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

        // Weighted overlap-add at the synthesis hop. Divide by the summed
        // squared window so the gain stays flat for any n_synth.
        for (i, (s, w)) in synth.iter().zip(&self.window).enumerate().take(self.n_fft) {
            self.ola[i] += s * w;
            self.ola_norm[i] += w * w;
        }
        let obuf: Vec<f32> = self.ola[..self.n_synth]
            .iter()
            .zip(&self.ola_norm[..self.n_synth])
            .map(|(x, n)| if *n > 1e-3 { x / n } else { 0.0 })
            .collect();
        self.ola.copy_within(self.n_synth.., 0);
        self.ola_norm.copy_within(self.n_synth.., 0);
        let tail = self.n_fft - self.n_synth;
        self.ola[tail..].fill(0.0);
        self.ola_norm[tail..].fill(0.0);

        // Resample obuf of length n_synth back to n_anal via linear interpolation
        let mut out = vec![0.0; self.n_anal];
        let hop = self.n_synth as f32 / self.n_anal as f32;
        for (i, o) in out.iter_mut().enumerate() {
            let idx = i as f32 * hop;
            let idx_floor = idx.floor() as usize;
            let frac = idx - idx.floor();
            let a = obuf.get(idx_floor).copied().unwrap_or(0.0);
            let b = obuf.get(idx_floor + 1).copied().unwrap_or(0.0);
            *o = a * (1.0 - frac) + b * frac;
        }
        // Save phi for next iteration
        self.phi_prev.clone_from_slice(&phi);

        PitchShiftResult {
            samples: out,
            peak_freq,
        }
    }
}

/// Identity phase locking (Laroche and Dolson, 1999). Only spectral peaks
/// advance their phase. Other bins keep their phase offset to the peak of
/// their region. Without this, bin phases drift apart over time and the
/// output loses level and sounds "phasey".
fn lock_phases(phi_syn: &mut [f32], phi: &[f32], phi_unwrap: &[f32], mag: &[f32]) {
    let bins = mag.len();
    let peaks: Vec<usize> = (1..bins - 1)
        .filter(|&k| mag[k] > mag[k - 1] && mag[k] >= mag[k + 1])
        .collect();
    if peaks.is_empty() {
        for k in 0..bins {
            phi_syn[k] = (phi_syn[k] + phi_unwrap[k]).rem_euclid(2.0 * PI);
        }
        return;
    }

    let mut region_start = 0;
    for (j, &p) in peaks.iter().enumerate() {
        let region_end = match peaks.get(j + 1) {
            Some(&next) => (p + 1..next)
                .min_by(|&a, &b| mag[a].total_cmp(&mag[b]))
                .unwrap_or(next),
            None => bins,
        };
        let peak_syn = (phi_syn[p] + phi_unwrap[p]).rem_euclid(2.0 * PI);
        for k in region_start..region_end {
            phi_syn[k] = peak_syn + phi[k] - phi[p];
        }
        region_start = region_end;
    }
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

    // Divide by the Hann coherent gain (0.5) because the spectrum is windowed.
    let amplitude = peak_bin.norm() / (spectrum.len() - 1) as f32 / 0.5;
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
