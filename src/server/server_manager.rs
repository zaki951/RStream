use crate::audio::file::FileFormat;
use crate::audio::wav::WavFileSender;
use crate::protocol::extract_header;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

pub struct Server {
    address: String,
    port: u16,
    send_file_format: FileFormat,
    file_path: String,
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
    }
}

async fn start_play(socket: &mut TcpStream) -> Result<bool, String> {
    let mut recv_buf = [0u8; 4096];
    let number_of_attempts = 4;
    let mut attempts = 0;
    let start_playing = loop {
        if attempts >= number_of_attempts {
            println!("Client invalid or not sending header, closing connection");
            return Ok(false);
        }
        attempts += 1;
        match socket.read(&mut recv_buf).await {
            Ok(0) => return Err("Connection closed by the client".to_string()),
            Ok(_) => (),
            Err(e) => return Err(format!("Error reading from socket: {}", e)),
        };

        if let Some((header, _payload, _)) = extract_header(&recv_buf) {
            println!("Received header: {:?}", header);
            break header.is_start_playing_message() && header.is_valid_magic();
        }
    };
    Ok(start_playing)
}

impl Server {
    pub fn new(address: String, port: u16, file_path: String) -> Self {
        Self {
            address,
            port,
            send_file_format: FileFormat::Wav,
            file_path,
        }
    }
    #[allow(unused)]
    pub fn set_file_format(&mut self, format: FileFormat) -> &mut Self {
        self.send_file_format = format;
        self
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
            let file = self.file_path.clone();

            tokio::spawn(async move {
                if let Ok(start) = start_play(&mut socket).await {
                    if start {
                        if send_file(file_format, &mut socket, &file).await.is_ok() {
                            println!("Finished streaming to {}", addr);
                        }
                    }
                }
            });
        }
    }
}
