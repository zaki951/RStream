use std::io::{Read, Write};

use streamapp::audio::{AudioWriter, WavFileWrite};
use streamapp::protocol::extract_header;

// Interface to interact with the server
pub struct ClientInterface {
    tcp_stream: std::net::TcpStream,
    audio_capabilities: Vec<Box<dyn AudioWriter>>,
}

// Manage the tcp connection to the server
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
                todo!()
            }
        }
        self
    }

    fn update_audio_capabilities(&mut self, header: &streamapp::protocol::Header) {
        for capability in &mut self.audio_capabilities {
            capability.update_format(header);
        }
    }
    fn write_audio_data(&mut self, data: &[u8]) {
        for capability in &mut self.audio_capabilities {
            capability.write(data);
        }
    }

    fn end_audio(&mut self) {
        for capability in &mut self.audio_capabilities {
            capability.finalize();
        }
    }

    pub fn start_playing(&mut self) -> Result<(), String> {
        let buf = streamapp::protocol::make_start_playing_message(0, 0, 0);
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
                    self.end_audio();
                    return Ok(());
                }

                self.write_audio_data(payload);

                recv_buf.drain(..message_len);
            }
        }
        println!("Connection closed by server");
        self.end_audio();
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
        };
        Ok(interface)
    }
}
