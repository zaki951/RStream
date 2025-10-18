use crate::audio::file::FileFormat;
use crate::audio::wav::WavFileSender;
use crate::protocol::MessageType;
use crate::protocol::extract_message_type;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
pub struct Server {
    address: String,
    port: u16,
    send_file_format: FileFormat,
    file_path: String,
}

async fn send_file(file_format: FileFormat, mut socket: &mut TcpStream, file: &str) -> Result<()> {
    match file_format {
        FileFormat::Wav => {
            let fs = WavFileSender;
            fs.send_file(&mut socket, file).await?;
            Ok(())
        }
    }
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

    async fn expect_client_hello(&self, socket: &mut TcpStream) -> Result<()> {
        let mut recv_buf = [0u8; 4096];
        match socket.read(&mut recv_buf).await {
            Ok(0) => Err(anyhow::anyhow!(
                "Connection closed by the client during hello"
            )),
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
        }
    }

    async fn send_hello_response(&self, socket: &mut TcpStream) -> Result<()> {
        let server_hello_msg = crate::protocol::make_server_hello_message();
        socket
            .write_all(&server_hello_msg)
            .await
            .map_err(|e| anyhow::anyhow!("Error sending server hello: {}", e))
    }

    async fn expect_ok_message(&self, socket: &mut TcpStream) -> Result<()> {
        let mut recv_buf = [0u8; 4096];
        match socket.read(&mut recv_buf).await {
            Ok(0) => Err(anyhow::anyhow!(
                "Connection closed by the server during OK message"
            )),
            Ok(n) => {
                let recv_buf = &recv_buf[..n];
                if crate::protocol::check_ok_message(recv_buf) {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Did not receive OK message from server"))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
        }
    }

    async fn expect_message_type(&self, socket: &mut TcpStream) -> Result<MessageType> {
        let mut recv_buf = [0u8; 4096];
        match socket.read(&mut recv_buf).await {
            Ok(0) => Err(anyhow::anyhow!(
                "Connection closed by the client during message type"
            )),
            Ok(n) => extract_message_type(&recv_buf[..n]).ok_or_else(|| {
                anyhow::anyhow!("Failed to extract message type from received data")
            }),
            Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
        }
    }

    async fn send_bye_message(&self, socket: &mut TcpStream) -> Result<()> {
        let bye_msg = crate::protocol::make_bye_message();
        socket
            .write_all(&bye_msg)
            .await
            .map_err(|e| anyhow::anyhow!("Error sending BYE message: {}", e))
    }

    async fn process_client_request(&self, socket: &mut TcpStream) -> Result<()> {
        loop {
            let message_type = self.expect_message_type(socket).await?;
            match message_type {
                MessageType::Bye => return self.send_bye_message(socket).await,
                MessageType::StartPlaying => {
                    dbg!("Received START_PLAYING from client");
                    let file = self.file_path.clone();
                    send_file(self.file_format(), socket, &file).await?;
                    dbg!("Finished sending file to client");
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unexpected message type from client: {:?}",
                        message_type
                    ));
                }
            }
        }
    }

    async fn client_handshake(&self, socket: &mut TcpStream) -> Result<()> {
        // First check hello
        self.expect_client_hello(socket).await?;

        self.send_hello_response(socket).await?;

        self.expect_ok_message(socket).await?;

        Ok(())
    }

    async fn client_handler(&self, mut socket: TcpStream) -> Result<()> {
        // First check hello
        self.client_handshake(&mut socket).await?;

        self.process_client_request(&mut socket).await?;

        Ok(())
    }
    pub async fn run(self: Arc<Self>) {
        let listener = TcpListener::bind(format!("{}:{}", self.address, self.port))
            .await
            .expect("Failed to bind to address");

        println!("Server listening on {}:{}", self.address, self.port);

        loop {
            let (socket, addr) = listener
                .accept()
                .await
                .expect("Failed to accept connection");
            println!("New connection from {}", addr);

            let server = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = server.client_handler(socket).await {
                    eprintln!("Client connection error: {}", e);
                }
            });
        }
    }
}
