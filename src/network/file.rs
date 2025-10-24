use crate::{
    audio::{
        file::{AudioReader, FileFormat},
        wav::WavFileRead,
    },
    protocol,
};
use anyhow::Result;
use bytes::Bytes;
use futures::SinkExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub async fn send_wav_file(socket: &mut TcpStream, file_path: &str) -> Result<()> {
    let mut audio_reader = WavFileRead::new();
    audio_reader.open_file(file_path)?;
    let mut buffer = vec![0u8; 4096];

    let mut header = protocol::AudioHeader::new();
    audio_reader.update_header(&mut header);

    // send header
    let header_bytes = protocol::audio_header_to_bytes(&header);

    socket.write_all(&header_bytes).await?;

    let mut framed = Framed::new(socket, LengthDelimitedCodec::new());

    loop {
        let n = audio_reader.read(&mut buffer[..])?;
        if n == 0 {
            break;
        }

        let chunk = Bytes::copy_from_slice(&buffer[..n]);
        framed.send(chunk).await?;
        if n < buffer.len() {
            break;
        }
    }

    // send stop playing message
    let stop_msg = protocol::make_stop_playing_message();
    framed.send(Bytes::from(stop_msg)).await?;

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
