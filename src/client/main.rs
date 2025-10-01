use clap::Parser;
use streamapp::client::client_manager;

#[derive(Parser, Debug)]
#[command(author, version, about = "Audio Streaming Client")]
struct Args {
    /// File output path (for saving received audio)
    #[arg(long, default_value = "/tmp/client_output.wav")]
    output: String,

    /// Server address
    #[arg(long, default_value = "localhost")]
    address: String,

    /// Server port
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// Play audio after download
    /// Default is false
    #[arg(long, default_value_t = false)]
    play: bool,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();
    let client = client_manager::ClientSocket {
        address: args.address,
        port: args.port,
    };

    let mut handler = client.connect().await.expect("Failed to connect to server");

    if args.play {
        handler.add_capability(client_manager::Capabilities::PlayFileAfterDownload(
            args.output.clone(),
        ));
    }

    handler
        .add_capability(client_manager::Capabilities::SaveToFile(args.output))
        .start_playing()
        .await
}
