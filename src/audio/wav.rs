use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::{
    audio::file::{AudioReader, AudioWriter},
    protocol::{self},
};
use std::io::BufWriter;

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
) -> Result<usize, String> {
    dbg!();
    let mut pos = 0;
    for sample in reader.samples::<i32>().take(data.len() / 4) {
        if pos + 4 > data.len() {
            break;
        }

        let s = sample.map_err(|e| e.to_string())?;
        let bytes = s.to_le_bytes();

        data[pos..pos + 4].copy_from_slice(&bytes);

        pos += 4;
    }
    Ok(pos)
}

fn read_i16_samples(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    data: &mut [u8],
) -> Result<usize, String> {
    dbg!();
    let mut pos = 0;
    for sample in reader.samples::<i16>().take(data.len() / 2) {
        if pos + 2 > data.len() {
            break;
        }

        let s = sample.map_err(|e| e.to_string())?;
        let bytes = s.to_le_bytes();

        data[pos..pos + 2].copy_from_slice(&bytes);

        pos += 2;
    }
    Ok(pos)
}

fn read_f32_samples(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    data: &mut [u8],
) -> Result<usize, String> {
    let mut pos = 0;
    for sample in reader.samples::<f32>().take(data.len() / 4) {
        if pos + 4 > data.len() {
            break;
        }

        let s = sample.map_err(|e| e.to_string())?;
        let bytes = s.to_le_bytes();

        data[pos..pos + 4].copy_from_slice(&bytes);

        pos += 4;
    }
    Ok(pos)
}

impl AudioReader for WavFileRead {
    fn read(&mut self, data: &mut [u8]) -> Result<usize, String> {
        if let Some(reader) = &mut self.reader {
            let sample_format = reader.spec().sample_format;

            let pos = match sample_format {
                hound::SampleFormat::Int => match reader.spec().bits_per_sample {
                    16 => read_i16_samples(reader, data)?,
                    32 => read_i32_samples(reader, data)?,
                    bits => {
                        return Err(format!("Unsupported bit depth: {}", bits));
                    }
                },
                hound::SampleFormat::Float => match reader.spec().bits_per_sample {
                    32 => read_f32_samples(reader, data)?,
                    64 => unimplemented!("64-bit float samples not supported"),
                    bits => {
                        return Err(format!("Unsupported bit depth: {}", bits));
                    }
                },
            };
            return Ok(pos);
        }

        Ok(0)
    }

    fn open_file(&mut self, file_path: &str) -> Result<(), String> {
        if self.reader.is_some() {
            return Err("File already opened".to_string());
        }
        let reader = hound::WavReader::open(file_path).map_err(|e| e.to_string())?;
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
    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        if self.writer.is_none() {
            panic!("Writer not initialized");
        }

        if let Some(writer) = &mut self.writer {
            let size = match writer.spec().bits_per_sample {
                16 => 2,
                32 => 4,
                _ => return Err("Unsupported bits_per_sample".to_string()),
            };
            for chunk in data.chunks(size) {
                if chunk.len() < size {
                    break;
                }
                match writer.spec().sample_format {
                    hound::SampleFormat::Int => match writer.spec().bits_per_sample {
                        16 => {
                            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                            writer.write_sample(sample).map_err(|e| e.to_string())?;
                        }
                        32 => {
                            let sample =
                                i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                            writer.write_sample(sample).map_err(|e| e.to_string())?;
                        }
                        _ => todo!(),
                    },
                    hound::SampleFormat::Float => match writer.spec().bits_per_sample {
                        32 => {
                            let sample =
                                f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                            writer.write_sample(sample).map_err(|e| e.to_string())?;
                        }
                        _ => todo!(),
                    },
                }
            }
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), String> {
        if let Some(writer) = self.writer.take() {
            writer.finalize().map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }
    fn update_format(&mut self, header: &crate::protocol::Header) {
        if self.writer.is_none() {
            dbg!("Initializing WAV writer with header: {:?}", header);
            let spec = header.to_wavspec();
            let writer = hound::WavWriter::create(&self.file_path, spec).unwrap();
            self.writer = Some(writer);
        } else {
            todo!()
        }
    }
}

pub struct WavFileSender;

impl WavFileSender {
    pub async fn send_file(&self, socket: &mut TcpStream, file_path: &str) -> Result<(), String> {
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
            header.set_payload_size(n as u16);
            let fmessage = protocol::make_full_message(&header, &buffer[..n]);
            socket
                .write_all(&fmessage)
                .await
                .map_err(|e| e.to_string())?;
            if n < buffer.len() {
                break;
            }
        }

        Ok(())
    }
}
