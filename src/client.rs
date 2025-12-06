use crate::buffer_pool::BufferPool;
use crate::config::{Config, Protocol};
use crate::measurements::{
    get_connection_info, get_system_info, get_tcp_stats, IntervalStats, MeasurementsCollector,
    TestConfig,
};
use crate::protocol::{deserialize_message, serialize_message, Message, DEFAULT_STREAM_ID};
use crate::{Error, Result};
use log::{debug, error, info};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time;
use tokio_util::sync::CancellationToken;

/// Progress event types reported during test execution.
///
/// These events allow monitoring of test progress in real-time through callbacks.
/// Events are emitted for test lifecycle stages and periodic updates.
///
/// # Examples
///
/// ```no_run
/// use rperf3::{Client, Config, ProgressEvent};
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::client("127.0.0.1".to_string(), 5201)
///     .with_duration(Duration::from_secs(10));
///
/// let client = Client::new(config)?
///     .with_callback(|event: ProgressEvent| {
///         match event {
///             ProgressEvent::TestStarted => println!("Starting..."),
///             ProgressEvent::IntervalUpdate { bits_per_second, .. } => {
///                 println!("Speed: {:.2} Mbps", bits_per_second / 1_000_000.0);
///             }
///             ProgressEvent::TestCompleted { total_bytes, .. } => {
///                 println!("Transferred {} bytes", total_bytes);
///             }
///             ProgressEvent::Error(msg) => eprintln!("Error: {}", msg),
///         }
///     });
///
/// client.run().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Test is starting.
    ///
    /// This event is emitted once at the beginning of test execution.
    TestStarted,
    /// Interval update with statistics.
    ///
    /// Emitted periodically (based on the interval configuration) with
    /// cumulative statistics for the current interval.
    ///
    /// # Fields
    ///
    /// * `interval_start` - Start time of this interval relative to test start
    /// * `interval_end` - End time of this interval relative to test start
    /// * `bytes` - Number of bytes transferred during this interval
    /// * `bits_per_second` - Throughput in bits per second for this interval
    /// * `packets` - Number of packets (UDP only)
    /// * `jitter_ms` - Jitter in milliseconds (UDP only)
    /// * `lost_packets` - Number of lost packets (UDP only)
    /// * `lost_percent` - Packet loss percentage (UDP only)
    /// * `retransmits` - Number of TCP retransmits (TCP only)
    IntervalUpdate {
        interval_start: Duration,
        interval_end: Duration,
        bytes: u64,
        bits_per_second: f64,
        packets: Option<u64>,
        jitter_ms: Option<f64>,
        lost_packets: Option<u64>,
        lost_percent: Option<f64>,
        retransmits: Option<u64>,
    },
    /// Test completed with final measurements.
    ///
    /// Emitted once at the end of a successful test with total statistics.
    ///
    /// # Fields
    ///
    /// * `total_bytes` - Total bytes transferred during the entire test
    /// * `duration` - Actual test duration
    /// * `bits_per_second` - Average throughput over the entire test
    /// * `total_packets` - Total packets sent/received (UDP only)
    /// * `jitter_ms` - Final jitter measurement in milliseconds (UDP only)
    /// * `lost_packets` - Total lost packets (UDP only)
    /// * `lost_percent` - Final packet loss percentage (UDP only)
    /// * `out_of_order` - Out-of-order packet count (UDP only)
    TestCompleted {
        total_bytes: u64,
        duration: Duration,
        bits_per_second: f64,
        total_packets: Option<u64>,
        jitter_ms: Option<f64>,
        lost_packets: Option<u64>,
        lost_percent: Option<f64>,
        out_of_order: Option<u64>,
    },
    /// Error occurred during test execution.
    ///
    /// Contains a descriptive error message. After this event, the test
    /// will typically terminate.
    Error(String),
}

/// Callback trait for receiving progress updates during test execution.
///
/// Implement this trait to receive real-time notifications about test progress.
/// The trait is automatically implemented for any function or closure with the
/// correct signature.
///
/// # Examples
///
/// ## Using a Closure
///
/// ```no_run
/// use rperf3::{Client, Config, ProgressEvent};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::client("127.0.0.1".to_string(), 5201);
/// let client = Client::new(config)?
///     .with_callback(|event| {
///         println!("Event: {:?}", event);
///     });
/// # Ok(())
/// # }
/// ```
///
/// ## Custom Implementation
///
/// ```
/// use rperf3::ProgressCallback;
/// use rperf3::ProgressEvent;
///
/// struct MyCallback;
///
/// impl ProgressCallback for MyCallback {
///     fn on_progress(&self, event: ProgressEvent) {
///         // Custom handling
///     }
/// }
/// ```
pub trait ProgressCallback: Send + Sync {
    /// Called when a progress event occurs.
    ///
    /// # Arguments
    ///
    /// * `event` - The progress event that occurred
    fn on_progress(&self, event: ProgressEvent);
}

/// Simple function-based callback
impl<F> ProgressCallback for F
where
    F: Fn(ProgressEvent) + Send + Sync,
{
    fn on_progress(&self, event: ProgressEvent) {
        self(event)
    }
}

type CallbackRef = Arc<dyn ProgressCallback>;

