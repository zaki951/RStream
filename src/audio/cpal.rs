use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample};
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufWriter;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, mpsc};

use crate::audio::file::{AudioPlayer, AudioRecorder, AudioWriter, FileFormat};
use crate::protocol::AudioHeader;

pub struct CpalInterface;

impl AudioPlayer for CpalInterface {
    fn play_from_file(&self, file_path: &str, format: FileFormat) -> Result<()> {
        match format {
            FileFormat::Wav => play_audio_from_wav(file_path),
        }
    }
}

impl AudioRecorder for CpalInterface {
    fn record_into_file(
        &self,
        duration: u64,
        path: &str,
        format: FileFormat,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        match format {
            FileFormat::Wav => record_audio(duration, path),
        }
    }
}

fn play_audio_wav_file<T>(
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

    let (tx, rx) = std::sync::mpsc::channel::<()>();

    let mut samples_iter = samples.into_iter();

    let err_fn = move |err| eprintln!("an error occurred on stream: {err}");

    let stream = device.build_output_stream(
        &config,
        move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
            for sample in output.iter_mut() {
                *sample = samples_iter.next().unwrap_or(T::EQUILIBRIUM);
            }
            if samples_iter.len() == 0 {
                tx.send(()).unwrap();
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    rx.recv().unwrap();

    Ok(())
}

pub fn play_audio_from_wav(path: &str) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No output device available"))?;
    println!("Output device: {}", device.name()?);

    let reader: hound::WavReader<std::io::BufReader<File>> = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let config = cpal::StreamConfig {
        channels: spec.channels as u16,
        sample_rate: cpal::SampleRate(spec.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    match spec.sample_format {
        hound::SampleFormat::Float => match spec.bits_per_sample {
            32 => play_audio_wav_file::<f32>(reader, device, config),
            _ => unimplemented!(),
        },
        hound::SampleFormat::Int => match spec.bits_per_sample {
            32 => play_audio_wav_file::<i32>(reader, device, config),
            16 => play_audio_wav_file::<i16>(reader, device, config),
            _ => unimplemented!(),
        },
    }
}

async fn record_audio(duration: u64, path: &str) -> Result<()> {
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

    tokio::time::sleep(std::time::Duration::from_secs(duration)).await;
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

pub struct CpalFileWrite {
    buf: Arc<Mutex<VecDeque<u8>>>,
    play_done_tx: mpsc::Sender<()>,
    play_done_rx: mpsc::Receiver<()>,
    first_play: AtomicBool,
    stream: Option<cpal::Stream>,
    header: Option<AudioHeader>,
}

impl CpalFileWrite {
    pub fn new() -> Self {
        const DEFAULT_CAPACITY: usize = 400_000;
        let (tx, rx) = mpsc::channel();

        Self {
            buf: Arc::new(Mutex::new(VecDeque::with_capacity(DEFAULT_CAPACITY))),
            play_done_tx: tx,
            play_done_rx: rx,
            first_play: AtomicBool::new(true),
            stream: None,
            header: None,
        }
    }

    fn play_audio_from_buf(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device available"))?;

        if self.header.is_none() {
            return Err(anyhow::anyhow!("Audio format header not set"));
        }
        let header = self.header.as_ref().unwrap();
        dbg!(header);
        let config = cpal::StreamConfig {
            channels: header.get_channels() as u16,
            sample_rate: cpal::SampleRate(header.get_sample_rate()),
            buffer_size: cpal::BufferSize::Default,
        };

        let err_fn = move |err| eprintln!("an error occurred on stream: {err}");
        let cloned_buf = Arc::clone(&self.buf);

        match header.get_sample_format() {
            crate::protocol::SampleFormat::Int => match header.get_bits_per_sample() {
                16 => return self.build_stream::<i16>(device, config, cloned_buf, err_fn),
                32 => return self.build_stream::<i32>(device, config, cloned_buf, err_fn),
                _ => return Err(anyhow::anyhow!("Unsupported bits per sample")),
            },
            crate::protocol::SampleFormat::Float => match header.get_bits_per_sample() {
                32 => return self.build_stream::<f32>(device, config, cloned_buf, err_fn),
                _ => return Err(anyhow::anyhow!("Unsupported bits per sample")),
            },
        }
    }

    fn extract_bytes_from_buf(buf: &mut VecDeque<u8>, sample_size: usize) -> Option<Vec<u8>> {
        if buf.len() < sample_size {
            return None;
        }
        let mut bytes = Vec::with_capacity(sample_size);
        for _ in 0..sample_size {
            bytes.push(buf.pop_front().unwrap());
        }
        Some(bytes)
    }

    fn build_stream<T>(
        &mut self,
        device: cpal::Device,
        config: cpal::StreamConfig,
        buf: Arc<Mutex<VecDeque<u8>>>,
        err_fn: impl Fn(cpal::StreamError) + Send + 'static,
    ) -> Result<(), anyhow::Error>
    where
        T: cpal::Sample
            + cpal::SizedSample
            + FromSample<i16>
            + FromSample<i32>
            + FromSample<f32>
            + Send
            + 'static,
    {
        let channels = config.channels as usize;
        let sample_size = std::mem::size_of::<T>();
        let frame_size = channels * sample_size;
        let tx1 = self.play_done_tx.clone();
        let notified = std::sync::Arc::new(AtomicBool::new(false));
        let notified_clone = notified.clone();
        let stream = device.build_output_stream(
            &config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut buf = buf.lock().unwrap();
                for frame in output.chunks_mut(channels) {
                    if buf.len() >= frame_size {
                        for sample in frame.iter_mut() {
                            let bytes =
                                Self::extract_bytes_from_buf(&mut buf, sample_size).unwrap();
                            let value = match sample_size {
                                2 => {
                                    let arr: [u8; 2] =
                                        bytes.try_into().expect("bytes must be 2 long");
                                    let val = i16::from_le_bytes(arr);
                                    T::from_sample(val)
                                }
                                4 => {
                                    let arr: [u8; 4] =
                                        bytes.try_into().expect("bytes must be 4 long");
                                    if std::any::TypeId::of::<T>() == std::any::TypeId::of::<f32>()
                                    {
                                        let val = f32::from_le_bytes(arr);
                                        T::from_sample(val)
                                    } else {
                                        let val = i32::from_le_bytes(arr);
                                        T::from_sample(val)
                                    }
                                }
                                _ => T::EQUILIBRIUM,
                            };
                            *sample = value;
                        }
                    } else {
                        for sample in frame.iter_mut() {
                            *sample = T::EQUILIBRIUM;
                        }
                    }

                    if buf.is_empty() && !notified_clone.load(Ordering::Relaxed) {
                        tx1.send(()).unwrap();
                        notified_clone.store(true, Ordering::Relaxed);
                    }
                }
            },
            err_fn,
            None,
        )?;

        self.stream = Some(stream);
        Ok(())
    }
}

impl AudioWriter for CpalFileWrite {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        if self.first_play.load(Ordering::Relaxed) {
            self.play_audio_from_buf()?;
            if let Some(stream) = &self.stream {
                stream.play()?;
            }
            self.first_play.store(false, Ordering::Relaxed);
        }
        let mut buf = self.buf.lock().unwrap();
        buf.extend(data);
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        self.play_done_rx.recv().unwrap();
        dbg!("Buffer emptied, stopping stream.");
        if let Some(stream) = &self.stream {
            stream.pause()?;
            self.stream = None;
        }

        Ok(())
    }

    fn update_format(&mut self, header: &crate::protocol::AudioHeader) -> Result<()> {
        self.header = Some(header.clone());
        Ok(())
    }
}
