use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use clap::Parser;
use pitch_shift::{shift_pitch_window, PitchShiftResult};

// Matches the realtime app's default analysis window length
// (DEFAULT_ANALYSIS_WIN_LENGTH in voice-pitch-feedback/src/main.rs).
const DEFAULT_WINDOW: usize = 1500;

/// Shifts the pitch of a WAV file by a fixed amount and writes the result to
/// another WAV file.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Input WAV file to read.
    input: PathBuf,

    /// Output WAV file to write.
    output: PathBuf,

    /// Pitch shift amount, in Hz.
    #[arg(short = 'p', long = "pitch-hz")]
    pitch_amount_hz: f32,

    /// Analysis window length, in samples.
    #[arg(short = 'w', long, default_value_t = DEFAULT_WINDOW)]
    window: usize,
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
        .map(|channel| {
            shift_channel(
                channel,
                spec.sample_rate,
                args.window,
                args.pitch_amount_hz,
            )
        })
        .collect();

    let output_interleaved = interleave(&shifted_channels);
    write_wav(&args.output, spec, &output_interleaved)?;

    Ok(())
}

/// Runs `samples` through `shift_pitch_window` one non-overlapping window at
/// a time, carrying phase state across windows the same way the realtime
/// `PitchProcessor` does.
fn shift_channel(
    samples: &[f32],
    sample_rate: u32,
    analysis_win_length: usize,
    pitch_amount_hz: f32,
) -> Vec<f32> {
    let mut angle_buffer = vec![0.0; analysis_win_length];
    let mut first_window = true;
    let mut output = Vec::with_capacity(samples.len());

    for chunk in samples.chunks(analysis_win_length) {
        let mut window = chunk.to_vec();
        window.resize(analysis_win_length, 0.0);

        let PitchShiftResult {
            samples: shifted, ..
        } = shift_pitch_window(
            &window,
            sample_rate,
            analysis_win_length,
            pitch_amount_hz,
            &mut angle_buffer,
            first_window,
        );
        first_window = false;

        output.extend_from_slice(&shifted[..chunk.len()]);
    }

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
