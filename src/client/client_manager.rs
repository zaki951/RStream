use crate::audio::file::{AudioPlayer, AudioWriter};
use crate::audio::wav::WavFileWrite;
use crate::{audio, network, protocol};
use anyhow::Result;
use tokio::io::AsyncReadExt;

pub struct ClientInterface {
    tcp_stream: tokio::net::TcpStream,
    audio_capabilities: Vec<Box<dyn AudioWriter>>,
    play_audio_after_download: Option<String>,
    audio_player: Box<dyn AudioPlayer>,
    #[allow(unused)]
    protocol_info: crate::protocol::ProtocolInfo,
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
        let mut stream = tokio::net::TcpStream::connect(addr).await?;
        let pinfo = network::common::client_authenticate(&mut stream).await?;
        let interface = ClientInterface {
            tcp_stream: stream,
            audio_capabilities: vec![],
            play_audio_after_download: None,
            audio_player: Box::new(audio::cpal::CpalInterface),
            protocol_info: pinfo,
        };
        Ok(interface)
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

    async fn recv_data_and_write_it(&mut self) -> Result<()> {
        let mut framed = FramedRead::new(&mut self.tcp_stream, LengthDelimitedCodec::new());

        while let Some(frame) = framed.next().await {
            let bytes: Bytes = frame?.into();

            if protocol::is_stop_playing_message(&bytes) {
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

    pub async fn start_playing(&mut self) -> Result<()> {
        network::common::send_start_playing(&mut self.tcp_stream).await?;

        self.update_audio_header().await?;

        self.recv_data_and_write_it().await?;

        self.end_audio()?;

        network::common::send_bye_message(&mut self.tcp_stream).await?;

        network::common::expect_bye_message(&mut self.tcp_stream).await?;

        if let Some(file) = self.play_audio_after_download.as_ref() {
            self.audio_player
                .as_ref()
                .play_from_file(file, audio::file::FileFormat::Wav)?;
        }
        Ok(())
    }
}
