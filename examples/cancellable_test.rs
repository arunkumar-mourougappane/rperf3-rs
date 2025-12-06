/// Example demonstrating cancellable/interruptible test
///
/// This example shows how tests can be cancelled gracefully mid-execution
/// using the cancellation token.
///
/// Run a server: cargo run --example cancellable_test server
/// Run a client: cargo run --example cancellable_test client
use rperf3::{Client, Config, ProgressEvent, Server};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("client");

    match mode {
        "server" => run_server().await?,
        "client" => run_client().await?,
        _ => {
            eprintln!("Usage: {} [server|client]", args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting TCP server on port 5201...");
    println!("Press Ctrl+C to stop the server gracefully.\n");

    let config = Config::server(5201);
    let server = Server::new(config);

    // Clone the cancellation token to handle CTRL+C
    let cancel_token = server.cancellation_token().clone();

    // Set up CTRL+C handler
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for CTRL+C");
        println!("\nReceived CTRL+C, shutting down server gracefully...");
        cancel_token.cancel();
    });

    server.run().await?;
    println!("Server stopped.");
    Ok(())
}

async fn run_client() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting TCP client test (30 seconds with 5-second auto-cancel)...");
    println!("Test will be automatically cancelled after 5 seconds.\n");

    let config =
        Config::client("127.0.0.1".to_string(), 5201).with_duration(Duration::from_secs(30)); // 30 second test

    let client = Client::new(config)?.with_callback(|event: ProgressEvent| match event {
        ProgressEvent::TestStarted => {
            println!("Test started");
        }
        ProgressEvent::IntervalUpdate {
            interval_end,
            bits_per_second,
            ..
        } => {
            println!(
                "[{:.1}s] {:.2} Mbps",
                interval_end.as_secs_f64(),
                bits_per_second / 1_000_000.0
            );
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
        }
        ProgressEvent::Error(msg) => {
            eprintln!("Error: {}", msg);
        }
    });

    // Clone the cancellation token
    let cancel_token = client.cancellation_token().clone();

    // Spawn a task to cancel the test after 5 seconds
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        println!("\n*** Cancelling test after 5 seconds ***\n");
        cancel_token.cancel();
    });

    // Also set up CTRL+C handler for manual cancellation
    let cancel_token_ctrl_c = client.cancellation_token().clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for CTRL+C");
        println!("\nReceived CTRL+C, cancelling test...");
        cancel_token_ctrl_c.cancel();
    });

    client.run().await?;
    println!("Client test stopped (cancelled after ~5 seconds).");
    Ok(())
}
