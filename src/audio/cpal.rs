use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::audio::file::{AudioPlayer, AudioRecorder, FileFormat};

pub struct CpalInterface;

impl AudioPlayer for CpalInterface {
    fn play_from_file(&self, file_path: &str, format: FileFormat) -> Result<(), String> {
        match format {
            FileFormat::Wav => play_audio_from_wav(file_path).map_err(|e| e.to_string()),
        }
    }
}

impl AudioRecorder for CpalInterface {
    fn record_into_file(
        &self,
        duration: u64,
        path: &str,
        format: FileFormat,
    ) -> Result<(), String> {
        match format {
            FileFormat::Wav => record_audio(duration, path).map_err(|e| e.to_string()),
        }
    }
}

fn play_audio<T>(
    mut reader: hound::WavReader<std::io::BufReader<File>>,
    device: Device,
    config: cpal::StreamConfig,
) -> Result<(), anyhow::Error>
where
    T: hound::Sample + cpal::SizedSample + Send + Sync + 'static,
{
    let samples: Vec<T> = reader
        .samples::<T>()
        .map(|s| s.unwrap_or(T::EQUILIBRIUM))
        .collect();

    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();

    let mut samples_iter = samples.into_iter();

    let err_fn = move |err| eprintln!("an error occurred on stream: {err}");

    let stream = device.build_output_stream(
        &config,
        move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
            for sample in output.iter_mut() {
                *sample = samples_iter.next().unwrap_or(T::EQUILIBRIUM);
            }
            if samples_iter.len() == 0 {
                done_clone.store(true, Ordering::Release);
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    while !done.load(Ordering::Acquire) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(())
}

pub fn play_audio_from_wav(path: &str) -> Result<(), anyhow::Error> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No output device available"))?;
    println!("Output device: {}", device.name()?);

    let default_config = device.default_output_config()?;
    let reader: hound::WavReader<std::io::BufReader<File>> = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let config = cpal::StreamConfig {
        channels: default_config.channels(),
        sample_rate: cpal::SampleRate(spec.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    match spec.sample_format {
        hound::SampleFormat::Float => match spec.bits_per_sample {
            32 => play_audio::<f32>(reader, device, config),
            _ => unimplemented!(),
        },
        hound::SampleFormat::Int => match spec.bits_per_sample {
            32 => play_audio::<i32>(reader, device, config),
            16 => play_audio::<i16>(reader, device, config),
            _ => unimplemented!(),
        },
    }
}

fn record_audio(duration: u64, path: &str) -> Result<(), anyhow::Error> {
    let host = cpal::default_host();

    let device = host.default_input_device().unwrap();

    println!("Input device: {}", device.name()?);

    let config = device.default_input_config()?;

    let spec = wav_spec_from_config(&config);
    let writer = hound::WavWriter::create(path, spec)?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    println!("Begin recording...");

    let writer_2 = writer.clone();

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {err}");
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::I8 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i8, i8>(data, &writer_2),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i16, i16>(data, &writer_2),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I32 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i32, i32>(data, &writer_2),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<f32, f32>(data, &writer_2),
            err_fn,
            None,
        )?,
        sample_format => {
            return Err(anyhow::Error::msg(format!(
                "Unsupported sample format '{sample_format}'"
            )));
        }
    };

    stream.play()?;

    std::thread::sleep(std::time::Duration::from_secs(duration));
    drop(stream);
    writer.lock().unwrap().take().unwrap().finalize()?;
    println!("Recording {path} complete!");
    Ok(())
}

fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    }
}

fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format(config.sample_format()),
    }
}

type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                writer.write_sample(sample).ok();
            }
        }
    }
}
