use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use clap::Parser;
use pitch_shift::{
    synthesis_length, PitchShiftResult, PitchShifter, ANALYSIS_WIN_LENGTH_OPTIONS,
    DEFAULT_ANALYSIS_WIN_LENGTH, DEFAULT_FFT_LENGTH, DEFAULT_PITCH_AMOUNT, DEFAULT_TARGET_PITCH,
    FFT_LENGTH_OPTIONS,
};

/// Shifts the pitch of a WAV file and writes the result to another WAV file.
/// Uses the same processing and settings as the realtime app.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Input WAV file to read.
    input: PathBuf,

    /// Output WAV file to write.
    output: PathBuf,

    /// Pitch shift amount, in Hz.
    #[arg(short = 'p', long = "pitch-hz", default_value_t = DEFAULT_PITCH_AMOUNT)]
    pitch_amount_hz: f32,

    /// Target pitch, in Hz.
    #[arg(short = 't', long = "target-pitch", default_value_t = DEFAULT_TARGET_PITCH)]
    target_pitch_hz: f32,

    /// Analysis window length, in samples.
    #[arg(short = 'w', long, default_value_t = DEFAULT_ANALYSIS_WIN_LENGTH,
          value_parser = one_of(&ANALYSIS_WIN_LENGTH_OPTIONS))]
    window: usize,

    /// FFT length, in samples.
    #[arg(short = 'f', long, default_value_t = DEFAULT_FFT_LENGTH,
          value_parser = one_of(&FFT_LENGTH_OPTIONS))]
    fft_length: usize,
}

fn one_of(options: &'static [usize]) -> impl Fn(&str) -> Result<usize, String> + Clone {
    move |s| {
        let value: usize = s.parse().map_err(|e| format!("{e}"))?;
        if options.contains(&value) {
            Ok(value)
        } else {
            Err(format!("must be one of {options:?}"))
        }
    }
}

fn main() {
    let args = Args::parse();
    if let Err(err) = run(&args) {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Box<dyn Error>> {
    let mut reader = hound::WavReader::open(&args.input)?;
    let spec = reader.spec();
    let interleaved = read_samples(&mut reader)?;
    let channels = deinterleave(&interleaved, spec.channels as usize);

    let shifted_channels: Vec<Vec<f32>> = channels
        .iter()
        .map(|channel| shift_channel(channel, spec.sample_rate, args))
        .collect();

    let output_interleaved = interleave(&shifted_channels);
    write_wav(&args.output, spec, &output_interleaved)?;

    Ok(())
}

/// Copies the buffering of the realtime `PitchProcessor`, so the output has
/// the same latency as the app. The output is `fft_length` samples longer
/// than the input, so the tail is not cut off.
fn shift_channel(samples: &[f32], sample_rate: u32, args: &Args) -> Vec<f32> {
    let n_fft = args.fft_length;
    let n_anal = args.window;
    let n_synth = synthesis_length(n_anal, args.target_pitch_hz, args.pitch_amount_hz);
    let mut shifter = PitchShifter::new(n_anal, n_synth, n_fft, sample_rate as usize);

    let mut padded = samples.to_vec();
    padded.resize(samples.len() + n_fft, 0.0);

    // The app outputs silence until the first full FFT frame is available.
    let mut output = vec![0.0; n_fft];
    let mut pos = 0;
    while pos + n_fft <= padded.len() {
        let PitchShiftResult { samples: shifted, .. } = shifter.process(&padded[pos..pos + n_fft]);
        output.extend_from_slice(&shifted);
        pos += n_anal;
    }
    output.truncate(padded.len());
    output
}

fn deinterleave(samples: &[f32], channels: usize) -> Vec<Vec<f32>> {
    let mut result = vec![Vec::with_capacity(samples.len() / channels.max(1)); channels];
    for frame in samples.chunks(channels) {
        for (c, &s) in frame.iter().enumerate() {
            result[c].push(s);
        }
    }
    result
}

fn interleave(channels: &[Vec<f32>]) -> Vec<f32> {
    let len = channels.first().map_or(0, Vec::len);
    let mut result = Vec::with_capacity(len * channels.len());
    for i in 0..len {
        for channel in channels {
            result.push(channel[i]);
        }
    }
    result
}

fn read_samples(
    reader: &mut hound::WavReader<BufReader<File>>,
) -> Result<Vec<f32>, hound::Error> {
    let spec = reader.spec();
    match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect(),
        hound::SampleFormat::Int => {
            let max_value = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_value))
                .collect()
        }
    }
}

fn write_wav(path: &Path, spec: hound::WavSpec, interleaved: &[f32]) -> Result<(), hound::Error> {
    let mut writer = hound::WavWriter::create(path, spec)?;
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for &s in interleaved {
                writer.write_sample(s)?;
            }
        }
        hound::SampleFormat::Int => {
            let max_value = (1i64 << (spec.bits_per_sample - 1)) as f32 - 1.0;
            for &s in interleaved {
                let v = (s.clamp(-1.0, 1.0) * max_value).round() as i32;
                writer.write_sample(v)?;
            }
        }
    }
    writer.finalize()?;
    Ok(())
}
