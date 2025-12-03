use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;

/// Transport protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
}

/// Test mode: client or server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Server,
    Client,
}

/// Configuration for rperf3 tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server mode or client mode
    pub mode: Mode,
    
    /// Protocol to use (TCP or UDP)
    pub protocol: Protocol,
    
    /// Port number to use
    pub port: u16,
    
    /// Server address (for client mode)
    pub server_addr: Option<String>,
    
    /// Bind address (for server mode)
    pub bind_addr: Option<IpAddr>,
    
    /// Test duration in seconds
    pub duration: Duration,
    
    /// Target bandwidth in bits per second (for UDP)
    pub bandwidth: Option<u64>,
    
    /// Buffer size in bytes
    pub buffer_size: usize,
    
    /// Number of parallel streams
    pub parallel: usize,
    
    /// Reverse mode (server sends, client receives)
    pub reverse: bool,
    
    /// Output in JSON format
    pub json: bool,
    
    /// Interval for periodic bandwidth reports in seconds
    pub interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Client,
            protocol: Protocol::Tcp,
            port: 5201,
            server_addr: None,
            bind_addr: None,
            duration: Duration::from_secs(10),
            bandwidth: None,
            buffer_size: 128 * 1024, // 128 KB
            parallel: 1,
            reverse: false,
            json: false,
            interval: Duration::from_secs(1),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn server(port: u16) -> Self {
        Self {
            mode: Mode::Server,
            port,
            ..Default::default()
        }
    }

    pub fn client(server_addr: String, port: u16) -> Self {
        Self {
            mode: Mode::Client,
            server_addr: Some(server_addr),
            port,
            ..Default::default()
        }
    }

    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_bandwidth(mut self, bandwidth: u64) -> Self {
        self.bandwidth = Some(bandwidth);
        self
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    pub fn with_parallel(mut self, parallel: usize) -> Self {
        self.parallel = parallel;
        self
    }

    pub fn with_reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    pub fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }
}
