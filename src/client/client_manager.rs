use std::io::{Read, Write};
use streamapp::audio;
use streamapp::audio::file::{AudioPlayer, AudioWriter};
use streamapp::audio::wav::WavFileWrite;
use streamapp::protocol::extract_header;

pub struct ClientInterface {
    tcp_stream: std::net::TcpStream,
    audio_capabilities: Vec<Box<dyn AudioWriter>>,
    play_audio_after_download: Option<String>,
    audio_player: Box<dyn AudioPlayer>,
}

pub struct ClientSocket {
    pub address: String,
    pub port: u16,
}

#[allow(unused)]
pub enum Capabilities {
    SaveToFile(String),
    PlayFileAfterDownload(String),
    RealTimePlayback,
}

impl ClientInterface {
    pub fn add_capability(&mut self, capability: Capabilities) -> &mut ClientInterface {
        match capability {
            Capabilities::SaveToFile(s) => {
                self.audio_capabilities.push(Box::new(WavFileWrite::new(s)));
            }
            Capabilities::RealTimePlayback => {
                todo!("Real-time playback not implemented yet");
            }
            Capabilities::PlayFileAfterDownload(file) => {
                self.play_audio_after_download = Some(file);
            }
        }
        self
    }

    fn update_audio_capabilities(&mut self, header: &streamapp::protocol::Header) {
        for capability in &mut self.audio_capabilities {
            capability.update_format(header);
        }
    }
    fn write_audio_data(&mut self, data: &[u8]) -> Result<(), String> {
        for capability in &mut self.audio_capabilities {
            capability.write(data)?;
        }
        Ok(())
    }

    fn end_audio(&mut self) -> Result<(), String> {
        for capability in &mut self.audio_capabilities {
            capability.finalize()?;
        }
        Ok(())
    }

    pub fn start_playing(&mut self) -> Result<(), String> {
        let buf = streamapp::protocol::make_start_playing_message();
        self.tcp_stream.write_all(&buf).map_err(|e| e.to_string())?;

        let mut recv_buf = Vec::new();
        let mut tmp_buf = [0u8; 4096];
        let mut updated = false;

        loop {
            let n = self
                .tcp_stream
                .read(&mut tmp_buf)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }

            recv_buf.extend_from_slice(&tmp_buf[..n]);

            while let Some((header, payload, message_len)) = extract_header(&recv_buf) {
                if !updated {
                    self.update_audio_capabilities(&header);
                    updated = true;
                }

                if !header.is_data_message() {
                    return self.end_audio();
                }

                self.write_audio_data(payload)?;

                recv_buf.drain(..message_len);
            }
        }
        self.end_audio()?;
        if let Some(file) = self.play_audio_after_download.as_ref() {
            self.audio_player
                .as_ref()
                .play_from_file(file, audio::file::FileFormat::Wav)
                .unwrap();
        }
        Ok(())
    }
}

impl ClientSocket {
    pub fn connect(&self) -> Result<ClientInterface, String> {
        let addr = format!("{}:{}", self.address, self.port);
        let stream = std::net::TcpStream::connect(addr).map_err(|e| e.to_string())?;
        let interface = ClientInterface {
            tcp_stream: stream,
            audio_capabilities: vec![],
            play_audio_after_download: None,
            audio_player: Box::new(audio::cpal::CpalInterface),
        };
        Ok(interface)
    }
}
