mod client_manager;
fn main() -> Result<(), String> {
    let client = client_manager::ClientSocket {
        address: "localhost".to_string(),
        port: 8080,
    };

    let mut handler = client.connect().expect("Failed to connect to server");

    handler
        .add_capability(client_manager::Capabilities::SaveToFile(
            "output.wav".to_string(),
        ))
        .start_playing()
}
