use parking_lot::Mutex;
use rperf3::{Config as RperfConfig, Measurements, ProgressEvent, Protocol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Shared application state containing all active and completed tests.
#[derive(Clone)]
pub struct AppState {
    pub tests: Arc<Mutex<HashMap<Uuid, TestHandle>>>,
}

impl AppState {
    /// Create a new empty application state.
    pub fn new() -> Self {
        Self {
            tests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a new test handle.
    pub fn add_test(&self, test_id: Uuid, handle: TestHandle) {
        self.tests.lock().insert(test_id, handle);
    }

    /// Get a test handle by ID.
    pub fn get_test(&self, test_id: &Uuid) -> Option<TestHandle> {
        self.tests.lock().get(test_id).cloned()
    }

    /// Remove a test handle.
    pub fn remove_test(&self, test_id: &Uuid) -> Option<TestHandle> {
        self.tests.lock().remove(test_id)
    }

    /// List all test IDs and their current status.
    pub fn list_tests(&self) -> Vec<TestInfo> {
        self.tests
            .lock()
            .iter()
            .map(|(id, handle)| TestInfo {
                test_id: *id,
                protocol: handle.config.protocol,
                status: handle.status.lock().clone(),
                started_at: handle.started_at,
            })
            .collect()
    }

    /// Subscribe to a test's progress events.
    pub fn subscribe_to_test(&self, test_id: &Uuid) -> Option<broadcast::Receiver<ProgressEvent>> {
        self.tests
            .lock()
            .get(test_id)
            .map(|handle| handle.sse_tx.subscribe())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle for a running or completed test.
#[derive(Clone)]
pub struct TestHandle {
    pub test_id: Uuid,
    pub config: TestConfiguration,
    pub status: Arc<Mutex<TestStatus>>,
    pub started_at: Instant,
    pub sse_tx: broadcast::Sender<ProgressEvent>,
}

impl TestHandle {
    /// Create a new test handle.
    pub fn new(test_id: Uuid, config: TestConfiguration) -> Self {
        let (sse_tx, _rx) = broadcast::channel(100); // Buffer last 100 messages

        Self {
            test_id,
            config,
            status: Arc::new(Mutex::new(TestStatus::Running)),
            started_at: Instant::now(),
            sse_tx,
        }
    }

    /// Update the test status.
    pub fn set_status(&self, status: TestStatus) {
        *self.status.lock() = status;
    }

    /// Get the current status.
    pub fn get_status(&self) -> TestStatus {
        self.status.lock().clone()
    }
}

/// Test configuration from user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfiguration {
    pub protocol: Protocol,
    pub server: String,
    pub port: u16,
    pub duration: Duration,
    #[serde(default)]
    pub bandwidth: Option<u64>,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default = "default_parallel")]
    pub parallel: usize,
}

fn default_parallel() -> usize {
    1
}

impl TestConfiguration {
    /// Convert to rperf3 Config.
    pub fn to_rperf_config(&self) -> RperfConfig {
        let mut config = RperfConfig::client(self.server.clone(), self.port)
            .with_protocol(self.protocol)
            .with_duration(self.duration)
            .with_reverse(self.reverse)
            .with_parallel(self.parallel);

        if let Some(bw) = self.bandwidth {
            config = config.with_bandwidth(bw);
        }

        config
    }
}

/// Status of a test (running, completed, or failed).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum TestStatus {
    Running,
    Completed {
        #[serde(skip_serializing_if = "Option::is_none")]
        measurements: Option<Measurements>,
        #[serde(skip)]
        completed_at: Option<Instant>,
    },
    Failed {
        error: String,
        #[serde(skip)]
        failed_at: Option<Instant>,
    },
    Cancelled {
        #[serde(skip)]
        cancelled_at: Option<Instant>,
    },
}

/// Summary information about a test for listing.
#[derive(Debug, Clone, Serialize)]
pub struct TestInfo {
    pub test_id: Uuid,
    pub protocol: Protocol,
    #[serde(flatten)]
    pub status: TestStatus,
    #[serde(skip)]
    pub started_at: Instant,
}
