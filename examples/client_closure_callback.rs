use rperf3::{Client, Config, ProgressEvent, Protocol};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("rperf3 Client with Closure Callback Example");
    println!("===========================================\n");

    // Track progress with shared state
    let progress = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = Arc::clone(&progress);

    // Configure the test
    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_protocol(Protocol::Tcp)
        .with_duration(Duration::from_secs(5))
        .with_buffer_size(128 * 1024)
        .with_interval(Duration::from_secs(1));

    // Create client with closure callback
    let client = Client::new(config)?.with_callback(move |event: ProgressEvent| {
        match &event {
            ProgressEvent::TestStarted => {
                println!("✨ Starting test...");
            }
            ProgressEvent::IntervalUpdate {
                interval_end,
                bytes,
                bits_per_second,
                ..
            } => {
                let mbps = bits_per_second / 1_000_000.0;
                println!(
                    "⏱️  {:>5.1}s | {:>10} bytes | {:>8.2} Mbps",
                    interval_end.as_secs_f64(),
                    bytes,
                    mbps
                );

                // Store progress for later analysis
                if let Ok(mut prog) = progress_clone.lock() {
                    prog.push((*interval_end, *bytes, *bits_per_second));
                }
            }
            ProgressEvent::TestCompleted {
                total_bytes,
                duration,
                bits_per_second,
            } => {
                println!("\n🎉 Test finished!");
                println!("   {} bytes in {:.2}s", total_bytes, duration.as_secs_f64());
                println!("   Average: {:.2} Mbps", bits_per_second / 1_000_000.0);
            }
            ProgressEvent::Error(msg) => {
                eprintln!("❌ Error: {}", msg);
            }
        }
    });

    println!("Connecting to server at 127.0.0.1:5201...\n");

    // Run the test
    client.run().await?;

    // Analyze collected progress data
    if let Ok(prog) = progress.lock() {
        if !prog.is_empty() {
            println!("\n📊 Progress Analysis:");
            let avg_mbps: f64 = prog
                .iter()
                .map(|(_, _, bps)| bps / 1_000_000.0)
                .sum::<f64>()
                / prog.len() as f64;
            println!("   Intervals recorded: {}", prog.len());
            println!("   Average interval speed: {:.2} Mbps", avg_mbps);

            let max_mbps = prog
                .iter()
                .map(|(_, _, bps)| bps / 1_000_000.0)
                .fold(0.0f64, f64::max);
            let min_mbps = prog
                .iter()
                .map(|(_, _, bps)| bps / 1_000_000.0)
                .fold(f64::INFINITY, f64::min);
            println!("   Peak speed: {:.2} Mbps", max_mbps);
            println!("   Lowest speed: {:.2} Mbps", min_mbps);
        }
    }

    Ok(())
}
