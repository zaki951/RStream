use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

// ===============================================
// RSTREAM PROTOCOL v0.1 (Draft)
// ===============================================
//
// Simple TCP-based audio streaming protocol.
// The goal is to define clear message types
// for client-server communication without
// adding unnecessary complexity.
// ===============================================

const PROTOCOL_MAGIC: u32 = 0xA1B2C3D4;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Encode, Decode, PartialEq)]
pub enum MessageType {
    Hello,
    Ok,
    Bye,
    StartPlaying,
    StopPlaying,
    AudioHeader,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Encode, Decode)]

pub enum SampleFormat {
    Int,
    Float,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Encode, Decode)]

pub struct ProtocolInfo {
    version: u8,
}

impl ProtocolInfo {
    fn new() -> Self {
        const VERSION: u8 = 1;
        Self { version: VERSION }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Encode, Decode)]
pub struct AudioHeader {
    sample_rate: u32,
    channels: u8,
    bits_per_sample: u8,
    sample_format: SampleFormat,
}

impl AudioHeader {
    pub fn new() -> Self {
        Self {
            sample_rate: 0,
            channels: 0,
            bits_per_sample: 0,
            sample_format: SampleFormat::Int,
        }
    }
    pub fn get_sample_format(&self) -> SampleFormat {
        self.sample_format
    }

    pub fn get_bits_per_sample(&self) -> u8 {
        self.bits_per_sample
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn get_channels(&self) -> u8 {
        self.channels
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
}

// ===============================================
// Authentication Process
// ===============================================
//
// [client -> server]  [Magic][HELLO]
//   - Magic: 4 bytes constant used for protocol sync
//   - HELLO: u8 (0x01)
//   => Client initiates handshake
//
// [server -> client]  [HELLO][PROTOCOL INFO]
//   - HELLO: u8 (0x01)
//   - PROTOCOL INFO: variable bytes
//   => Server acknowledges and shares capabilities
//
// [client -> server]  [OK]
//   - OK: u8 (0x02)
//   => Client confirms handshake success

pub fn make_client_hello_message() -> Vec<u8> {
    let config = bincode::config::standard();

    let mut message = bincode::encode_to_vec(PROTOCOL_MAGIC, config).unwrap();
    message.push(MessageType::Hello as u8);
    message
}

fn get_hello_message_size() -> usize {
    make_client_hello_message().len()
}

pub fn check_client_hello_message(data: &[u8]) -> bool {
    if data.len() != get_hello_message_size() {
        return false;
    }

    let config = bincode::config::standard();
    let (magic, _): (u32, usize) = match bincode::decode_from_slice(&data[0..4], config) {
        Ok(result) => result,
        Err(_) => return false,
    };

    let msg_type = data[4];

    magic == PROTOCOL_MAGIC && msg_type == MessageType::Hello as u8
}

pub fn make_server_hello_message() -> Vec<u8> {
    let config = bincode::config::standard();

    let protocol_info = ProtocolInfo::new();
    let protocol_info_bytes = bincode::encode_to_vec(&protocol_info, config).unwrap();

    let mut message = bincode::encode_to_vec(MessageType::Hello, config).unwrap();
    message.extend_from_slice(&protocol_info_bytes);
    message
}

pub fn extract_protocol_info(data: &[u8]) -> Option<ProtocolInfo> {
    let config = bincode::config::standard();

    let (protocol_info, _): (ProtocolInfo, usize) =
        bincode::decode_from_slice(&data[1..], config).ok()?;

    Some(protocol_info)
}

pub fn make_ok_message() -> Vec<u8> {
    let config = bincode::config::standard();

    bincode::encode_to_vec(MessageType::Ok, config).unwrap()
}

// ===============================================
// Audio Streaming Process
// ===============================================
//
// [client -> server]  [START_PLAY]
//   - START_PLAY: u8 (0x10)
//   => Client requests to start receiving audio
//
// [server -> client]  [WAV_HEADER]
//   - WAV_HEADER: u8 (0x11)
//   - Data: fixed-size WAV header (44 bytes for PCM)
//   => Sent once before audio stream
// [client -> server]  [OK]
// [server -> client]  [AUDIO_DATA]
//   - AUDIO_DATA: u8 (0x12)
//   - Data: raw PCM samples or encoded chunk
//   => Streamed continuously until stopped

pub fn make_start_playing_message() -> Vec<u8> {
    let config = bincode::config::standard();

    bincode::encode_to_vec(MessageType::StartPlaying, config).unwrap()
}

pub fn extract_message_type(data: &[u8]) -> Option<MessageType> {
    if data.is_empty() {
        return None;
    }

    let config = bincode::config::standard();
    let (msg_type, _): (MessageType, usize) = match bincode::decode_from_slice(&data[0..1], config)
    {
        Ok(result) => result,
        Err(_) => return None,
    };

    Some(msg_type)
}

pub fn extract_wav_header(data: &[u8]) -> Option<AudioHeader> {
    let config = bincode::config::standard();

    let (header, _): (AudioHeader, usize) = bincode::decode_from_slice(&data[1..], config).ok()?;

    Some(header)
}

pub fn audio_header_to_bytes(header: &AudioHeader) -> Vec<u8> {
    let config = bincode::config::standard();

    let mut message = bincode::encode_to_vec(MessageType::AudioHeader, config).unwrap();
    let header_bytes = bincode::encode_to_vec(header, config).unwrap();
    message.extend_from_slice(&header_bytes);
    message
}

pub fn check_ok_message(data: &[u8]) -> bool {
    if data.len() != 1 {
        return false;
    }

    let config = bincode::config::standard();
    let (msg_type, _): (MessageType, usize) = match bincode::decode_from_slice(&data[0..1], config)
    {
        Ok(result) => result,
        Err(_) => return false,
    };

    msg_type == MessageType::Ok
}

// ===============================================
// End / Termination Process
// ===============================================
//
// [server -> client]  [STOP_PLAY]
//   - STOP_PLAY: u8 (0x13)
//   => Server signals end of stream
//
// [client -> server]  [BYE]
//   - BYE: u8 (0x14)
//   => Client requests connection close
//
// [server -> client]  [BYE]
//   - BYE: u8 (0x14)
//   => Server confirms disconnection
//
//

pub fn make_stop_playing_message() -> Vec<u8> {
    let config = bincode::config::standard();

    bincode::encode_to_vec(MessageType::StopPlaying, config).unwrap()
}

pub fn make_bye_message() -> Vec<u8> {
    let config = bincode::config::standard();

    bincode::encode_to_vec(MessageType::Bye, config).unwrap()
}

pub fn check_bye_message(data: &[u8]) -> bool {
    if data.len() != 1 {
        return false;
    }

    let config = bincode::config::standard();
    let (msg_type, _): (MessageType, usize) = match bincode::decode_from_slice(&data[0..1], config)
    {
        Ok(result) => result,
        Err(_) => return false,
    };

    msg_type == MessageType::Bye
}
