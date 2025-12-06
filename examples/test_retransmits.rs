/// Example demonstrating TCP retransmit tracking
///
/// This example shows how retransmits are tracked and reported during TCP tests.
/// Retransmits will only be visible on Linux systems that support TCP_INFO.
///
/// Run a server: cargo run --example test_retransmits server
/// Run a client: cargo run --example test_retransmits client
use rperf3::{Client, Config, ProgressEvent, Server};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("client");

    match mode {
        "server" => {
            println!("Starting TCP server on port 5201...");
            println!("Retransmits will be shown if they occur during the test.\n");

            let config = Config::server(5201);
            let server = Server::new(config);
            server.run().await?;
        }
        "client" => {
            println!("Starting TCP client test (10 seconds)...");
            println!("Retransmits will be tracked and displayed per interval.\n");

            let config = Config::client("127.0.0.1".to_string(), 5201)
                .with_duration(Duration::from_secs(10));

            let client = Client::new(config)?.with_callback(|event: ProgressEvent| match event {
                ProgressEvent::TestStarted => {
                    println!("Test started");
                }
                ProgressEvent::IntervalUpdate {
                    interval_start,
                    interval_end,
                    bits_per_second,
                    retransmits,
                    ..
                } => {
                    if let Some(retrans) = retransmits {
                        println!(
                            "[{:.1}-{:.1}s] {:.2} Mbps  - {} retransmits detected",
                            interval_start.as_secs_f64(),
                            interval_end.as_secs_f64(),
                            bits_per_second / 1_000_000.0,
                            retrans
                        );
                    } else {
                        println!(
                            "[{:.1}-{:.1}s] {:.2} Mbps  - no retransmits",
                            interval_start.as_secs_f64(),
                            interval_end.as_secs_f64(),
                            bits_per_second / 1_000_000.0
                        );
                    }
                }
                ProgressEvent::TestCompleted {
                    total_bytes,
                    bits_per_second,
                    ..
                } => {
                    println!("\nTest completed:");
                    println!("  Total bytes: {}", total_bytes);
                    println!(
                        "  Average throughput: {:.2} Mbps",
                        bits_per_second / 1_000_000.0
                    );
                    println!("\nNote: Retransmit tracking is only available on Linux systems.");
                }
                ProgressEvent::Error(msg) => {
                    eprintln!("Error: {}", msg);
                }
            });

            client.run().await?;
        }
        _ => {
            eprintln!("Usage: {} [server|client]", args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}
