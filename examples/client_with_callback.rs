use rperf3::{Client, Config, ProgressCallback, ProgressEvent, Protocol};
use std::time::Duration;

/// Custom callback that logs progress events
struct MyProgressCallback;

impl ProgressCallback for MyProgressCallback {
    fn on_progress(&self, event: ProgressEvent) {
        match event {
            ProgressEvent::TestStarted => {
                println!("🚀 Test has started!");
            }
            ProgressEvent::IntervalUpdate {
                interval_start,
                interval_end,
                bytes,
                bits_per_second,
                ..
            } => {
                println!(
                    "📊 [{:.1}-{:.1}s] {} bytes transferred @ {:.2} Mbps",
                    interval_start.as_secs_f64(),
                    interval_end.as_secs_f64(),
                    bytes,
                    bits_per_second / 1_000_000.0
                );
            }
            ProgressEvent::TestCompleted {
                total_bytes,
                duration,
                bits_per_second,
                ..
            } => {
                println!(
                    "✅ Test completed! {} bytes in {:.2}s @ {:.2} Mbps",
                    total_bytes,
                    duration.as_secs_f64(),
                    bits_per_second / 1_000_000.0
                );
            }
            ProgressEvent::Error(msg) => {
                eprintln!("❌ Error: {}", msg);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("rperf3 Client with Callback Example");
    println!("====================================\n");

    // Configure the test
    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_protocol(Protocol::Tcp)
        .with_duration(Duration::from_secs(10))
        .with_buffer_size(128 * 1024)
        .with_interval(Duration::from_secs(2)); // Report every 2 seconds

    // Create client with custom callback
    let client = Client::new(config)?.with_callback(MyProgressCallback);

    println!("Connecting to server at 127.0.0.1:5201...\n");

    // Run the test (callback will be invoked during execution)
    client.run().await?;

    // Get final measurements
    let measurements = client.get_measurements();
    println!("\n📈 Final Statistics:");
    println!("   Total bytes: {}", measurements.total_bytes_sent);
    println!(
        "   Duration: {:.2}s",
        measurements.total_duration.as_secs_f64()
    );
    println!(
        "   Average bandwidth: {:.2} Mbps",
        measurements.total_bits_per_second() / 1_000_000.0
    );

    Ok(())
}
