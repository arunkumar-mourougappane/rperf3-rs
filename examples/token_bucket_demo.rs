/// Example demonstrating token bucket bandwidth limiting
///
/// This example shows how the token bucket algorithm provides efficient
/// bandwidth control with minimal overhead.
///
/// Run this example with:
///   cargo run --example token_bucket_demo
use rperf3::token_bucket::TokenBucket;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("Token Bucket Bandwidth Limiting Demo");
    println!("======================================\n");

    // Simulate sending 1500-byte UDP packets at 10 Mbps
    let target_bandwidth: u64 = 10_000_000; // 10 Mbps in bits/sec
    let bytes_per_sec = target_bandwidth / 8; // Convert to bytes
    let packet_size: usize = 1500;

    println!("Configuration:");
    println!("  Target bandwidth: {} Mbps", target_bandwidth / 1_000_000);
    println!("  Packet size: {} bytes", packet_size);
    println!(
        "  Theoretical packets/sec: {}\n",
        bytes_per_sec / packet_size as u64
    );

    let mut bucket = TokenBucket::new(bytes_per_sec);
    let start = Instant::now();
    let mut total_bytes = 0u64;
    let mut packet_count = 0u64;

    // Send packets for 1 second
    while start.elapsed().as_secs_f64() < 1.0 {
        // Simulate sending a packet
        bucket.consume(packet_size).await;
        total_bytes += packet_size as u64;
        packet_count += 1;

        // Print progress every 100 packets
        if packet_count.is_multiple_of(100) {
            let elapsed = start.elapsed().as_secs_f64();
            let current_bps = (total_bytes as f64 * 8.0) / elapsed;
            let current_mbps = current_bps / 1_000_000.0;
            println!(
                "[{:5.3}s] Sent {:5} packets, {:8} bytes, {:.2} Mbps (available tokens: {})",
                elapsed,
                packet_count,
                total_bytes,
                current_mbps,
                bucket.available_tokens()
            );
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let actual_bps = (total_bytes as f64 * 8.0) / elapsed;
    let actual_mbps = actual_bps / 1_000_000.0;
    let accuracy = (actual_mbps / (target_bandwidth as f64 / 1_000_000.0)) * 100.0;

    println!("\nResults:");
    println!("  Duration: {:.3} seconds", elapsed);
    println!("  Total packets: {}", packet_count);
    println!(
        "  Total bytes: {} ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_000_000.0
    );
    println!("  Target bandwidth: {} Mbps", target_bandwidth / 1_000_000);
    println!("  Actual bandwidth: {:.2} Mbps", actual_mbps);
    println!("  Accuracy: {:.2}%", accuracy);
    println!("\n✓ Token bucket provides accurate rate limiting with integer arithmetic!");
}
