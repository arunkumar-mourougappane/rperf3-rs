use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use rperf3::ProgressEvent;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{debug, info};
use uuid::Uuid;

use crate::test_manager::AppState;

/// SSE handler for streaming test progress events
pub async fn test_sse_handler(
    Path(test_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    info!("Client subscribing to SSE for test {}", test_id);

    // Subscribe to the test's broadcast channel
    let rx = state
        .subscribe_to_test(&test_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Convert broadcast receiver to stream
    let stream = BroadcastStream::new(rx)
        .filter_map(|result| match result {
            Ok(event) => Some(Ok(convert_progress_to_sse_event(event))),
            Err(e) => {
                debug!("Broadcast stream error (likely lag): {}", e);
                None // Skip lagged messages
            }
        });

    // Create SSE response with keep-alive
    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive"),
    ))
}

/// Convert a ProgressEvent to an SSE Event
fn convert_progress_to_sse_event(event: ProgressEvent) -> Event {
    match event {
        ProgressEvent::TestStarted => Event::default()
            .event("test_started")
            .data(json!({}).to_string()),

        ProgressEvent::IntervalUpdate {
            interval_start,
            interval_end,
            bytes,
            bits_per_second,
            packets,
            jitter_ms,
            lost_packets,
            lost_percent,
            retransmits,
        } => Event::default()
            .event("interval_update")
            .data(
                json!({
                    "interval_start": interval_start.as_secs_f64(),
                    "interval_end": interval_end.as_secs_f64(),
                    "bytes": bytes,
                    "bits_per_second": bits_per_second,
                    "packets": packets,
                    "jitter_ms": jitter_ms,
                    "lost_packets": lost_packets,
                    "lost_percent": lost_percent,
                    "retransmits": retransmits,
                })
                .to_string(),
            ),

        ProgressEvent::TestCompleted {
            total_bytes,
            duration,
            bits_per_second,
            total_packets,
            jitter_ms,
            lost_packets,
            lost_percent,
            out_of_order,
        } => Event::default()
            .event("test_completed")
            .data(
                json!({
                    "total_bytes": total_bytes,
                    "duration": duration.as_secs_f64(),
                    "bits_per_second": bits_per_second,
                    "total_packets": total_packets,
                    "jitter_ms": jitter_ms,
                    "lost_packets": lost_packets,
                    "lost_percent": lost_percent,
                    "out_of_order": out_of_order,
                })
                .to_string(),
            ),

        ProgressEvent::Error(msg) => Event::default()
            .event("error")
            .data(
                json!({
                    "message": msg,
                })
                .to_string(),
            ),
    }
}
