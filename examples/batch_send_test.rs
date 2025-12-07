//! Example demonstrating batch socket operations.
//!
//! This example shows how to use the batch socket API for improved UDP performance.
//! On Linux, this will use sendmmsg/recvmmsg for batched operations.

use rperf3::{UdpSendBatch, MAX_BATCH_SIZE};
use std::net::SocketAddr;
use std::time::Instant;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let local_addr = socket.local_addr()?;
    println!("Socket bound to: {}", local_addr);

    // Target address (loopback for testing)
    let target: SocketAddr = "127.0.0.1:5201".parse()?;

    // Create a batch
    let mut batch = UdpSendBatch::new();

    // Prepare packets
    let packet_size = 1024;
    let packet_count = 1000;

    println!(
        "\nSending {} packets of {} bytes each",
        packet_count, packet_size
    );
    println!("Batch size: {} packets per system call", MAX_BATCH_SIZE);

    // Measure batch sending
    let start = Instant::now();
    let mut total_bytes = 0;
    let mut total_packets = 0;
    let mut batch_count = 0;

    for i in 0..packet_count {
        let mut packet = vec![0u8; packet_size];
        // Add sequence number
        packet[0..4].copy_from_slice(&(i as u32).to_be_bytes());

        if !batch.add(packet, target) {
            // Batch is full, send it
            match batch.send(&socket).await {
                Ok((bytes, packets)) => {
                    total_bytes += bytes;
                    total_packets += packets;
                    batch_count += 1;
                }
                Err(e) => {
                    eprintln!("Error sending batch: {}", e);
                    break;
                }
            }
        }
    }

    // Send any remaining packets
    if !batch.is_empty() {
        match batch.send(&socket).await {
            Ok((bytes, packets)) => {
                total_bytes += bytes;
                total_packets += packets;
                batch_count += 1;
            }
            Err(e) => eprintln!("Error sending final batch: {}", e),
        }
    }

    let duration = start.elapsed();

    println!("\n=== Results ===");
    println!("Total packets sent: {}", total_packets);
    println!(
        "Total bytes sent: {} ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_000_000.0
    );
    println!("Batches sent: {}", batch_count);
    println!(
        "Avg packets per batch: {:.1}",
        total_packets as f64 / batch_count as f64
    );
    println!("Duration: {:.3} seconds", duration.as_secs_f64());
    println!(
        "Throughput: {:.2} Mbps",
        (total_bytes * 8) as f64 / duration.as_secs_f64() / 1_000_000.0
    );
    println!(
        "Packet rate: {:.0} pps",
        total_packets as f64 / duration.as_secs_f64()
    );

    #[cfg(target_os = "linux")]
    println!("\n✓ Using Linux sendmmsg for batched operations");

    #[cfg(not(target_os = "linux"))]
    println!("\n⚠ Using fallback (individual send_to calls)");

    Ok(())
}
