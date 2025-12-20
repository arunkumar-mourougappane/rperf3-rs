use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;
use uuid::Uuid;

use crate::api::models::{StartTestRequest, StartTestResponse, TestStatusResponse};
use crate::test_manager::{spawn_test, AppState, TestConfiguration, TestHandle, TestInfo};

/// POST /api/tests/start - Start a new test
pub async fn start_test(
    State(state): State<Arc<AppState>>,
    Json(request): Json<StartTestRequest>,
) -> Result<(StatusCode, Json<StartTestResponse>), (StatusCode, String)> {
    info!(
        "Starting new test: {:?} to {}:{}",
        request.protocol, request.server, request.port
    );

    // Generate test ID
    let test_id = Uuid::new_v4();

    // Convert request to configuration
    let config: TestConfiguration = request.into();

    // Create test handle
    let handle = TestHandle::new(test_id, config.clone());

    // Add to state
    state.add_test(test_id, handle.clone());

    // Spawn test task
    let rperf_config = config.to_rperf_config();
    let sse_tx = handle.sse_tx.clone();
    let status = handle.status.clone();

    tokio::spawn(async move {
        if let Err(e) = spawn_test(rperf_config, sse_tx, status).await {
            tracing::error!("Test {} failed: {}", test_id, e);
        }
    });

    Ok((
        StatusCode::CREATED,
        Json(StartTestResponse { test_id }),
    ))
}

/// GET /api/tests/:test_id - Get test status
pub async fn get_test_status(
    Path(test_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<TestStatusResponse>, (StatusCode, String)> {
    let handle = state
        .get_test(&test_id)
        .ok_or((StatusCode::NOT_FOUND, "Test not found".to_string()))?;

    let started_at_ms = handle
        .started_at
        .elapsed()
        .as_millis()
        .saturating_sub(handle.started_at.elapsed().as_millis())
        + SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

    Ok(Json(TestStatusResponse {
        test_id,
        protocol: handle.config.protocol,
        status: handle.get_status(),
        started_at_ms,
    }))
}

/// GET /api/tests - List all tests
pub async fn list_tests(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<TestInfo>> {
    Json(state.list_tests())
}

/// DELETE /api/tests/:test_id - Cancel a running test
pub async fn cancel_test(
    Path(test_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let handle = state
        .get_test(&test_id)
        .ok_or((StatusCode::NOT_FOUND, "Test not found".to_string()))?;

    // Update status to cancelled
    use crate::test_manager::TestStatus;
    handle.set_status(TestStatus::Cancelled {
        cancelled_at: Some(std::time::Instant::now()),
    });

    // Note: We don't actually have a cancellation mechanism in the rperf3 client yet
    // This just marks it as cancelled in our state

    Ok(StatusCode::NO_CONTENT)
}
