use streamapp::audio::AudioReader;
use streamapp::protocol::{self, extract_header};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;

#[derive(Clone)]
enum FileFormat {
    None,
    Wav,
    // Future formats can be added here
}

async fn send_file(
    file_format: FileFormat,
    mut socket: &mut TcpStream,
    file: &str,
) -> Result<(), String> {
    match file_format {
        FileFormat::Wav => {
            let fs = WavFileSender;
            fs.send_file(&mut socket, file).await?;
            Ok(())
        }
        FileFormat::None => {
            println!("No file format specified");
            Ok(())
        }
    }
}

pub struct Server {
    address: String,
    port: u16,
    send_file_format: FileFormat,
}

struct WavFileSender;

impl WavFileSender {
    async fn send_file(&self, socket: &mut TcpStream, file_path: &str) -> Result<(), String> {
        let mut audio_reader = streamapp::audio::WavFileRead::new();
        audio_reader.open_file(file_path)?;
        let mut buffer = vec![0u8; 4096];

        let mut header = protocol::Header::new(protocol::MessageType::RawData);
        audio_reader.update_header(&mut header);
        loop {
            let n = audio_reader.read(&mut buffer[..]);
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

impl Server {
    pub fn new(address: String, port: u16) -> Self {
        Self {
            address,
            port,
            send_file_format: FileFormat::Wav,
        }
    }
    fn file_format(&self) -> FileFormat {
        self.send_file_format.clone()
    }

    pub async fn run(&self) {
        let listener = TcpListener::bind(format!("{}:{}", self.address, self.port))
            .await
            .expect("Failed to bind to address");

        println!("Server listening on {}:{}", self.address, self.port);

        loop {
            let (mut socket, addr) = listener
                .accept()
                .await
                .expect("Failed to accept connection");
            println!("New connection from {}", addr);
            let file_format = self.file_format();
            tokio::spawn(async move {
                let mut recv_buf = Vec::new();
                let mut tmp_buf = [0u8; 1024];

                let start_playing = loop {
                    let n = match socket.read(&mut tmp_buf).await {
                        Ok(0) => return println!("Connection closed by {}", addr),
                        Ok(n) => n,
                        Err(e) => return println!("Error reading from socket: {}", e),
                    };

                    recv_buf.extend_from_slice(&tmp_buf[..n]);

                    if let Some((header, _payload, _)) = extract_header(&recv_buf) {
                        println!("Received header: {:?}", header);
                        break header.is_start_playing_message();
                    }
                };

                if start_playing {
                    if let Err(e) = send_file(file_format, &mut socket, "/tmp/input.wav").await {
                        println!("Error sending WAV: {}", e);
                    }
                    println!("Finished streaming to {}", addr);
                }
            });
        }
    }
}
