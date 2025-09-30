#[derive(Clone)]
pub enum FileFormat {
    Wav,
}

pub trait AudioWriter {
    fn write(&mut self, data: &[u8]) -> Result<(), String>;
    fn finalize(&mut self) -> Result<(), String>;
    fn update_format(&mut self, header: &crate::protocol::Header);
}

pub trait AudioReader {
    fn read(&mut self, data: &mut [u8]) -> Result<usize, String>;
    fn open_file(&mut self, file_path: &str) -> Result<(), String>;
    fn update_header(&mut self, header: &mut crate::protocol::Header);
}

pub trait AudioPlayer {
    fn play_from_file(&self, file_path: &str, format: FileFormat) -> Result<(), String>;
}

pub trait AudioRecorder {
    fn record_into_file(&self, duration: u64, path: &str, format: FileFormat)
    -> Result<(), String>;
}
