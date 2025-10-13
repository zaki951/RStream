use crate::audio;
use crate::audio::file::{AudioPlayer, AudioWriter};
use crate::audio::wav::WavFileWrite;
use crate::protocol::extract_header;
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct ClientInterface {
    tcp_stream: tokio::net::TcpStream,
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
    RealTimePlayback,
}

impl ClientInterface {
    pub fn add_capability(&mut self, capability: Capabilities) -> &mut ClientInterface {
        match capability {
            Capabilities::SaveToFile(s) => {
                self.audio_capabilities.push(Box::new(WavFileWrite::new(s)));
            }
            Capabilities::RealTimePlayback => {
                self.audio_capabilities
                    .push(Box::new(audio::cpal::CpalFileWrite::new()));
            }
        }
        self
    }

    fn update_audio_capabilities(&mut self, header: &crate::protocol::Header) -> Result<()> {
        for capability in &mut self.audio_capabilities {
            capability.update_format(header)?;
        }
        Ok(())
    }
    fn write_audio_data(&mut self, data: &[u8]) -> Result<()> {
        for capability in &mut self.audio_capabilities {
            capability.write(data)?;
        }
        Ok(())
    }

    fn end_audio(&mut self) -> Result<()> {
        for capability in &mut self.audio_capabilities {
            capability.finalize()?;
        }
        Ok(())
    }

    async fn send_start_playing(&mut self) -> Result<()> {
        let buf = crate::protocol::make_start_playing_message();
        self.tcp_stream
            .write_all(&buf)
            .await
            .map_err(|e: std::io::Error| anyhow::anyhow!(e))
    }

    async fn recv_data_and_write_it(&mut self) -> Result<()> {
        let mut updated = false;
        let mut recv_buf = Vec::new();
        let mut tmp_buf = [0u8; 4096];
        loop {
            let n = self.tcp_stream.read(&mut tmp_buf).await?;
            if n == 0 {
                break;
            }

            recv_buf.extend_from_slice(&tmp_buf[..n]);

            while let Some((header, payload, message_len)) = extract_header(&recv_buf) {
                if !updated {
                    self.update_audio_capabilities(&header)?;
                    updated = true;
                }

                if !header.is_data_message() {
                    return Ok(());
                }

                self.write_audio_data(payload)?;

                recv_buf.drain(..message_len);
            }
        }
        Ok(())
    }

    pub async fn start_playing(&mut self) -> Result<()> {
        self.send_start_playing().await?;

        self.recv_data_and_write_it().await?;

        self.end_audio()?;
        if let Some(file) = self.play_audio_after_download.as_ref() {
            self.audio_player
                .as_ref()
                .play_from_file(file, audio::file::FileFormat::Wav)?;
        }
        Ok(())
    }
}

impl ClientSocket {
    pub async fn connect(&self) -> Result<ClientInterface, String> {
        let addr = format!("{}:{}", self.address, self.port);
        let stream = tokio::net::TcpStream::connect(addr)
            .await
            .map_err(|e| e.to_string())?;
        let interface = ClientInterface {
            tcp_stream: stream,
            audio_capabilities: vec![],
            play_audio_after_download: None,
            audio_player: Box::new(audio::cpal::CpalInterface),
        };
        Ok(interface)
    }
}
