use rperf3::{client::Client, Config, Protocol};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Running UDP test with 50 Mbps target bandwidth...");

    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_protocol(Protocol::Udp)
        .with_duration(Duration::from_secs(10))
        .with_bandwidth(50 * 1_000_000) // 50 Mbps
        .with_buffer_size(1024);

    let client = Client::new(config)?;
    client.run().await?;

    let measurements = client.get_measurements();
    println!(
        "\nFinal Results: {:.2} Mbps",
        measurements.total_bits_per_second() / 1_000_000.0
    );

    Ok(())
}
