use super::*;
use std::f32::consts::PI;

const SAMPLE_RATE: u32 = 48_000;
// hz_ratio = SAMPLE_RATE / WINDOW_LEN = 10.0 Hz/bin exactly, so test
// tones landing on round multiples of 10 Hz sit dead-center on a bin.
const WINDOW_LEN: usize = 4800;
const HZ_RATIO: f32 = SAMPLE_RATE as f32 / WINDOW_LEN as f32;
const TEST_HZ_RAISE: f32 = 70.;

// The spectral stretch copies each input bin into its nearest output
// bin with no interpolation, so several adjacent output bins can end up
// as exact copies of the same input bin. After an irfft/rfft round
// trip, floating-point noise decides which copy reads back as the
// peak. A couple bins of tolerance absorbs that noise; an actual
// wrong-ratio bug would miss by much more than that.
const PEAK_TOLERANCE_HZ: f32 = 2.0 * HZ_RATIO;
const PEAK_TOLERANCE_BINS: usize = 2;


/// Generates a bin-aligned test tone.
///
/// Uses cosine phase rather than sine: `pitch_weight` ranks bins by
/// `re` alone, and a zero-phase sine's energy at an exactly-aligned
/// bin lands almost entirely in `im` (by DFT orthogonality, `re` is
/// ~0), which would make peak detection pick an arbitrary bin. A
/// cosine keeps the energy in `re`, matching what the detector reads.
fn tone(freq_hz: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|n| (2.0 * PI * freq_hz * n as f32 / SAMPLE_RATE as f32).cos())
        .collect()
}

fn dominant_freq(samples: &[f32]) -> f32 {
    let spectrum = rfft(&samples[..WINDOW_LEN]);
    peak_frequency(&spectrum, HZ_RATIO)
}

/// Locates the strongest bin's frequency within `spectrum[lo_idx..hi_idx]`.
/// Ranks bins the same way `pitch_weight` does (by `re`); the `hz_ratio`
/// factor there is a positive constant multiplier, so it can't change
/// which bin wins and is dropped here.
fn peak_freq_in_range(
    spectrum: &[oxifft::Complex<f32>],
    hz_ratio: f32,
    lo_idx: usize,
    hi_idx: usize,
) -> f32 {
    let (idx, _) = spectrum[lo_idx..hi_idx]
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.re.partial_cmp(&b.re).unwrap())
        .unwrap();
    (lo_idx + idx) as f32 * hz_ratio
}

/// Finds the peak within `PEAK_TOLERANCE_BINS` bins of `target_hz`.
fn peak_freq_near(spectrum: &[oxifft::Complex<f32>], target_hz: f32) -> f32 {
    let center_idx = (target_hz / HZ_RATIO).round() as usize;
    let lo = center_idx.saturating_sub(PEAK_TOLERANCE_BINS);
    let hi = (center_idx + PEAK_TOLERANCE_BINS + 1).min(spectrum.len());
    peak_freq_in_range(spectrum, HZ_RATIO, lo, hi)
}

#[test]
fn dominant_freq_finds_a_pure_tone() {
    for test_freq_hz in [110.0_f32, 220.0, 330.0, 440.0] {
        let samples = tone(test_freq_hz, WINDOW_LEN);

        let detected_hz = dominant_freq(&samples);

        assert!(
            (detected_hz - test_freq_hz).abs() <= PEAK_TOLERANCE_HZ,
            "dominant frequency of a {test_freq_hz} Hz pure tone should land near \
             {test_freq_hz} Hz (got {detected_hz} Hz)"
        );
    }
}

#[test]
fn shift_pitch_window_doubles_the_detected_pitch() {
    // Permutations of input tone, all below the 500 Hz octave-downshift
    // threshold so `peak_freq` reflects the fundamental unmodified.
    for test_freq_hz in [110.0_f32, 220.0, 330.0, 440.0] {
        let samples = tone(test_freq_hz, WINDOW_LEN);

        // Setting pitch_amount_hz == the detected pitch makes
        // ratio = (peak + amount) / peak == 2.0.
        let result = shift_pitch_window(&samples, SAMPLE_RATE, WINDOW_LEN, TEST_HZ_RAISE);

        assert_eq!(
            result.peak_freq, test_freq_hz,
            "detected pitch for a {test_freq_hz} Hz input tone"
        );
        assert_eq!(
            result.samples.len(),
            WINDOW_LEN,
            "shifted window length for a {test_freq_hz} Hz input tone"
        );

        let shifted_peak_hz = dominant_freq(&result.samples);
        let expected_hz = test_freq_hz + TEST_HZ_RAISE;
        assert!(
            (shifted_peak_hz - expected_hz).abs() <= PEAK_TOLERANCE_HZ,
            "re-analyzed pitch of the shifted output for a {test_freq_hz} Hz input tone \
             should land near {expected_hz} Hz (got {shifted_peak_hz} Hz)"
        );
    }
}

#[test]
fn shift_pitch_window_preserves_a_perfect_fifth() {
    // A perfect fifth is a 3:2 frequency ratio. The spectral stretch
    // applies one multiplicative ratio to the whole spectrum, so both
    // tones in the chord should move by the same ratio and the interval
    // between them should survive the shift.
    let low_hz = 220.0_f32;
    let high_hz = low_hz * 1.5; // 330 Hz

    let chord: Vec<f32> = tone(low_hz, WINDOW_LEN)
        .iter()
        .zip(tone(high_hz, WINDOW_LEN))
        .map(|(a, b)| a + b)
        .collect();

    // First, detect the fundamental with pitch_amount_hz = 0 (ratio == 1,
    // so the spectrum passes through unchanged). It doesn't matter which
    // of the two equal-amplitude tones gets detected: setting
    // pitch_amount_hz to that value always gives ratio == 2.0.
    let detected_hz = shift_pitch_window(&chord, SAMPLE_RATE, WINDOW_LEN, 0.0).peak_freq;
    let result = shift_pitch_window(&chord, SAMPLE_RATE, WINDOW_LEN, detected_hz);

    let out_spectrum = rfft(&result.samples[..WINDOW_LEN]);
    let low_out_hz = peak_freq_near(&out_spectrum, 2.0 * low_hz);
    let high_out_hz = peak_freq_near(&out_spectrum, 2.0 * high_hz);

    assert!(
        (low_out_hz - 2.0 * low_hz).abs() <= PEAK_TOLERANCE_HZ,
        "lower tone ({low_hz} Hz) should shift to ~{} Hz (got {low_out_hz} Hz)",
        2.0 * low_hz
    );
    assert!(
        (high_out_hz - 2.0 * high_hz).abs() <= PEAK_TOLERANCE_HZ,
        "upper tone ({high_hz} Hz) should shift to ~{} Hz (got {high_out_hz} Hz)",
        2.0 * high_hz
    );

    let interval_ratio = high_out_hz / low_out_hz;
    assert!(
        (interval_ratio - 1.5).abs() < 0.05,
        "perfect fifth (3:2) should be preserved after shifting, got ratio {interval_ratio} \
         ({low_out_hz} Hz to {high_out_hz} Hz)"
    );
}
