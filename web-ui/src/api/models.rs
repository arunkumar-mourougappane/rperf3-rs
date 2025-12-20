use crate::test_manager::{TestConfiguration, TestInfo, TestStatus};
use rperf3::Protocol;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Request to start a new test
#[derive(Debug, Clone, Deserialize)]
pub struct StartTestRequest {
    pub protocol: Protocol,
    pub server: String,
    pub port: u16,
    #[serde(deserialize_with = "deserialize_duration_secs")]
    pub duration: Duration,
    pub bandwidth: Option<u64>,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default = "default_parallel")]
    pub parallel: usize,
}

fn default_parallel() -> usize {
    1
}

fn deserialize_duration_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let secs = u64::deserialize(deserializer)?;
    Ok(Duration::from_secs(secs))
}

impl From<StartTestRequest> for TestConfiguration {
    fn from(req: StartTestRequest) -> Self {
        Self {
            protocol: req.protocol,
            server: req.server,
            port: req.port,
            duration: req.duration,
            bandwidth: req.bandwidth,
            reverse: req.reverse,
            parallel: req.parallel,
        }
    }
}

/// Response when starting a test
#[derive(Debug, Clone, Serialize)]
pub struct StartTestResponse {
    pub test_id: Uuid,
}

/// Response when querying test status
#[derive(Debug, Clone, Serialize)]
pub struct TestStatusResponse {
    pub test_id: Uuid,
    pub protocol: Protocol,
    #[serde(flatten)]
    pub status: TestStatus,
    pub started_at_ms: u128, // Milliseconds since epoch
}

/// List of all tests
#[derive(Debug, Clone, Serialize)]
pub struct TestListResponse {
    pub tests: Vec<TestInfo>,
}

/// SSE event wrapper
#[derive(Debug, Clone, Serialize)]
pub struct SseEvent {
    pub event: String,
    pub data: serde_json::Value,
}

impl SseEvent {
    pub fn new(event: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            event: event.into(),
            data,
        }
    }
}
