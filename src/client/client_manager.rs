use crate::audio;
use crate::audio::file::{AudioPlayer, AudioWriter};
use crate::audio::wav::WavFileWrite;
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct ClientInterface {
    tcp_stream: tokio::net::TcpStream,
    audio_capabilities: Vec<Box<dyn AudioWriter>>,
    play_audio_after_download: Option<String>,
    audio_player: Box<dyn AudioPlayer>,
    protocol_info: Option<crate::protocol::ProtocolInfo>,
}

#[allow(unused)]
pub enum Capabilities {
    SaveToFile(String),
    RealTimePlayback,
}

use bytes::Bytes;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

impl ClientInterface {
    pub async fn connect(address: String, port: u16) -> Result<ClientInterface> {
        let addr = format!("{}:{}", address, port);
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let mut interface = ClientInterface {
            tcp_stream: stream,
            audio_capabilities: vec![],
            play_audio_after_download: None,
            audio_player: Box::new(audio::cpal::CpalInterface),
            protocol_info: None,
        };
        interface.authenticate().await?;
        Ok(interface)
    }

    async fn authenticate(&mut self) -> Result<()> {
        self.send_hello().await?;
        self.protocol_info = Some(self.expect_protocol_info().await?);
        self.send_ok_message().await?;
        Ok(())
    }

    async fn send_ok_message(&mut self) -> Result<()> {
        let ok_msg = crate::protocol::make_ok_message();
        self.tcp_stream
            .write_all(&ok_msg)
            .await
            .map_err(|e| anyhow::anyhow!("Error sending OK message: {}", e))
    }

    async fn expect_protocol_info(&mut self) -> Result<crate::protocol::ProtocolInfo> {
        let mut recv_buf = [0u8; 4096];
        match self.tcp_stream.read(&mut recv_buf).await {
            Ok(0) => Err(anyhow::anyhow!(
                "Connection closed by the server during protocol info"
            )),
            Ok(n) => {
                let recv_buf = &recv_buf[..n];
                crate::protocol::extract_protocol_info(recv_buf).ok_or_else(|| {
                    anyhow::anyhow!("Failed to extract protocol info from server response")
                })
            }
            Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
        }
    }

    async fn send_hello(&mut self) -> Result<()> {
        let client_hello_msg = crate::protocol::make_client_hello_message();
        self.tcp_stream
            .write_all(&client_hello_msg)
            .await
            .map_err(|e: std::io::Error| anyhow::anyhow!(e))?;
        Ok(())
    }

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

    fn update_audio_capabilities(&mut self, header: &crate::protocol::AudioHeader) -> Result<()> {
        for capability in &mut self.audio_capabilities {
            capability.update_format(header)?;
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

    fn is_stop_playing_message(data: &[u8]) -> bool {
        if data.len() != 1 {
            return false;
        }

        let config = bincode::config::standard();
        let (msg_type, _): (crate::protocol::MessageType, usize) =
            match bincode::decode_from_slice(&data[0..1], config) {
                Ok(result) => result,
                Err(_) => return false,
            };

        msg_type == crate::protocol::MessageType::StopPlaying
    }

    async fn recv_data_and_write_it(&mut self) -> Result<()> {
        let mut framed = FramedRead::new(&mut self.tcp_stream, LengthDelimitedCodec::new());

        dbg!("Start receiving audio data from server");

        while let Some(frame) = framed.next().await {
            let bytes: Bytes = frame?.into();

            if Self::is_stop_playing_message(&bytes) {
                dbg!("Stop message received");
                break;
            }
            for capability in &mut self.audio_capabilities {
                capability.write(&bytes)?;
            }
        }

        Ok(())
    }
    async fn update_audio_header(&mut self) -> Result<()> {
        let mut recv_buf = [0u8; 4096];
        match self.tcp_stream.read(&mut recv_buf).await {
            Ok(0) => Err(anyhow::anyhow!(
                "Connection closed by the server during audio header"
            )),
            Ok(n) => {
                let recv_buf = &recv_buf[..n];
                let header =
                    crate::protocol::extract_wav_header(&recv_buf[..n]).ok_or_else(|| {
                        anyhow::anyhow!("Failed to extract audio header from server response")
                    })?;
                dbg!("Received audio header from server: {:?}", header);
                self.update_audio_capabilities(&header)
            }
            Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
        }
    }

    async fn expect_bye_message(&mut self) -> Result<()> {
        let mut recv_buf = [0u8; 4096];
        match self.tcp_stream.read(&mut recv_buf).await {
            Ok(0) => Err(anyhow::anyhow!(
                "Connection closed by the server during BYE message"
            )),
            Ok(n) => {
                let recv_buf = &recv_buf[..n];
                if crate::protocol::check_bye_message(recv_buf) {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Did not receive BYE message from server"))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
        }
    }

    async fn send_bye_message(&mut self) -> Result<()> {
        let bye_msg = crate::protocol::make_bye_message();
        self.tcp_stream
            .write_all(&bye_msg)
            .await
            .map_err(|e| anyhow::anyhow!("Error sending BYE message: {}", e))
    }

    pub async fn start_playing(&mut self) -> Result<()> {
        self.send_start_playing().await?;

        self.update_audio_header().await?;

        dbg!("Audio header updated");

        self.recv_data_and_write_it().await?;
        dbg!("Finished receiving audio data from server");
        self.end_audio()?;

        self.send_bye_message().await?;

        self.expect_bye_message().await?;

        if let Some(file) = self.play_audio_after_download.as_ref() {
            self.audio_player
                .as_ref()
                .play_from_file(file, audio::file::FileFormat::Wav)?;
        }
        Ok(())
    }
}
