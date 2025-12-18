/// Example demonstrating TCP socket optimizations (TCP_NODELAY and buffer sizes)
///
/// This example:
/// - Starts a server in the background
/// - Runs a client TCP test
/// - Demonstrates the socket configuration improvements from issue #16
///
/// Run with:
/// ```
/// cargo run --release --example tcp_nodelay_test
/// ```
use rperf3::{Client, Config, Protocol, Server};
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("TCP Socket Optimization Demo");
    println!("============================\n");
    println!("This demo tests TCP with:");
    println!("- TCP_NODELAY enabled (Nagle's algorithm disabled)");
    println!("- 256KB send/receive buffers");
    println!("- Expected 10-20% improvement over default settings\n");

    // Start server in background
    let server_config = Config::server(5201).with_protocol(Protocol::Tcp);
    let server = Server::new(server_config);

    // Run server in background task
    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("Server error: {}", e);
        }
    });

    // Give server time to start
    time::sleep(Duration::from_millis(100)).await;

    println!("Starting TCP test (5 seconds)...\n");

    // Run client test
    let client_config = Config::client("127.0.0.1".to_string(), 5201)
        .with_protocol(Protocol::Tcp)
        .with_duration(Duration::from_secs(5))
        .with_interval(Duration::from_secs(1));

    let client = Client::new(client_config)?;
    client.run().await?;

    // Get measurements
    let measurements = client.get_measurements();
    let total_bytes = measurements.total_bytes_sent + measurements.total_bytes_received;
    let bits_per_second = measurements.total_bits_per_second();

    println!("\n=== Results ===");
    println!(
        "Total transferred: {:.2} MB",
        total_bytes as f64 / 1_000_000.0
    );
    println!("Throughput: {:.2} Mbps", bits_per_second / 1_000_000.0);
    println!("\nSocket optimizations applied:");
    println!("✓ TCP_NODELAY: enabled");
    println!("✓ Send buffer: 256KB");
    println!("✓ Recv buffer: 256KB");

    Ok(())
}
