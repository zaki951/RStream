use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::protocol::ProtocolInfo;

pub async fn send_hello(tcp_stream: &mut TcpStream) -> Result<()> {
    let client_hello_msg = crate::protocol::make_client_hello_message();
    tcp_stream
        .write_all(&client_hello_msg)
        .await
        .map_err(|e: std::io::Error| anyhow::anyhow!(e))?;
    Ok(())
}

pub async fn client_authenticate(tcp_stream: &mut TcpStream) -> Result<ProtocolInfo> {
    send_hello(tcp_stream).await?;
    let protocol_info = Some(expect_protocol_info(tcp_stream).await?);
    send_ok_message(tcp_stream).await?;
    protocol_info.ok_or(anyhow::anyhow!(
        "Failed to receive protocol info from server"
    ))
}

async fn send_ok_message(tcp_stream: &mut TcpStream) -> Result<()> {
    let ok_msg = crate::protocol::make_ok_message();
    tcp_stream
        .write_all(&ok_msg)
        .await
        .map_err(|e| anyhow::anyhow!("Error sending OK message: {}", e))
}

async fn expect_protocol_info(tcp_stream: &mut TcpStream) -> Result<crate::protocol::ProtocolInfo> {
    let mut recv_buf = [0u8; 4096];
    match tcp_stream.read(&mut recv_buf).await {
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
pub async fn expect_bye_message(tcp_stream: &mut TcpStream) -> Result<()> {
    let mut recv_buf = [0u8; 4096];
    match tcp_stream.read(&mut recv_buf).await {
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

pub async fn send_bye_message(tcp_stream: &mut TcpStream) -> Result<()> {
    let bye_msg = crate::protocol::make_bye_message();
    tcp_stream
        .write_all(&bye_msg)
        .await
        .map_err(|e| anyhow::anyhow!("Error sending BYE message: {}", e))
}

pub async fn send_start_playing(tcp_stream: &mut TcpStream) -> Result<()> {
    let buf = crate::protocol::make_start_playing_message();
    tcp_stream
        .write_all(&buf)
        .await
        .map_err(|e: std::io::Error| anyhow::anyhow!(e))
}

async fn expect_hello(socket: &mut TcpStream) -> Result<()> {
    let mut recv_buf = [0u8; 4096];
    match socket.read(&mut recv_buf).await {
        Ok(0) => Err(anyhow::anyhow!(
            "Connection closed by the client during hello"
        )),
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
    }
}

async fn expect_ok_message(socket: &mut TcpStream) -> Result<()> {
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

pub async fn expect_message_type(socket: &mut TcpStream) -> Result<crate::protocol::MessageType> {
    let mut recv_buf = [0u8; 4096];
    match socket.read(&mut recv_buf).await {
        Ok(0) => Err(anyhow::anyhow!(
            "Connection closed by the client during message type"
        )),
        Ok(n) => crate::protocol::extract_message_type(&recv_buf[..n])
            .ok_or_else(|| anyhow::anyhow!("Failed to extract message type from received data")),
        Err(e) => Err(anyhow::anyhow!("Error reading from socket: {}", e)),
    }
}

pub async fn handshake_from_server(socket: &mut TcpStream) -> Result<()> {
    // First check hello
    expect_hello(socket).await?;

    send_hello(socket).await?;

    expect_ok_message(socket).await?;

    Ok(())
}
