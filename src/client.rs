use crate::config::{Config, Protocol};
use crate::measurements::{
    get_connection_info, get_system_info, get_tcp_stats, IntervalStats, MeasurementsCollector,
    TestConfig,
};
use crate::protocol::{deserialize_message, serialize_message, Message};
use crate::{Error, Result};
use log::{debug, error, info};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time;

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
    IntervalUpdate {
        interval_start: Duration,
        interval_end: Duration,
        bytes: u64,
        bits_per_second: f64,
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
    TestCompleted {
        total_bytes: u64,
        duration: Duration,
        bits_per_second: f64,
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
/// and provides real-time progress updates through callbacks.
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

        Ok(Self {
            config,
            measurements: MeasurementsCollector::new(),
            callback: None,
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

        if self.config.reverse {
            // Client receives data from server
            receive_data(
                &mut stream,
                0,
                &self.measurements,
                &self.config,
                &self.callback,
            )
            .await?;
        } else {
            // Client sends data to server
            send_data(
                &mut stream,
                0,
                &self.measurements,
                &self.config,
                &self.callback,
            )
            .await?;
        }

        // Collect TCP stats before closing
        let _tcp_stats = get_tcp_stats(&stream).ok();

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
        if !self.config.json {
            print_results(&final_measurements);
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
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(server_addr).await?;

        info!("UDP client connected to {}", server_addr);
        self.notify(ProgressEvent::TestStarted);

        let buffer = vec![0u8; self.config.buffer_size];
        let start = Instant::now();
        let mut last_interval = start;
        let mut interval_bytes = 0u64;
        let mut interval_packets = 0u64;

        // Calculate delay between packets for bandwidth limiting
        let packet_delay = if let Some(bandwidth) = self.config.bandwidth {
            let bytes_per_sec = bandwidth / 8;
            let packets_per_sec = bytes_per_sec as f64 / self.config.buffer_size as f64;
            if packets_per_sec > 0.0 {
                Some(Duration::from_secs_f64(1.0 / packets_per_sec))
            } else {
                None
            }
        } else {
            None
        };

        while start.elapsed() < self.config.duration {
            match socket.send(&buffer).await {
                Ok(n) => {
                    self.measurements.record_bytes_sent(0, n as u64);
                    self.measurements.record_udp_packet(0);
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

                        // Notify callback
                        self.notify(ProgressEvent::IntervalUpdate {
                            interval_start,
                            interval_end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                        });

                        if !self.config.json {
                            println!(
                                "[{:4.1}-{:4.1} sec] {} bytes  {:.2} Mbps  ({} packets)",
                                interval_start.as_secs_f64(),
                                elapsed.as_secs_f64(),
                                interval_bytes,
                                bps / 1_000_000.0,
                                interval_packets
                            );
                        }
                        interval_bytes = 0;
                        interval_packets = 0;
                        last_interval = Instant::now();
                    }

                    // Bandwidth limiting
                    if let Some(delay) = packet_delay {
                        time::sleep(delay).await;
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

        // Notify callback of completion
        self.notify(ProgressEvent::TestCompleted {
            total_bytes: final_measurements.total_bytes_sent
                + final_measurements.total_bytes_received,
            duration: final_measurements.total_duration,
            bits_per_second: final_measurements.total_bits_per_second(),
        });

        if !self.config.json {
            print_results(&final_measurements);
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
    /// println!("Bytes sent: {}", measurements.total_bytes_sent);
    /// # Ok(())
    /// # }
    /// ```
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
) -> Result<()> {
    let buffer = vec![0u8; config.buffer_size];
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;

    while start.elapsed() < config.duration {
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
                        });
                    }

                    if !config.json {
                        println!(
                            "[{:4.1}-{:4.1} sec] {} bytes  {:.2} Mbps",
                            interval_start.as_secs_f64(),
                            elapsed.as_secs_f64(),
                            interval_bytes,
                            bps / 1_000_000.0
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
) -> Result<()> {
    let mut buffer = vec![0u8; config.buffer_size];
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;

    while start.elapsed() < config.duration {
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
                        });
                    }

                    if !config.json {
                        println!(
                            "[{:4.1}-{:4.1} sec] {} bytes  {:.2} Mbps",
                            interval_start.as_secs_f64(),
                            elapsed.as_secs_f64(),
                            interval_bytes,
                            bps / 1_000_000.0
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

fn print_results(measurements: &crate::Measurements) {
    println!("\n- - - - - - - - - - - - - - - - - - - - - - - - -");
    println!("Test Complete");
    println!("- - - - - - - - - - - - - - - - - - - - - - - - -");

    for (i, stream) in measurements.streams.iter().enumerate() {
        println!(
            "Stream {}: {:.2} seconds, {} bytes, {:.2} Mbps",
            i,
            stream.duration.as_secs_f64(),
            stream.bytes_sent + stream.bytes_received,
            stream.bits_per_second() / 1_000_000.0
        );
    }

    println!("- - - - - - - - - - - - - - - - - - - - - - - - -");
    println!(
        "Total: {:.2} seconds, {} bytes sent, {} bytes received",
        measurements.total_duration.as_secs_f64(),
        measurements.total_bytes_sent,
        measurements.total_bytes_received
    );
    println!(
        "Bandwidth: {:.2} Mbps",
        measurements.total_bits_per_second() / 1_000_000.0
    );
    println!("- - - - - - - - - - - - - - - - - - - - - - - - -\n");
}
