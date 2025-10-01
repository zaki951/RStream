# Rust Audio Streaming Client/Server

This project demonstrates a **TCP-based audio streaming system** in Rust. It includes a **server** that streams audio from a WAV file or microphone, and a **client** that receives and optionally plays or saves the audio.

## Features

- Server streams audio to multiple clients over TCP.
- Client can:
    - Save received audio to a WAV file.
    - Play audio after download using the system audio device (via CPAL).

## Requirements

- Rust 1.80+
- Linux or macOS

## Usage

### Server

Record from microphone:

```bash
cargo run --bin server -- --mode rec --duration 10 --output /tmp/recorded.wav
```

Stream a WAV file:

```bash
cargo run --bin server -- --mode file --path /path/to/file.wav
```

Default host and port: localhost:8080.

### Client

Save audio to file and optionally play it:

```bash
cargo run --bin client -- --play
```

### Notes

Tested on Linux, macOS support is expected but not fully verified.

### Possible improvements
- Add live streaming support
- Improve the reliability of the protocol
- Add new features such as client-side audio selection