/// Network performance test client.
///
/// The `Client` is responsible for connecting to a server and running network
/// performance tests. It supports TCP and UDP protocols, reverse mode testing,
/// bandwidth limiting, and provides real-time progress updates through callbacks.
///
/// # Features
///
/// - **TCP and UDP**: Test both reliable (TCP) and unreliable (UDP) protocols
/// - **Reverse Mode**: Server sends data to client instead of client to server
/// - **Bandwidth Limiting**: Control send rate with configurable bandwidth targets
/// - **UDP Metrics**: Packet loss, jitter (RFC 3550), and out-of-order detection
/// - **Progress Callbacks**: Real-time updates during test execution
///
/// # Examples
///
/// ## Basic TCP Test
///
/// ```no_run
/// use rperf3::{Client, Config, Protocol};
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::client("192.168.1.100".to_string(), 5201)
///     .with_protocol(Protocol::Tcp)
///     .with_duration(Duration::from_secs(10));
///
/// let client = Client::new(config)?;
/// client.run().await?;
///
/// let measurements = client.get_measurements();
/// println!("Average throughput: {:.2} Mbps",
///          measurements.total_bits_per_second() / 1_000_000.0);
/// # Ok(())
/// # }
/// ```
///
/// ## UDP Test with Bandwidth Limit
///
/// ```no_run
/// use rperf3::{Client, Config, Protocol};
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::client("192.168.1.100".to_string(), 5201)
///     .with_protocol(Protocol::Udp)
///     .with_bandwidth(100_000_000) // 100 Mbps
///     .with_duration(Duration::from_secs(10));
///
/// let client = Client::new(config)?;
/// client.run().await?;
///
/// let measurements = client.get_measurements();
/// println!("Packets: {}, Loss: {}, Jitter: {:.3} ms",
///          measurements.total_packets,
///          measurements.lost_packets,
///          measurements.jitter_ms);
/// # Ok(())
/// # }
/// ```
///
/// ## With Progress Callback
///
/// ```no_run
/// use rperf3::{Client, Config, ProgressEvent};
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::client("127.0.0.1".to_string(), 5201);
///
/// let client = Client::new(config)?
///     .with_callback(|event: ProgressEvent| {
///         match event {
///             ProgressEvent::IntervalUpdate { bits_per_second, .. } => {
///                 println!("{:.2} Mbps", bits_per_second / 1_000_000.0);
///             }
///             _ => {}
///         }
///     });
///
/// client.run().await?;
/// # Ok(())
/// # }
/// ```
pub struct Client {
    config: Config,
    measurements: MeasurementsCollector,
    callback: Option<CallbackRef>,
    tcp_buffer_pool: Arc<BufferPool>,
    udp_buffer_pool: Arc<BufferPool>,
    cancellation_token: CancellationToken,
    stream_id: usize,
}

impl Client {
    /// Creates a new client with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The test configuration. Must have a server address set.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration doesn't have a server address set.
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::{Client, Config};
    ///
    /// let config = Config::client("127.0.0.1".to_string(), 5201);
    /// let client = Client::new(config).expect("Failed to create client");
    /// ```
    pub fn new(config: Config) -> Result<Self> {
        if config.server_addr.is_none() {
            return Err(Error::Config(
                "Server address is required for client mode".to_string(),
            ));
        }

        // Create buffer pools for TCP and UDP
        // TCP: use configured buffer size, pool up to 10 buffers per stream
        let tcp_pool_size = config.parallel * 2; // 2 buffers per stream (send + receive)
        let tcp_buffer_pool = Arc::new(BufferPool::new(config.buffer_size, tcp_pool_size));

        // UDP: fixed 65536 bytes (max UDP packet size), pool up to 10 buffers
        let udp_buffer_pool = Arc::new(BufferPool::new(65536, 10));

        Ok(Self {
            config,
            measurements: MeasurementsCollector::new(),
            callback: None,
            tcp_buffer_pool,
            udp_buffer_pool,
            cancellation_token: CancellationToken::new(),
            stream_id: DEFAULT_STREAM_ID, // Use default stream ID matching iperf3
        })
    }

