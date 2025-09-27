use std::io::BufWriter;

pub trait AudioWriter {
    fn write(&mut self, data: &[u8]);
    fn finalize(&mut self);
    fn update_format(&mut self, header: &crate::protocol::Header);
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
    fn write(&mut self, data: &[u8]) {
        if self.writer.is_none() {
            panic!("Writer not initialized");
        }

        if let Some(writer) = &mut self.writer {
            for chunk in data.chunks(2) {
                if chunk.len() < 2 {
                    break;
                }
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                writer.write_sample(sample).unwrap();
            }
        }
    }

    fn finalize(&mut self) {
        if let Some(writer) = self.writer.take() {
            writer.finalize().unwrap();
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

pub trait AudioReader {
    fn read(&mut self, data: &mut [u8]) -> usize;
    fn open_file(&mut self, file_path: &str) -> Result<(), String>;
    fn update_header(&mut self, header: &mut crate::protocol::Header);
}

pub struct WavFileRead {
    reader: Option<hound::WavReader<std::io::BufReader<std::fs::File>>>,
}

impl WavFileRead {
    pub fn new() -> Self {
        Self { reader: None }
    }
}

impl AudioReader for WavFileRead {
    fn read(&mut self, data: &mut [u8]) -> usize {
        let mut pos = 0;
        if let Some(reader) = &mut self.reader {
            for sample in reader.samples::<i16>().take(data.len() / 2) {
                if pos + 2 > data.len() {
                    break;
                }

                let s = sample.unwrap();
                let bytes = s.to_le_bytes();

                data[pos..pos + 2].copy_from_slice(&bytes);

                pos += 2;
            }
        }

        pos
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
