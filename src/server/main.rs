use clap::Parser;
use streamapp::audio::cpal::record_audio;
mod server_manager;

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
async fn main() -> Result<(), String> {
    let args = Args::parse();

    let path = match args.mode.as_str() {
        "rec" => {
            let duration = args.duration.unwrap_or(10);
            println!("Recording from microphone for {} seconds...", duration);
            record_audio(duration, &args.output).map_err(|e| e.to_string())?;
            println!("Recording saved to {}", &args.output);
            args.output
        }
        "file" => {
            let path = args.path.unwrap();
            println!("Starting server to stream file: {}", &path);
            path
        }
        _ => {
            return Err("Invalid mode. Use 'rec' or 'file'.".to_string());
        }
    };

    println!("Starting server...");
    dbg!(&path);
    let server = server_manager::Server::new(args.address, args.port, path);
    server.run().await;

    Ok(())
}