    /// Attaches a progress callback to receive real-time test updates.
    ///
    /// The callback will be invoked for each progress event during test execution,
    /// including test start, interval updates, completion, and errors.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function or closure that implements `ProgressCallback`
    ///
    /// # Returns
    ///
    /// Returns `self` for method chaining.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rperf3::{Client, Config, ProgressEvent};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::client("127.0.0.1".to_string(), 5201);
    /// let client = Client::new(config)?
    ///     .with_callback(|event: ProgressEvent| {
    ///         println!("Progress: {:?}", event);
    ///     });
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_callback<C: ProgressCallback + 'static>(mut self, callback: C) -> Self {
        self.callback = Some(Arc::new(callback));
        self
    }

    /// Notify callback of progress event
    fn notify(&self, event: ProgressEvent) {
        if let Some(callback) = &self.callback {
            callback.on_progress(event);
        }
    }

    /// Returns a reference to the cancellation token.
    ///
    /// This allows external code to cancel the running test gracefully.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rperf3::{Client, Config};
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::client("127.0.0.1".to_string(), 5201);
    /// let client = Client::new(config)?;
    ///
    /// // Get cancellation token to cancel from another task
    /// let cancel_token = client.cancellation_token().clone();
    ///
    /// tokio::spawn(async move {
    ///     tokio::time::sleep(Duration::from_secs(5)).await;
    ///     cancel_token.cancel();
    /// });
    ///
    /// client.run().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    /// Runs the network performance test.
    ///
    /// This method connects to the server and executes the configured test.
    /// It will block until the test completes or an error occurs.
    ///
    /// Progress events are emitted through the callback (if set) during execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Cannot connect to the server
    /// - Network communication fails
    /// - Protocol errors occur
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rperf3::{Client, Config};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::client("127.0.0.1".to_string(), 5201);
    /// let client = Client::new(config)?;
    ///
    /// client.run().await?;
    /// println!("Test completed successfully");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(&self) -> Result<()> {
        let server_addr = self
            .config
            .server_addr
            .as_ref()
            .ok_or_else(|| Error::Config("Server address not set".to_string()))?;

        let full_addr = format!("{}:{}", server_addr, self.config.port);

        info!("Connecting to rperf3 server at {}", full_addr);

        match self.config.protocol {
            Protocol::Tcp => self.run_tcp(&full_addr).await,
            Protocol::Udp => self.run_udp(&full_addr).await,
        }
    }

    async fn run_tcp(&self, server_addr: &str) -> Result<()> {
        let mut stream = TcpStream::connect(server_addr).await?;
        info!("Connected to {}", server_addr);

        // Print iperf3-style connection info
        if !self.config.json {
            let local_addr = stream.local_addr()?;
            let remote_addr = stream.peer_addr()?;
            println!(
                "Connecting to host {}, port {}",
                remote_addr.ip(),
                remote_addr.port()
            );
            println!(
                "[{:3}] local {} port {} connected to {} port {}",
                self.stream_id,
                local_addr.ip(),
                local_addr.port(),
                remote_addr.ip(),
                remote_addr.port()
            );
        }

        // Collect connection and system information
        let connection_info = get_connection_info(&stream).ok();
        let system_info = Some(get_system_info());

        // Send setup message
        let setup = Message::setup(
            format!("{:?}", self.config.protocol),
            self.config.duration,
            self.config.bandwidth,
            self.config.buffer_size,
            self.config.parallel,
            self.config.reverse,
        );
        let setup_bytes = serialize_message(&setup)?;
        stream.write_all(&setup_bytes).await?;
        stream.flush().await?;

        // Read setup acknowledgment
        let ack_msg = deserialize_message(&mut stream).await?;
        match ack_msg {
            Message::SetupAck { port, cookie } => {
                debug!("Received setup ack: port={}, cookie={}", port, cookie);
            }
            Message::Error { message } => {
                return Err(Error::Protocol(format!("Server error: {}", message)));
            }
            _ => {
                return Err(Error::Protocol("Expected SetupAck message".to_string()));
            }
        }

        // Read start signal
        let start_msg = deserialize_message(&mut stream).await?;
        match start_msg {
            Message::Start { .. } => {
                info!("Test started");
                self.notify(ProgressEvent::TestStarted);
            }
            _ => {
                return Err(Error::Protocol("Expected Start message".to_string()));
            }
        }

        self.measurements.set_start_time(Instant::now());

        // Print iperf3-style header
        if !self.config.json {
            if self.config.reverse {
                println!("[ ID] Interval           Transfer        Bitrate            Retr");
            } else {
                println!("[ ID] Interval           Transfer        Bitrate            Retr  Cwnd");
            }
        }

        if self.config.reverse {
            // Client receives data from server
            receive_data(
                &mut stream,
                self.stream_id,
                &self.measurements,
                &self.config,
                &self.callback,
                self.tcp_buffer_pool.clone(),
                &self.cancellation_token,
            )
            .await?;
        } else {
            // Client sends data to server
            send_data(
                &mut stream,
                self.stream_id,
                &self.measurements,
                &self.config,
                &self.callback,
                self.tcp_buffer_pool.clone(),
                &self.cancellation_token,
            )
            .await?;
        }

        // Read final results - handle connection errors gracefully
        match deserialize_message(&mut stream).await {
            Ok(result_msg) => match result_msg {
                Message::Result {
                    stream_id,
                    bytes_sent,
                    bytes_received,
                    duration: _,
                    bits_per_second,
                    ..
                } => {
                    info!(
                        "Stream {}: {} bytes sent, {} bytes received, {:.2} Mbps",
                        stream_id,
                        bytes_sent,
                        bytes_received,
                        bits_per_second / 1_000_000.0
                    );
                }
                _ => {
                    debug!("Unexpected message, continuing");
                }
            },
            Err(e) => {
                debug!(
                    "Could not read result message (connection may be closed): {}",
                    e
                );
            }
        }

        // Read done signal - handle connection errors gracefully
        match deserialize_message(&mut stream).await {
            Ok(done_msg) => match done_msg {
                Message::Done => {
                    info!("Test completed");
                }
                _ => {
                    debug!("Expected Done message");
                }
            },
            Err(e) => {
                debug!(
                    "Could not read done message (connection may be closed): {}",
                    e
                );
                info!("Test completed");
            }
        }

        let final_measurements = self.measurements.get();

        // Notify callback of completion
        self.notify(ProgressEvent::TestCompleted {
            total_bytes: final_measurements.total_bytes_sent
                + final_measurements.total_bytes_received,
            duration: final_measurements.total_duration,
            bits_per_second: final_measurements.total_bits_per_second(),
            total_packets: None, // TCP doesn't track packets
            jitter_ms: None,
            lost_packets: None,
            lost_percent: None,
            out_of_order: None,
        });

        if !self.config.json {
            print_results(&final_measurements, self.stream_id, self.config.reverse);
        } else {
            // Use detailed results for JSON output
            let test_config = TestConfig {
                protocol: format!("{:?}", self.config.protocol),
                num_streams: self.config.parallel,
                blksize: self.config.buffer_size,
                omit: 0,
                duration: self.config.duration.as_secs(),
                reverse: self.config.reverse,
            };
            let detailed_results =
                self.measurements
                    .get_detailed_results(connection_info, system_info, test_config);
            let json = serde_json::to_string_pretty(&detailed_results)?;
            println!("{}", json);
        }

        Ok(())
    }

    async fn run_udp(&self, server_addr: &str) -> Result<()> {
        // For UDP, we still need a TCP control connection for setup
        // This is similar to how iperf3 works
        let mut control_stream = TcpStream::connect(server_addr).await?;

        // Send setup message via TCP
        let setup = Message::setup(
            format!("{:?}", self.config.protocol),
            self.config.duration,
            self.config.bandwidth,
            self.config.buffer_size,
            self.config.parallel,
            self.config.reverse,
        );
        let setup_bytes = serialize_message(&setup)?;
        control_stream.write_all(&setup_bytes).await?;
        control_stream.flush().await?;

        // Read setup acknowledgment
        let ack_msg = deserialize_message(&mut control_stream).await?;
        match ack_msg {
            Message::SetupAck { port, cookie } => {
                debug!("Received setup ack: port={}, cookie={}", port, cookie);
            }
            Message::Error { message } => {
                return Err(Error::Protocol(format!("Server error: {}", message)));
            }
            _ => {
                return Err(Error::Protocol("Expected SetupAck message".to_string()));
            }
        }

        // Read start signal
        let start_msg = deserialize_message(&mut control_stream).await?;
        match start_msg {
            Message::Start { .. } => {
                info!("Test started");
                self.notify(ProgressEvent::TestStarted);
            }
            _ => {
                return Err(Error::Protocol("Expected Start message".to_string()));
            }
        }

        // Now create UDP socket for data
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(server_addr).await?;

        info!("UDP client connected to {}", server_addr);

        // Print iperf3-style connection info
        if !self.config.json {
            let local_addr = socket.local_addr()?;
            let remote_addr = socket.peer_addr()?;
            println!(
                "Connecting to host {}, port {}",
                remote_addr.ip(),
                remote_addr.port()
            );
            println!(
                "[{:3}] local {} port {} connected to {} port {}",
                self.stream_id,
                local_addr.ip(),
                local_addr.port(),
                remote_addr.ip(),
                remote_addr.port()
            );
            println!("[ ID] Interval           Transfer        Bitrate            Total Datagrams");
        }

        let result = if self.config.reverse {
            // Reverse mode: Send one initialization packet to let server know our UDP port
            let init_packet = crate::udp_packet::create_packet(0, 0);
            socket.send(&init_packet).await?;

            // Receive data from server
            self.run_udp_receive(socket).await
        } else {
            // Normal mode: send data to server
            self.run_udp_send(socket).await
        };

        // Close control connection
        drop(control_stream);

        result
    }

    async fn run_udp_send(&self, socket: UdpSocket) -> Result<()> {
        let start = Instant::now();
        let mut last_interval = start;
        let mut interval_bytes = 0u64;
        let mut interval_packets = 0u64;
        let mut sequence = 0u64;

        // Calculate payload size accounting for UDP packet header
        let payload_size = if self.config.buffer_size > crate::udp_packet::UdpPacketHeader::SIZE {
            self.config.buffer_size - crate::udp_packet::UdpPacketHeader::SIZE
        } else {
            1024
        };

        // Calculate bandwidth limiting parameters
        // Strategy: Send packets without delay, but track bandwidth and sleep when needed
        let target_bytes_per_sec = self.config.bandwidth.map(|bw| bw / 8);

        let mut total_bytes_sent = 0u64;
        let mut last_bandwidth_check = start;

        while start.elapsed() < self.config.duration {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                info!("Test cancelled by user");
                break;
            }

            let packet = crate::udp_packet::create_packet_fast(sequence, payload_size);

            match socket.send(&packet).await {
                Ok(n) => {
                    self.measurements.record_bytes_sent(0, n as u64);
                    self.measurements.record_udp_packet(0);
                    interval_bytes += n as u64;
                    interval_packets += 1;
                    sequence += 1;
                    total_bytes_sent += n as u64;

                    // Bandwidth limiting: check if we're sending too fast
                    if let Some(target_bps) = target_bytes_per_sec {
                        let elapsed = last_bandwidth_check.elapsed().as_secs_f64();

                        // Check every 1ms for more accurate rate control
                        if elapsed >= 0.001 {
                            let expected_bytes = (target_bps as f64 * elapsed) as u64;
                            let bytes_sent_in_period = total_bytes_sent;

                            if bytes_sent_in_period > expected_bytes {
                                // We're sending too fast, sleep to catch up
                                let bytes_ahead = (bytes_sent_in_period - expected_bytes) as f64;
                                let sleep_time = bytes_ahead / target_bps as f64;
                                if sleep_time > 0.0001 {
                                    // Only sleep if > 0.1ms
                                    time::sleep(Duration::from_secs_f64(sleep_time)).await;
                                }
                            }

                            // Reset counters
                            last_bandwidth_check = Instant::now();
                            total_bytes_sent = 0;
                        }
                    }

                    // Report interval
                    if last_interval.elapsed() >= self.config.interval {
                        let elapsed = start.elapsed();
                        let interval_duration = last_interval.elapsed();
                        let bps = (interval_bytes as f64 * 8.0) / interval_duration.as_secs_f64();

                        let interval_start = if elapsed > interval_duration {
                            elapsed - interval_duration
                        } else {
                            Duration::ZERO
                        };

                        self.measurements.add_interval(IntervalStats {
                            start: interval_start,
                            end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: Some(interval_packets),
                        });

                        // Calculate UDP metrics for callback
                        let (lost, expected) = self.measurements.calculate_udp_loss();
                        let loss_percent = if expected > 0 {
                            (lost as f64 / expected as f64) * 100.0
                        } else {
                            0.0
                        };
                        let measurements = self.measurements.get();

                        // Notify callback
                        self.notify(ProgressEvent::IntervalUpdate {
                            interval_start,
                            interval_end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: Some(interval_packets),
                            jitter_ms: Some(measurements.jitter_ms),
                            lost_packets: Some(lost),
                            lost_percent: Some(loss_percent),
                            retransmits: None,
                        });

                        if !self.config.json {
                            // Format bytes as KBytes or MBytes
                            let (transfer_val, transfer_unit) = if interval_bytes >= 1_000_000 {
                                (interval_bytes as f64 / 1_000_000.0, "MBytes")
                            } else {
                                (interval_bytes as f64 / 1_000.0, "KBytes")
                            };

                            // Format bitrate as Mbits/sec
                            let bitrate_val = bps / 1_000_000.0;

                            println!(
                                "[{:3}]   {:4.2}-{:4.2}  sec   {:5.0} {}  {:6.2} Mbits/sec  {:4}",
                                self.stream_id,
                                interval_start.as_secs_f64(),
                                elapsed.as_secs_f64(),
                                transfer_val,
                                transfer_unit,
                                bitrate_val,
                                interval_packets
                            );
                        }
                        interval_bytes = 0;
                        interval_packets = 0;
                        last_interval = Instant::now();
                    }
                }
                Err(e) => {
                    error!("Error sending UDP packet: {}", e);
                    break;
                }
            }
        }

        self.measurements.set_duration(start.elapsed());

        let final_measurements = self.measurements.get();

        // Calculate final UDP metrics
        let (lost, expected) = self.measurements.calculate_udp_loss();
        let loss_percent = if expected > 0 {
            (lost as f64 / expected as f64) * 100.0
        } else {
            0.0
        };

        // Notify callback of completion
        self.notify(ProgressEvent::TestCompleted {
            total_bytes: final_measurements.total_bytes_sent
                + final_measurements.total_bytes_received,
            duration: final_measurements.total_duration,
            bits_per_second: final_measurements.total_bits_per_second(),
            total_packets: Some(final_measurements.total_packets),
            jitter_ms: Some(final_measurements.jitter_ms),
            lost_packets: Some(lost),
            lost_percent: Some(loss_percent),
            out_of_order: Some(final_measurements.out_of_order_packets),
        });

        if !self.config.json {
            print_results(&final_measurements, self.stream_id, self.config.reverse);
        } else {
            // Use detailed results for JSON output
            let system_info = Some(get_system_info());
            let test_config = TestConfig {
                protocol: format!("{:?}", self.config.protocol),
                num_streams: self.config.parallel,
                blksize: self.config.buffer_size,
                omit: 0,
                duration: self.config.duration.as_secs(),
                reverse: self.config.reverse,
            };
            let detailed_results = self.measurements.get_detailed_results(
                None, // UDP doesn't have connection info
                system_info,
                test_config,
            );
            let json = serde_json::to_string_pretty(&detailed_results)?;
            println!("{}", json);
        }

        Ok(())
    }

    async fn run_udp_receive(&self, socket: UdpSocket) -> Result<()> {
        let start = Instant::now();
        let mut last_interval = start;
        let mut interval_bytes = 0u64;
        let mut interval_packets = 0u64;
        let mut buffer = self.udp_buffer_pool.get();

        while start.elapsed() < self.config.duration {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                info!("Test cancelled by user");
                break;
            }

            // Set a timeout for recv to check duration periodically
            let timeout =
                tokio::time::timeout(Duration::from_millis(100), socket.recv(&mut buffer));

            match timeout.await {
                Ok(Ok(n)) => {
                    // Try to parse as UDP packet to get sequence and timestamp
                    if let Some((header, _payload)) = crate::udp_packet::parse_packet(&buffer[..n])
                    {
                        // Get current receive timestamp
                        let recv_timestamp_us = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_micros() as u64;

                        self.measurements.record_udp_packet_received(
                            header.sequence,
                            header.timestamp_us,
                            recv_timestamp_us,
                        );
                    }

                    self.measurements.record_bytes_received(0, n as u64);
                    interval_bytes += n as u64;
                    interval_packets += 1;

                    // Report interval
                    if last_interval.elapsed() >= self.config.interval {
                        let elapsed = start.elapsed();
                        let interval_duration = last_interval.elapsed();
                        let bps = (interval_bytes as f64 * 8.0) / interval_duration.as_secs_f64();

                        let interval_start = if elapsed > interval_duration {
                            elapsed - interval_duration
                        } else {
                            Duration::ZERO
                        };

                        self.measurements.add_interval(IntervalStats {
                            start: interval_start,
                            end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: Some(interval_packets),
                        });

                        // Calculate UDP metrics for callback
                        let (lost, expected) = self.measurements.calculate_udp_loss();
                        let loss_percent = if expected > 0 {
                            (lost as f64 / expected as f64) * 100.0
                        } else {
                            0.0
                        };
                        let measurements = self.measurements.get();

                        // Notify callback
                        self.notify(ProgressEvent::IntervalUpdate {
                            interval_start,
                            interval_end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: Some(interval_packets),
                            jitter_ms: Some(measurements.jitter_ms),
                            lost_packets: Some(lost),
                            lost_percent: Some(loss_percent),
                            retransmits: None,
                        });

                        if !self.config.json {
                            // Format bytes as KBytes or MBytes
                            let (transfer_val, transfer_unit) = if interval_bytes >= 1_000_000 {
                                (interval_bytes as f64 / 1_000_000.0, "MBytes")
                            } else {
                                (interval_bytes as f64 / 1_000.0, "KBytes")
                            };

                            // Format bitrate as Mbits/sec
                            let bitrate_val = bps / 1_000_000.0;

                            println!(
                                "[{:3}]   {:4.2}-{:4.2}  sec   {:5.0} {}  {:6.2} Mbits/sec  {:4}",
                                self.stream_id,
                                interval_start.as_secs_f64(),
                                elapsed.as_secs_f64(),
                                transfer_val,
                                transfer_unit,
                                bitrate_val,
                                interval_packets
                            );
                        }
                        interval_bytes = 0;
                        interval_packets = 0;
                        last_interval = Instant::now();
                    }
                }
                Ok(Err(e)) => {
                    error!("Error receiving UDP packet: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout - continue to check duration
                    continue;
                }
            }
        }

        self.measurements.set_duration(start.elapsed());

        let final_measurements = self.measurements.get();

        // Calculate final UDP metrics
        let (lost, expected) = self.measurements.calculate_udp_loss();
        let loss_percent = if expected > 0 {
            (lost as f64 / expected as f64) * 100.0
        } else {
            0.0
        };

        // Notify callback of completion
        self.notify(ProgressEvent::TestCompleted {
            total_bytes: final_measurements.total_bytes_sent
                + final_measurements.total_bytes_received,
            duration: final_measurements.total_duration,
            bits_per_second: final_measurements.total_bits_per_second(),
            total_packets: Some(final_measurements.total_packets),
            jitter_ms: Some(final_measurements.jitter_ms),
            lost_packets: Some(lost),
            lost_percent: Some(loss_percent),
            out_of_order: Some(final_measurements.out_of_order_packets),
        });

        if !self.config.json {
            print_results(&final_measurements, self.stream_id, self.config.reverse);
        } else {
            // Use detailed results for JSON output
            let system_info = Some(get_system_info());
            let test_config = TestConfig {
                protocol: format!("{:?}", self.config.protocol),
                num_streams: self.config.parallel,
                blksize: self.config.buffer_size,
                omit: 0,
                duration: self.config.duration.as_secs(),
                reverse: self.config.reverse,
            };
            let detailed_results = self.measurements.get_detailed_results(
                None, // UDP doesn't have connection info
                system_info,
                test_config,
            );
            let json = serde_json::to_string_pretty(&detailed_results)?;
            println!("{}", json);
        }

        Ok(())
    }

    /// Retrieves the measurements collected during the test.
    ///
    /// This method should be called after `run()` completes to get the final
    /// test statistics including throughput, bytes transferred, and timing information.
    ///
    /// # Returns
    ///
    /// A `Measurements` struct containing all test statistics.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rperf3::{Client, Config};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::client("127.0.0.1".to_string(), 5201);
    /// let client = Client::new(config)?;
    ///
    /// client.run().await?;
    ///
    /// let measurements = client.get_measurements();
    /// println!("Throughput: {:.2} Mbps",
    ///          measurements.total_bits_per_second() / 1_000_000.0);
    /// println!("Bytes transferred: {} sent, {} received",
    ///          measurements.total_bytes_sent,
    ///          measurements.total_bytes_received);
    ///
    /// // UDP-specific metrics
    /// if measurements.total_packets > 0 {
    ///     println!("UDP Loss: {} / {} ({:.2}%)",
    ///              measurements.lost_packets,
    ///              measurements.total_packets,
    ///              (measurements.lost_packets as f64 / measurements.total_packets as f64) * 100.0);
    ///     println!("Jitter: {:.3} ms", measurements.jitter_ms);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Returns
    ///
    /// A snapshot of test measurements including:
    /// - Total bytes sent/received (bidirectional support)
    /// - Test duration and bandwidth calculations
    /// - Per-stream statistics
    /// - Interval measurements
    /// - UDP-specific metrics: packet count, loss percentage, jitter (RFC 3550),
    ///   and out-of-order detection
    pub fn get_measurements(&self) -> crate::Measurements {
        self.measurements.get()
    }
}

