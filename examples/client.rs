use rperf3::{client::Client, Config, Protocol};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Connecting to rperf3 server at 127.0.0.1:5201...");

    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_protocol(Protocol::Tcp)
        .with_duration(Duration::from_secs(10))
        .with_buffer_size(128 * 1024);

    let client = Client::new(config)?;
    client.run().await?;

    let measurements = client.get_measurements();
    println!(
        "\nFinal Results: {:.2} Mbps",
        measurements.total_bits_per_second() / 1_000_000.0
    );

    Ok(())
}
