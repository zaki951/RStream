use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use streamapp::audio::{cpal::CpalInterface, file::AudioRecorder};
use streamapp::server::server_manager;

#[derive(Parser, Debug)]
#[command(author, version, about = "Audio Streaming Server")]
struct Args {
    /// Mode: rec = microphone, file = read wav
    #[arg(long)]
    mode: String,

    /// Duration in seconds (for microphone)
    #[arg(long)]
    duration: Option<u64>,

    /// File path (for file mode)
    #[arg(long)]
    path: Option<String>,

    /// File output path (for microphone mode)
    #[arg(long, default_value = "/tmp/recorded.wav")]
    output: String,

    /// Server address
    /// Default is localhost
    #[arg(long, default_value = "localhost")]
    address: String,

    /// Server port
    /// Default is 8080
    #[arg(long, default_value_t = 8080)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let audio_interface = CpalInterface;
    let path = match args.mode.as_str() {
        "rec" => {
            let duration = args.duration.unwrap_or(10);
            println!("Recording from microphone for {} seconds...", duration);
            audio_interface
                .record_into_file(
                    duration,
                    &args.output,
                    streamapp::audio::file::FileFormat::Wav,
                )
                .await
                .unwrap();
            println!("Recording saved to {}", &args.output);
            args.output
        }
        "file" => {
            let path = args
                .path
                .ok_or_else(|| anyhow::anyhow!("The file path should be specified"))?;
            if !std::path::Path::new(&path).is_file() {
                return Err(anyhow::anyhow!(format!("Invalid file path {}", path)));
            }
            path
        }
        _ => {
            return Err(anyhow::anyhow!("Invalid mode. Use 'rec' or 'file'."));
        }
    };

    println!("Starting server...");

    let server = Arc::new(server_manager::Server::new(args.address, args.port, path).await);
    server.run().await;

    Ok(())
}