async fn send_data(
    stream: &mut TcpStream,
    stream_id: usize,
    measurements: &MeasurementsCollector,
    config: &Config,
    callback: &Option<CallbackRef>,
    buffer_pool: Arc<BufferPool>,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let buffer = buffer_pool.get();
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;
    let mut last_retransmits = 0u64;

    while start.elapsed() < config.duration {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            info!("Test cancelled by user");
            break;
        }

        match stream.write(&buffer).await {
            Ok(n) => {
                measurements.record_bytes_sent(stream_id, n as u64);
                interval_bytes += n as u64;

                // Report interval
                if last_interval.elapsed() >= config.interval {
                    let elapsed = start.elapsed();
                    let interval_duration = last_interval.elapsed();
                    let bps = (interval_bytes as f64 * 8.0) / interval_duration.as_secs_f64();

                    let interval_start = if elapsed > interval_duration {
                        elapsed - interval_duration
                    } else {
                        Duration::ZERO
                    };

                    // Get TCP stats for retransmits
                    let tcp_stats = get_tcp_stats(stream).ok();
                    let current_retransmits =
                        tcp_stats.as_ref().map(|s| s.retransmits).unwrap_or(0);
                    let interval_retransmits = current_retransmits.saturating_sub(last_retransmits);
                    last_retransmits = current_retransmits;

                    measurements.add_interval(IntervalStats {
                        start: interval_start,
                        end: elapsed,
                        bytes: interval_bytes,
                        bits_per_second: bps,
                        packets: None,
                    });

                    // Notify callback
                    if let Some(cb) = callback {
                        cb.on_progress(ProgressEvent::IntervalUpdate {
                            interval_start,
                            interval_end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: None,
                            jitter_ms: None,
                            lost_packets: None,
                            lost_percent: None,
                            retransmits: if interval_retransmits > 0 {
                                Some(interval_retransmits)
                            } else {
                                None
                            },
                        });
                    }

                    if !config.json {
                        // Format bytes as GBytes or MBytes
                        let (transfer_val, transfer_unit) = if interval_bytes >= 1_000_000_000 {
                            (interval_bytes as f64 / 1_000_000_000.0, "GBytes")
                        } else {
                            (interval_bytes as f64 / 1_000_000.0, "MBytes")
                        };

                        // Format bitrate as Gbits/sec or Mbits/sec
                        let (bitrate_val, bitrate_unit) = if bps >= 1_000_000_000.0 {
                            (bps / 1_000_000_000.0, "Gbits/sec")
                        } else {
                            (bps / 1_000_000.0, "Mbits/sec")
                        };

                        // Get congestion window in KBytes
                        let cwnd_kbytes = tcp_stats
                            .as_ref()
                            .and_then(|s| s.snd_cwnd)
                            .map(|cwnd| cwnd / 1024)
                            .unwrap_or(0);

                        println!(
                            "[{:3}]   {:4.2}-{:4.2}  sec  {:6.2} {:>7}  {:6.1} {:>10}  {:4}   {:4} KBytes",
                            stream_id,
                            interval_start.as_secs_f64(),
                            elapsed.as_secs_f64(),
                            transfer_val,
                            transfer_unit,
                            bitrate_val,
                            bitrate_unit,
                            interval_retransmits,
                            cwnd_kbytes
                        );
                    }

                    interval_bytes = 0;
                    last_interval = Instant::now();
                }
            }
            Err(e) => {
                error!("Error sending data: {}", e);
                break;
            }
        }
    }

    measurements.set_duration(start.elapsed());
    stream.flush().await?;

    Ok(())
}

