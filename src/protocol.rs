use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

const PROTOCOL_MAGIC: u16 = 0xA1B2;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Encode, Decode, PartialEq)]
pub enum MessageType {
    StartPlaying,
    StopPlaying,
    RawData,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Encode, Decode)]

pub enum SampleFormat {
    Int,
    Float,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Encode, Decode)]
pub struct Header {
    magic: u16,
    version: u8,
    msg_type: MessageType,
    payload_size: u16,
    sample_rate: u32,
    channels: u8,
    bits_per_sample: u8,
    sample_format: SampleFormat,
}

pub fn make_full_message(header: &Header, payload: &[u8]) -> Vec<u8> {
    let config = bincode::config::standard();

    let mut message = bincode::encode_to_vec(header, config).unwrap();
    message.extend_from_slice(payload);
    message
}
pub fn make_start_playing_message() -> Vec<u8> {
    let header = Header {
        magic: PROTOCOL_MAGIC,
        version: 1,
        msg_type: MessageType::StartPlaying,
        payload_size: 0,
        sample_rate: 0,
        channels: 0,
        bits_per_sample: 0,
        sample_format: SampleFormat::Int,
    };
    make_full_message(&header, &[])
}

pub fn extract_header(data: &[u8]) -> Option<(Header, &[u8], usize)> {
    let config = bincode::config::standard();
    let (header, bytes_read): (Header, usize) = bincode::decode_from_slice(data, config).ok()?;

    let message_len = bytes_read + header.payload_size as usize;
    if data.len() < message_len {
        return None;
    }

    let payload = &data[bytes_read..message_len];

    Some((header, payload, message_len))
}

impl Header {
    pub fn new(msg_type: MessageType) -> Self {
        Self {
            magic: PROTOCOL_MAGIC,
            version: 1,
            msg_type,
            payload_size: 0,
            sample_rate: 0,
            channels: 0,
            bits_per_sample: 0,
            sample_format: SampleFormat::Int,
        }
    }
    pub fn to_wavspec(&self) -> hound::WavSpec {
        hound::WavSpec {
            channels: self.channels as u16,
            sample_rate: self.sample_rate,
            bits_per_sample: self.bits_per_sample as u16,
            sample_format: match self.sample_format {
                SampleFormat::Float => hound::SampleFormat::Float,
                SampleFormat::Int => hound::SampleFormat::Int,
            },
        }
    }

    pub fn update_wavspec(&mut self, spec: &hound::WavSpec) {
        self.channels = spec.channels as u8;
        self.sample_rate = spec.sample_rate;
        self.bits_per_sample = spec.bits_per_sample as u8;
        self.sample_format = match spec.sample_format {
            hound::SampleFormat::Float => SampleFormat::Float,
            hound::SampleFormat::Int => SampleFormat::Int,
        };
    }

    pub fn set_payload_size(&mut self, payload_size: u16) {
        self.payload_size = payload_size
    }

    pub fn is_data_message(&self) -> bool {
        self.msg_type == MessageType::RawData
    }

    pub fn is_start_playing_message(&self) -> bool {
        self.msg_type == MessageType::StartPlaying
    }

    pub fn is_valid_magic(&self) -> bool {
        self.magic == PROTOCOL_MAGIC
    }

    pub fn bits_per_sample(&self) -> u8 {
        self.bits_per_sample
    }
}
