use rperf3::{Client, Config};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_duration(Duration::from_secs(3))
        .with_json(true);

    let client = Client::new(config)?;

    match client.run().await {
        Ok(_) => println!("\nTest completed successfully"),
        Err(e) => eprintln!("\nError: {}", e),
    }

    Ok(())
}