async fn receive_data(
    stream: &mut TcpStream,
    stream_id: usize,
    measurements: &MeasurementsCollector,
    config: &Config,
    callback: &Option<CallbackRef>,
    buffer_pool: Arc<BufferPool>,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let mut buffer = buffer_pool.get();
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;
    let mut last_retransmits = 0u64;

    while start.elapsed() < config.duration {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            info!("Test cancelled by user");
            break;
        }

        match time::timeout(Duration::from_millis(100), stream.read(&mut buffer)).await {
            Ok(Ok(0)) => {
                // Connection closed
                break;
            }
            Ok(Ok(n)) => {
                measurements.record_bytes_received(stream_id, n as u64);
                interval_bytes += n as u64;

                // Report interval
                if last_interval.elapsed() >= config.interval {
                    let elapsed = start.elapsed();
                    let interval_duration = last_interval.elapsed();
                    let bps = (interval_bytes as f64 * 8.0) / interval_duration.as_secs_f64();

                    let interval_start = if elapsed > interval_duration {
                        elapsed - interval_duration
                    } else {
                        Duration::ZERO
                    };

                    // Get TCP stats for retransmits
                    let tcp_stats = get_tcp_stats(stream).ok();
                    let current_retransmits =
                        tcp_stats.as_ref().map(|s| s.retransmits).unwrap_or(0);
                    let interval_retransmits = current_retransmits.saturating_sub(last_retransmits);
                    last_retransmits = current_retransmits;

                    measurements.add_interval(IntervalStats {
                        start: interval_start,
                        end: elapsed,
                        bytes: interval_bytes,
                        bits_per_second: bps,
                        packets: None,
                    });

                    // Notify callback
                    if let Some(cb) = callback {
                        cb.on_progress(ProgressEvent::IntervalUpdate {
                            interval_start,
                            interval_end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: None,
                            jitter_ms: None,
                            lost_packets: None,
                            lost_percent: None,
                            retransmits: if interval_retransmits > 0 {
                                Some(interval_retransmits)
                            } else {
                                None
                            },
                        });
                    }

                    if !config.json {
                        // Format bytes as GBytes or MBytes
                        let (transfer_val, transfer_unit) = if interval_bytes >= 1_000_000_000 {
                            (interval_bytes as f64 / 1_000_000_000.0, "GBytes")
                        } else {
                            (interval_bytes as f64 / 1_000_000.0, "MBytes")
                        };

                        // Format bitrate as Gbits/sec or Mbits/sec
                        let (bitrate_val, bitrate_unit) = if bps >= 1_000_000_000.0 {
                            (bps / 1_000_000_000.0, "Gbits/sec")
                        } else {
                            (bps / 1_000_000.0, "Mbits/sec")
                        };

                        println!(
                            "[{:3}]   {:4.2}-{:4.2}  sec  {:6.2} {:>7}  {:6.1} {:>10}  {:4}",
                            stream_id,
                            interval_start.as_secs_f64(),
                            elapsed.as_secs_f64(),
                            transfer_val,
                            transfer_unit,
                            bitrate_val,
                            bitrate_unit,
                            interval_retransmits
                        );
                    }

                    interval_bytes = 0;
                    last_interval = Instant::now();
                }
            }
            Ok(Err(e)) => {
                error!("Error receiving data: {}", e);
                break;
            }
            Err(_) => {
                // Timeout, check if duration expired
                if start.elapsed() >= config.duration {
                    break;
                }
            }
        }
    }

    measurements.set_duration(start.elapsed());

    Ok(())
}

