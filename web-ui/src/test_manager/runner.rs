use super::state::TestStatus;
use anyhow::Result;
use rperf3::{Client, Measurements, ProgressEvent};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::{error, info};

/// Spawn a test in a background task.
///
/// This function creates an rperf3 Client with the given configuration,
/// attaches a callback that broadcasts progress events to SSE subscribers,
/// and runs the test to completion.
///
/// # Arguments
///
/// * `config` - The rperf3 configuration
/// * `sse_tx` - Broadcast channel for sending progress events
/// * `status` - Shared status that will be updated on completion/failure
///
/// # Returns
///
/// The final Measurements on success, or an error
pub async fn spawn_test(
    config: rperf3::Config,
    sse_tx: broadcast::Sender<ProgressEvent>,
    status: Arc<parking_lot::Mutex<TestStatus>>,
) -> Result<Measurements> {
    info!("Starting rperf3 test with config: {:?}", config);

    // Create client with callback that broadcasts to SSE
    let sse_tx_clone = sse_tx.clone();
    let client = Client::new(config)?;
    let client = client.with_callback(move |event: ProgressEvent| {
        // Broadcast event to all SSE subscribers
        if let Err(e) = sse_tx_clone.send(event.clone()) {
            error!("Failed to broadcast progress event: {}", e);
        }
    });

    // Run the test
    match client.run().await {
        Ok(()) => {
            let measurements = client.get_measurements();
            info!(
                "Test completed successfully. Total bytes: {}",
                measurements.total_bytes_sent + measurements.total_bytes_received
            );

            // Send final completion event
            let _ = sse_tx.send(ProgressEvent::TestCompleted {
                total_bytes: measurements.total_bytes_sent + measurements.total_bytes_received,
                duration: measurements.total_duration,
                bits_per_second: measurements.total_bits_per_second(),
                total_packets: if measurements.total_packets > 0 {
                    Some(measurements.total_packets)
                } else {
                    None
                },
                jitter_ms: if measurements.jitter_ms > 0.0 {
                    Some(measurements.jitter_ms)
                } else {
                    None
                },
                lost_packets: if measurements.lost_packets > 0 {
                    Some(measurements.lost_packets)
                } else {
                    None
                },
                lost_percent: if measurements.total_packets > 0 {
                    Some(
                        (measurements.lost_packets as f64 / measurements.total_packets as f64)
                            * 100.0,
                    )
                } else {
                    None
                },
                out_of_order: if measurements.out_of_order_packets > 0 {
                    Some(measurements.out_of_order_packets)
                } else {
                    None
                },
            });

            // Update status
            *status.lock() = TestStatus::Completed {
                measurements: Some(measurements.clone()),
                completed_at: Some(Instant::now()),
            };

            Ok(measurements)
        }
        Err(e) => {
            error!("Test failed: {}", e);

            // Send error event
            let _ = sse_tx.send(ProgressEvent::Error(format!("Test failed: {}", e)));

            // Update status
            *status.lock() = TestStatus::Failed {
                error: format!("{}", e),
                failed_at: Some(Instant::now()),
            };

            Err(e.into())
        }
    }
}
