use crate::{
    audio::{
        file::{AudioReader, FileFormat},
        wav::WavFileRead,
    },
    network::common::expect_ok_message,
    protocol,
};
use anyhow::Result;
use bytes::Bytes;
use futures::SinkExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

async fn send_stop_playing_message(
    framed: &mut Framed<&mut TcpStream, LengthDelimitedCodec>,
) -> Result<()> {
    let stop_msg = protocol::make_stop_playing_message();
    framed.send(Bytes::from(stop_msg)).await?;
    Ok(())
}

async fn send_header(audio_reader: &mut WavFileRead, socket: &mut TcpStream) -> Result<()> {
    let mut header = protocol::AudioHeader::new();
    audio_reader.update_header(&mut header);

    let header_bytes = protocol::audio_header_to_bytes(&header);

    socket.write_all(&header_bytes).await?;
    Ok(())
}

async fn read_and_send(
    audio_reader: &mut WavFileRead,
    framed: &mut Framed<&mut TcpStream, LengthDelimitedCodec>,
) -> Result<()> {
    let mut buffer = vec![0u8; 4096];

    let mut last_buffer = false;
    while !last_buffer {
        let n = audio_reader.read(&mut buffer[..])?;
        if n == 0 {
            break;
        }

        let chunk = Bytes::copy_from_slice(&buffer[..n]);
        framed.send(chunk).await?;

        last_buffer = n < buffer.len();
    }
    Ok(())
}

async fn send_wav_file(socket: &mut TcpStream, file_path: &str) -> Result<()> {
    let mut audio_reader = WavFileRead::new();
    audio_reader.open_file(file_path)?;

    send_header(&mut audio_reader, socket).await?;

    expect_ok_message(socket).await?;

    let mut framed: Framed<&mut TcpStream, LengthDelimitedCodec> =
        Framed::new(socket, LengthDelimitedCodec::new());

    read_and_send(&mut audio_reader, &mut framed).await?;

    send_stop_playing_message(&mut framed).await?;

    Ok(())
}

pub async fn send_file(
    file_format: FileFormat,
    mut socket: &mut TcpStream,
    file: &str,
) -> Result<()> {
    match file_format {
        FileFormat::Wav => send_wav_file(&mut socket, file).await,
    }
}
