use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;

/// Transport protocol type for network testing.
///
/// Specifies whether to use TCP or UDP for the performance test.
///
/// # Examples
///
/// ```
/// use rperf3::{Config, Protocol};
/// use std::time::Duration;
///
/// // TCP test
/// let tcp_config = Config::client("127.0.0.1".to_string(), 5201)
///     .with_protocol(Protocol::Tcp);
///
/// // UDP test with bandwidth limit
/// let udp_config = Config::client("127.0.0.1".to_string(), 5201)
///     .with_protocol(Protocol::Udp)
///     .with_bandwidth(100_000_000); // 100 Mbps
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    /// Transmission Control Protocol - provides reliable, ordered delivery
    Tcp,
    /// User Datagram Protocol - provides best-effort delivery with lower overhead
    Udp,
}

/// Test mode: client or server.
///
/// Determines whether this instance acts as a server (listening for connections)
/// or as a client (initiating connections to a server).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Server mode - listens for incoming connections
    Server,
    /// Client mode - connects to a server and initiates tests
    Client,
}

/// Configuration for rperf3 network performance tests.
///
/// This structure holds all configuration parameters for both client and server modes.
/// Use the builder pattern methods to customize the configuration.
///
/// # Examples
///
/// ## Basic TCP Client
///
/// ```
/// use rperf3::Config;
/// use std::time::Duration;
///
/// let config = Config::client("192.168.1.100".to_string(), 5201)
///     .with_duration(Duration::from_secs(30))
///     .with_buffer_size(256 * 1024); // 256 KB buffer
/// ```
///
/// ## UDP Client with Bandwidth Limit
///
/// ```
/// use rperf3::{Config, Protocol};
/// use std::time::Duration;
///
/// let config = Config::client("192.168.1.100".to_string(), 5201)
///     .with_protocol(Protocol::Udp)
///     .with_bandwidth(100_000_000) // 100 Mbps
///     .with_duration(Duration::from_secs(10));
/// ```
///
/// ## Server Configuration
///
/// ```
/// use rperf3::Config;
///
/// let config = Config::server(5201);
/// ```
///
/// ## Reverse Mode Test
///
/// ```
/// use rperf3::Config;
/// use std::time::Duration;
///
/// // Server sends data, client receives
/// let config = Config::client("192.168.1.100".to_string(), 5201)
///     .with_reverse(true)
///     .with_duration(Duration::from_secs(10));
/// ```
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
    /// Creates a new configuration with default values.
    ///
    /// This is equivalent to calling `Config::default()`. The default configuration
    /// is set up for client mode with TCP protocol.
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    ///
    /// let config = Config::new();
    /// assert_eq!(config.port, 5201);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new server configuration.
    ///
    /// Sets up the configuration for server mode, which listens for incoming
    /// connections on the specified port.
    ///
    /// # Arguments
    ///
    /// * `port` - The port number to listen on (typically 5201)
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    ///
    /// let config = Config::server(5201);
    /// ```
    pub fn server(port: u16) -> Self {
        Self {
            mode: Mode::Server,
            port,
            ..Default::default()
        }
    }

    /// Creates a new client configuration.
    ///
    /// Sets up the configuration for client mode, which connects to a server
    /// at the specified address and port.
    ///
    /// # Arguments
    ///
    /// * `server_addr` - The IP address or hostname of the server
    /// * `port` - The port number to connect to (typically 5201)
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    ///
    /// let config = Config::client("192.168.1.100".to_string(), 5201);
    /// ```
    pub fn client(server_addr: String, port: u16) -> Self {
        Self {
            mode: Mode::Client,
            server_addr: Some(server_addr),
            port,
            ..Default::default()
        }
    }

    /// Sets the protocol to use for the test.
    ///
    /// # Arguments
    ///
    /// * `protocol` - Either `Protocol::Tcp` or `Protocol::Udp`
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::{Config, Protocol};
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_protocol(Protocol::Udp);
    /// ```
    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        self.protocol = protocol;
        self
    }

    /// Sets the test duration.
    ///
    /// # Arguments
    ///
    /// * `duration` - How long the test should run
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    /// use std::time::Duration;
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_duration(Duration::from_secs(30));
    /// ```
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Sets the target bandwidth for UDP tests.
    ///
    /// This option only applies to UDP tests. For TCP tests, it is ignored.
    ///
    /// # Arguments
    ///
    /// * `bandwidth` - Target bandwidth in bits per second
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::{Config, Protocol};
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_protocol(Protocol::Udp)
    ///     .with_bandwidth(100_000_000); // 100 Mbps
    /// ```
    pub fn with_bandwidth(mut self, bandwidth: u64) -> Self {
        self.bandwidth = Some(bandwidth);
        self
    }

    /// Sets the buffer size for data transfer.
    ///
    /// Larger buffer sizes can improve throughput but use more memory.
    ///
    /// # Arguments
    ///
    /// * `size` - Buffer size in bytes (default: 128 KB)
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_buffer_size(256 * 1024); // 256 KB
    /// ```
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Sets the number of parallel streams.
    ///
    /// Multiple parallel streams can be used to maximize throughput on
    /// high-bandwidth networks.
    ///
    /// # Arguments
    ///
    /// * `parallel` - Number of parallel streams (default: 1)
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_parallel(4);
    /// ```
    pub fn with_parallel(mut self, parallel: usize) -> Self {
        self.parallel = parallel;
        self
    }

    /// Enables or disables reverse mode.
    ///
    /// In reverse mode, the server sends data and the client receives.
    /// In normal mode, the client sends data and the server receives.
    ///
    /// # Arguments
    ///
    /// * `reverse` - `true` for reverse mode, `false` for normal mode
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_reverse(true);
    /// ```
    pub fn with_reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Enables or disables JSON output format.
    ///
    /// When enabled, results are output in machine-readable JSON format
    /// similar to iperf3.
    ///
    /// # Arguments
    ///
    /// * `json` - `true` to enable JSON output, `false` for human-readable
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_json(true);
    /// ```
    pub fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    /// Sets the interval for periodic reporting.
    ///
    /// Statistics will be reported at this interval during the test.
    ///
    /// # Arguments
    ///
    /// * `interval` - How often to report statistics (default: 1 second)
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Config;
    /// use std::time::Duration;
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201)
    ///     .with_interval(Duration::from_secs(2));
    /// ```
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }
}
