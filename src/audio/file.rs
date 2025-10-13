use anyhow::Result;

#[derive(Clone)]
pub enum FileFormat {
    Wav,
}

pub trait AudioWriter {
    fn write(&mut self, data: &[u8]) -> Result<()>;
    fn finalize(&mut self) -> Result<()>;
    fn update_format(&mut self, header: &crate::protocol::Header) -> Result<()>;
}

pub trait AudioReader {
    fn read(&mut self, data: &mut [u8]) -> Result<usize>;
    fn open_file(&mut self, file_path: &str) -> Result<()>;
    fn update_header(&mut self, header: &mut crate::protocol::Header);
}

pub trait AudioPlayer {
    fn play_from_file(&self, file_path: &str, format: FileFormat) -> Result<()>;
}

pub trait AudioRecorder {
    fn record_into_file(&self, duration: u64, path: &str, format: FileFormat) -> Result<()>;
}
