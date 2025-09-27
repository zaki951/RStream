mod server_manager;

#[tokio::main]
async fn main() -> Result<(), String> {
    let server = server_manager::Server::new("localhost".to_string(), 8080);

    server.run().await;

    Ok(())
}
