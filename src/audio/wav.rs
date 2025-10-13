use crate::{
    audio::file::{AudioReader, AudioWriter},
    protocol::{self},
};
use anyhow::Result;
use std::io::BufWriter;
use tokio::{io::AsyncWriteExt, net::TcpStream};

pub struct WavFileRead {
    reader: Option<hound::WavReader<std::io::BufReader<std::fs::File>>>,
}

impl WavFileRead {
    pub fn new() -> Self {
        Self { reader: None }
    }
}

fn read_i32_samples(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    data: &mut [u8],
) -> Result<usize> {
    let mut pos = 0;
    for sample in reader.samples::<i32>().take(data.len() / 4) {
        if pos + 4 > data.len() {
            break;
        }

        let s = sample?;
        let bytes = s.to_le_bytes();

        data[pos..pos + 4].copy_from_slice(&bytes);

        pos += 4;
    }
    Ok(pos)
}

fn read_i16_samples(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    data: &mut [u8],
) -> Result<usize> {
    let mut pos = 0;
    for sample in reader.samples::<i16>().take(data.len() / 2) {
        if pos + 2 > data.len() {
            break;
        }

        let s = sample?;
        let bytes = s.to_le_bytes();

        data[pos..pos + 2].copy_from_slice(&bytes);

        pos += 2;
    }
    Ok(pos)
}

fn read_f32_samples(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    data: &mut [u8],
) -> Result<usize> {
    let mut pos = 0;
    for sample in reader.samples::<f32>().take(data.len() / 4) {
        if pos + 4 > data.len() {
            break;
        }

        let s = sample?;
        let bytes = s.to_le_bytes();

        data[pos..pos + 4].copy_from_slice(&bytes);

        pos += 4;
    }
    Ok(pos)
}

impl AudioReader for WavFileRead {
    fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        if let Some(reader) = &mut self.reader {
            let sample_format = reader.spec().sample_format;

            let pos = match sample_format {
                hound::SampleFormat::Int => match reader.spec().bits_per_sample {
                    16 => read_i16_samples(reader, data)?,
                    32 => read_i32_samples(reader, data)?,
                    bits => {
                        return Err(anyhow::anyhow!("Unsupported bit depth: {}", bits));
                    }
                },
                hound::SampleFormat::Float => match reader.spec().bits_per_sample {
                    32 => read_f32_samples(reader, data)?,
                    64 => return Err(anyhow::anyhow!("64-bit float samples not supported")),
                    bits => {
                        return Err(anyhow::anyhow!("Unsupported bit depth: {}", bits));
                    }
                },
            };
            return Ok(pos);
        }

        Ok(0)
    }

    fn open_file(&mut self, file_path: &str) -> Result<()> {
        if self.reader.is_some() {
            return Err(anyhow::anyhow!("File already opened"));
        }
        let reader = hound::WavReader::open(file_path)?;
        self.reader = Some(reader);
        Ok(())
    }

    fn update_header(&mut self, header: &mut crate::protocol::Header) {
        if let Some(reader) = &self.reader {
            let spec = reader.spec();
            header.update_wavspec(&spec);
        }
    }
}

pub struct WavFileWrite {
    writer: Option<hound::WavWriter<BufWriter<std::fs::File>>>,
    file_path: String,
}

impl WavFileWrite {
    pub fn new(file_path: String) -> Self {
        Self {
            writer: None,
            file_path,
        }
    }
}

impl AudioWriter for WavFileWrite {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        let writer = self
            .writer
            .as_mut()
            .ok_or(anyhow::anyhow!("Writer not initialized"))?;
        let spec = writer.spec();
        match spec.sample_format {
            hound::SampleFormat::Int => match spec.bits_per_sample {
                16 => {
                    for chunk in data.chunks_exact(2) {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                        writer.write_sample(sample)?;
                    }
                }
                32 => {
                    for chunk in data.chunks_exact(4) {
                        let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        writer.write_sample(sample)?;
                    }
                }
                _ => unimplemented!(),
            },
            hound::SampleFormat::Float => match spec.bits_per_sample {
                32 => {
                    for chunk in data.chunks_exact(4) {
                        let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        writer.write_sample(sample)?;
                    }
                }
                _ => unimplemented!(),
            },
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        if let Some(writer) = self.writer.take() {
            writer.finalize().map_err(|e| anyhow::anyhow!(e))
        } else {
            Ok(())
        }
    }
    fn update_format(&mut self, header: &crate::protocol::Header) -> Result<()> {
        if self.writer.is_none() {
            let spec = header.to_wavspec();
            let writer = hound::WavWriter::create(&self.file_path, spec)?;
            self.writer = Some(writer);
        }
        Ok(())
    }
}

pub struct WavFileSender;

impl WavFileSender {
    pub async fn send_file(&self, socket: &mut TcpStream, file_path: &str) -> Result<()> {
        let mut audio_reader = WavFileRead::new();
        audio_reader.open_file(file_path)?;
        let mut buffer = vec![0u8; 4096];

        let mut header = protocol::Header::new(protocol::MessageType::RawData);
        audio_reader.update_header(&mut header);
        loop {
            let n = audio_reader.read(&mut buffer[..])?;
            if n == 0 {
                break;
            }
            header.set_payload_size(n as u32);
            let fmessage = protocol::make_full_message(&header, &buffer[..n]);
            socket.write_all(&fmessage).await?;
            if n < buffer.len() {
                break;
            }
        }

        Ok(())
    }
}