fn print_results(measurements: &crate::Measurements, stream_id: usize, _reverse: bool) {
    let is_udp = measurements.total_packets > 0;

    if !is_udp {
        // TCP formatting
        println!("- - - - - - - - - - - - - - - - - - - - - - - - -");

        let duration = measurements.total_duration.as_secs_f64();

        // Print header for final summary
        println!("[ ID] Interval           Transfer        Bitrate            Retr");

        // Print sender summary
        let sent_bytes = measurements.total_bytes_sent;
        let (sent_val, sent_unit) = if sent_bytes >= 1_000_000_000 {
            (sent_bytes as f64 / 1_000_000_000.0, "GBytes")
        } else {
            (sent_bytes as f64 / 1_000_000.0, "MBytes")
        };
        let sent_bps = (sent_bytes as f64 * 8.0) / duration;
        let (sent_bitrate_val, sent_bitrate_unit) = if sent_bps >= 1_000_000_000.0 {
            (sent_bps / 1_000_000_000.0, "Gbits/sec")
        } else {
            (sent_bps / 1_000_000.0, "Mbits/sec")
        };

        println!(
            "[{:3}]   {:4.2}-{:4.2}  sec  {:6.2} {:>7}  {:6.1} {:>10}  {:4}             sender",
            stream_id,
            0.0,
            duration,
            sent_val,
            sent_unit,
            sent_bitrate_val,
            sent_bitrate_unit,
            0 // Total retransmits - would need to track cumulative
        );

        // Print receiver summary if we received data
        if measurements.total_bytes_received > 0 {
            let recv_bytes = measurements.total_bytes_received;
            let (recv_val, recv_unit) = if recv_bytes >= 1_000_000_000 {
                (recv_bytes as f64 / 1_000_000_000.0, "GBytes")
            } else {
                (recv_bytes as f64 / 1_000_000.0, "MBytes")
            };
            let recv_bps = (recv_bytes as f64 * 8.0) / duration;
            let (recv_bitrate_val, recv_bitrate_unit) = if recv_bps >= 1_000_000_000.0 {
                (recv_bps / 1_000_000_000.0, "Gbits/sec")
            } else {
                (recv_bps / 1_000_000.0, "Mbits/sec")
            };

            println!(
                "[{:3}]   {:4.2}-{:4.2}  sec  {:6.2} {:>7}  {:6.1} {:>10}                  receiver",
                stream_id, 0.0, duration, recv_val, recv_unit, recv_bitrate_val, recv_bitrate_unit
            );
        }

        println!();
    } else {
        // UDP formatting
        println!("- - - - - - - - - - - - - - - - - - - - - - - - -");

        let duration = measurements.total_duration.as_secs_f64();

        // Calculate loss statistics
        let (lost, expected) = if measurements.total_bytes_received > 0 {
            let (l, e) = measurements.calculate_udp_loss();
            (l, e)
        } else {
            (0, measurements.total_packets)
        };

        let loss_percent = if expected > 0 {
            (lost as f64 / expected as f64) * 100.0
        } else {
            0.0
        };

        // Print header for final summary
        println!(
            "[ ID] Interval           Transfer        Bitrate            Jitter    Lost/Total Datagrams"
        );

        // Print sender summary
        if measurements.total_bytes_sent > 0 {
            let sent_bytes = measurements.total_bytes_sent;
            let (sent_val, sent_unit) = if sent_bytes >= 1_000_000_000 {
                (sent_bytes as f64 / 1_000_000_000.0, "GBytes")
            } else if sent_bytes >= 1_000_000 {
                (sent_bytes as f64 / 1_000_000.0, "MBytes")
            } else {
                (sent_bytes as f64 / 1_000.0, "KBytes")
            };
            let sent_bps = (sent_bytes as f64 * 8.0) / duration;
            let (sent_bitrate_val, sent_bitrate_unit) = if sent_bps >= 1_000_000_000.0 {
                (sent_bps / 1_000_000_000.0, "Gbits/sec")
            } else {
                (sent_bps / 1_000_000.0, "Mbits/sec")
            };

            println!(
                "[{:3}]   {:4.2}-{:4.2}  sec  {:6.2} {:>7}  {:6.1} {:>10}  {:6.3} ms  {}/{} ({:.0}%)  sender",
                stream_id,
                0.0,
                duration,
                sent_val,
                sent_unit,
                sent_bitrate_val,
                sent_bitrate_unit,
                0.0, // Jitter can't be measured at sender
                lost,
                expected,
                loss_percent
            );
        }

        // Print receiver summary if we received data
        if measurements.total_bytes_received > 0 {
            let recv_bytes = measurements.total_bytes_received;
            let (recv_val, recv_unit) = if recv_bytes >= 1_000_000_000 {
                (recv_bytes as f64 / 1_000_000_000.0, "GBytes")
            } else if recv_bytes >= 1_000_000 {
                (recv_bytes as f64 / 1_000_000.0, "MBytes")
            } else {
                (recv_bytes as f64 / 1_000.0, "KBytes")
            };
            let recv_bps = (recv_bytes as f64 * 8.0) / duration;
            let (recv_bitrate_val, recv_bitrate_unit) = if recv_bps >= 1_000_000_000.0 {
                (recv_bps / 1_000_000_000.0, "Gbits/sec")
            } else {
                (recv_bps / 1_000_000.0, "Mbits/sec")
            };

            println!(
                "[{:3}]   {:4.2}-{:4.2}  sec  {:6.2} {:>7}  {:6.1} {:>10}  {:6.3} ms  {}/{} ({:.0}%)  receiver",
                stream_id,
                0.0,
                duration,
                recv_val,
                recv_unit,
                recv_bitrate_val,
                recv_bitrate_unit,
                measurements.jitter_ms,
                lost,
                expected,
                loss_percent
            );
        }

        println!();
    }
}
