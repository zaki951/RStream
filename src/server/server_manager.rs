use crate::audio::file::FileFormat;
use crate::network;
use crate::protocol::MessageType;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
pub struct Server {
    send_file_format: FileFormat,
    file_path: String,
    listener: TcpListener,
}

impl Server {
    pub async fn new(address: String, port: u16, file_path: String) -> Self {
        let listener = TcpListener::bind(format!("{}:{}", address, port))
            .await
            .expect("Failed to bind to address");

        println!("Server listening on {}:{}", address, port);

        Self {
            send_file_format: FileFormat::Wav,
            file_path,
            listener,
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

    async fn send_bye_message(&self, socket: &mut TcpStream) -> Result<()> {
        let bye_msg = crate::protocol::make_bye_message();
        socket
            .write_all(&bye_msg)
            .await
            .map_err(|e| anyhow::anyhow!("Error sending BYE message: {}", e))
    }

    async fn process_client_request(&self, socket: &mut TcpStream) -> Result<()> {
        loop {
            let message_type = crate::network::common::expect_message_type(socket).await?;
            match message_type {
                MessageType::Bye => return self.send_bye_message(socket).await,
                MessageType::StartPlaying => {
                    let file = self.file_path.clone();
                    network::file::send_file(self.file_format(), socket, &file).await?;
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

    async fn client_handler(&self, mut socket: TcpStream) -> Result<()> {
        // First check hello
        network::common::handshake_from_server(&mut socket).await?;

        self.process_client_request(&mut socket).await?;

        Ok(())
    }
    pub async fn run(self: Arc<Self>) {
        loop {
            let (socket, addr) = self
                .listener
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
