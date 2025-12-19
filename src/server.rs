use crate::buffer_pool::BufferPool;
use crate::config::Config;
use crate::interval_reporter::{run_reporter_task, IntervalReport, IntervalReporter};
use crate::measurements::{get_tcp_stats, IntervalStats, MeasurementsCollector};
use crate::protocol::{deserialize_message, serialize_message, Message, DEFAULT_STREAM_ID};
use crate::{Error, Result};
use log::{debug, error, info};
use socket2::SockRef;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time;
use tokio_util::sync::CancellationToken;

/// Configure TCP socket options for optimal performance.
///
/// This function applies the following optimizations:
/// - **TCP_NODELAY**: Disables Nagle's algorithm to reduce latency
/// - **Send buffer**: Increases to 256KB for higher throughput
/// - **Receive buffer**: Increases to 256KB for higher throughput
///
/// # Arguments
///
/// * `stream` - The TCP stream to configure
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `Error` if any socket option fails to set.
///
/// # Performance Impact
///
/// Expected 10-20% improvement in TCP throughput tests with these optimizations.
fn configure_tcp_socket(stream: &TcpStream) -> Result<()> {
    // Disable Nagle's algorithm for lower latency
    stream.set_nodelay(true).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to set TCP_NODELAY: {}", e),
        ))
    })?;

    // Set larger send and receive buffers for higher throughput
    const BUFFER_SIZE: usize = 256 * 1024; // 256KB
    let sock_ref = SockRef::from(stream);

    sock_ref.set_send_buffer_size(BUFFER_SIZE).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to set send buffer size: {}", e),
        ))
    })?;

    sock_ref.set_recv_buffer_size(BUFFER_SIZE).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to set recv buffer size: {}", e),
        ))
    })?;

    debug!(
        "TCP socket configured: TCP_NODELAY=true, buffers={}KB",
        BUFFER_SIZE / 1024
    );

    Ok(())
}

/// Configure UDP socket options for optimal performance.
///
/// This function applies the following optimizations:
/// - **Send buffer**: Increases to 2MB for better burst handling
/// - **Receive buffer**: Increases to 2MB to reduce packet loss
///
/// # Arguments
///
/// * `socket` - The UDP socket to configure
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `Error` if any socket option fails to set.
///
/// # Performance Impact
///
/// Expected 10-20% improvement in UDP throughput tests with reduced packet loss.
fn configure_udp_socket(socket: &UdpSocket) -> Result<()> {
    // Set larger send and receive buffers for UDP
    const BUFFER_SIZE: usize = 2 * 1024 * 1024; // 2MB
    let sock_ref = SockRef::from(socket);

    sock_ref.set_send_buffer_size(BUFFER_SIZE).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to set UDP send buffer size: {}", e),
        ))
    })?;

    sock_ref.set_recv_buffer_size(BUFFER_SIZE).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("Failed to set UDP recv buffer size: {}", e),
        ))
    })?;

    debug!(
        "UDP socket configured: buffers={}MB",
        BUFFER_SIZE / (1024 * 1024)
    );

    Ok(())
}

/// Network performance test server.
///
/// The `Server` listens for incoming TCP control connections and handles both TCP and UDP
/// performance test requests. All tests use a TCP control channel for coordination, with
/// UDP data transfer happening on the same port. The server supports reverse mode testing,
/// bandwidth limiting, interval reporting, JSON output, and can handle multiple concurrent clients.
///
/// # Features
///
/// - **TCP Control Channel**: Always uses TCP for client coordination
/// - **TCP and UDP Data**: Handle both reliable (TCP) and unreliable (UDP) performance tests
/// - **Reverse Mode**: Send data to client for reverse throughput testing
/// - **Bandwidth Limiting**: Control send rate in reverse mode tests
/// - **Interval Reporting**: Display periodic statistics during tests
/// - **JSON Output**: Machine-readable output format for automation
/// - **UDP Metrics**: Track packet loss, jitter, and out-of-order delivery
/// - **Concurrent Clients**: Handle multiple simultaneous test connections
///
/// # Examples
///
/// ## Basic TCP Server
///
/// ```no_run
/// use rperf3::{Server, Config};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::server(5201);
/// let server = Server::new(config);
///
/// println!("Starting server on port 5201...");
/// server.run().await?;
/// # Ok(())
/// # }
/// ```
///
/// ## UDP Server with Reverse Mode
///
/// ```no_run
/// use rperf3::{Server, Config, Protocol};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::server(5201)
///     .with_protocol(Protocol::Udp)
///     .with_reverse(true); // Server will send UDP data
///
/// let server = Server::new(config);
/// server.run().await?;
/// # Ok(())
/// # }
/// ```
pub struct Server {
    config: Config,
    measurements: MeasurementsCollector,
    tcp_buffer_pool: Arc<BufferPool>,
    udp_buffer_pool: Arc<BufferPool>,
    cancellation_token: CancellationToken,
}

impl Server {
    /// Creates a new server with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The server configuration including port and protocol
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::{Server, Config};
    ///
    /// let config = Config::server(5201);
    /// let server = Server::new(config);
    /// ```
    pub fn new(config: Config) -> Self {
        // Create buffer pools for TCP and UDP
        // TCP: use configured buffer size, pool up to 10 buffers per stream
        let tcp_pool_size = config.parallel * 2; // 2 buffers per stream (send + receive)
        let tcp_buffer_pool = Arc::new(BufferPool::new(config.buffer_size, tcp_pool_size));

        // UDP: fixed 65536 bytes (max UDP packet size), pool up to 10 buffers
        let udp_buffer_pool = Arc::new(BufferPool::new(65536, 10));

        Self {
            config,
            measurements: MeasurementsCollector::new(),
            tcp_buffer_pool,
            udp_buffer_pool,
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Returns a reference to the cancellation token.
    ///
    /// This allows external code to cancel the running server gracefully.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rperf3::{Server, Config};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::server(5201);
    /// let server = Server::new(config);
    ///
    /// // Get cancellation token to stop from another task
    /// let cancel_token = server.cancellation_token().clone();
    ///
    /// tokio::spawn(async move {
    ///     // Server will be running
    /// });
    ///
    /// // Later, to stop the server:
    /// cancel_token.cancel();
    /// # Ok(())
    /// # }
    /// ```
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    /// Starts the server and begins listening for client connections.
    ///
    /// This method will run indefinitely, accepting and handling client connections.
    /// For TCP, each client connection is handled in a separate task. For UDP,
    /// the server processes incoming datagrams.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Cannot bind to the specified port
    /// - Network I/O errors occur
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rperf3::{Server, Config};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = Config::server(5201);
    /// let server = Server::new(config);
    ///
    /// println!("Server running...");
    /// server.run().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(&self) -> Result<()> {
        let bind_addr = format!(
            "{}:{}",
            self.config
                .bind_addr
                .map(|a| a.to_string())
                .unwrap_or_else(|| "0.0.0.0".to_string()),
            self.config.port
        );

        info!("Starting rperf3 server on {}", bind_addr);

        // Server always uses TCP for control connections
        // The protocol config is just for display/logging purposes
        // Both TCP and UDP tests are handled via the TCP control channel
        self.run_tcp(&bind_addr).await
    }

    async fn run_tcp(&self, bind_addr: &str) -> Result<()> {
        let listener = TcpListener::bind(bind_addr).await?;
        info!("TCP server listening on {}", bind_addr);

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                info!("Server shutting down gracefully");
                break;
            }

            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            info!("New connection from {}", addr);
                            let config = self.config.clone();
                            let measurements = self.measurements.clone();
                            let tcp_buffer_pool = self.tcp_buffer_pool.clone();
                            let udp_buffer_pool = self.udp_buffer_pool.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_tcp_client(
                                    stream,
                                    addr,
                                    config,
                                    measurements,
                                    tcp_buffer_pool,
                                    udp_buffer_pool,
                                )
                                .await
                                {
                                    error!("Error handling client {}: {}", addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Error accepting connection: {}", e);
                        }
                    }
                }
                _ = self.cancellation_token.cancelled() => {
                    info!("Server shutting down gracefully");
                    break;
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    async fn run_udp(&self, bind_addr: &str) -> Result<()> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let local_addr = socket.local_addr()?;

        // Configure UDP socket for optimal performance
        configure_udp_socket(&socket)?;

        info!("UDP server listening on {}", local_addr);

        // Use batch operations on Linux for better performance
        #[cfg(target_os = "linux")]
        return self.run_udp_batched(socket).await;

        #[cfg(not(target_os = "linux"))]
        return self.run_udp_standard(socket).await;
    }

    /// Standard UDP receive implementation (one packet per system call)
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    async fn run_udp_standard(&self, socket: UdpSocket) -> Result<()> {
        // Create async interval reporter
        let (reporter, receiver) = IntervalReporter::new();
        let reporter_task = tokio::spawn(run_reporter_task(
            receiver,
            self.config.json,
            None, // Server doesn't have callbacks
        ));

        let mut buf = self.udp_buffer_pool.get();
        let start = Instant::now();
        let mut last_interval = start;
        let mut interval_bytes = 0u64;
        let mut interval_packets = 0u64;

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                info!("Server shutting down gracefully");
                break;
            }

            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    debug!("Received {} bytes from {}", len, addr);

                    // Parse UDP packet
                    if let Some((header, _payload)) = crate::udp_packet::parse_packet(&buf[..len]) {
                        // Get current receive timestamp
                        let recv_timestamp_us = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_micros() as u64;

                        // Record packet with timing information
                        self.measurements.record_udp_packet_received(
                            header.sequence,
                            header.timestamp_us,
                            recv_timestamp_us,
                        );
                        self.measurements.record_bytes_received(0, len as u64);

                        interval_bytes += len as u64;
                        interval_packets += 1;
                    } else {
                        debug!("Received non-rperf3 UDP packet from {}", addr);
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
                            packets: interval_packets,
                        });

                        // Calculate UDP metrics
                        let (lost, expected) = self.measurements.calculate_udp_loss();
                        let loss_percent = if expected > 0 {
                            (lost as f64 / expected as f64) * 100.0
                        } else {
                            0.0
                        };
                        let measurements = self.measurements.get();

                        // Send to reporter task (async, non-blocking)
                        reporter.report(IntervalReport {
                            stream_id: DEFAULT_STREAM_ID,
                            interval_start,
                            interval_end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: Some(interval_packets),
                            jitter_ms: Some(measurements.jitter_ms),
                            lost_packets: Some(lost),
                            lost_percent: Some(loss_percent),
                            retransmits: None,
                            cwnd: None,
                        });

                        interval_bytes = 0;
                        interval_packets = 0;
                        last_interval = Instant::now();
                    }
                }
                Err(e) => {
                    error!("Error receiving UDP packet: {}", e);
                }
            }
        }

        // Signal reporter completion and wait for it to finish
        reporter.complete();
        let _ = reporter_task.await;

        Ok(())
    }

    /// Batched UDP receive implementation using recvmmsg (Linux only)
    #[allow(dead_code)]
    #[cfg(target_os = "linux")]
    async fn run_udp_batched(&self, socket: UdpSocket) -> Result<()> {
        use crate::batch_socket::UdpRecvBatch;

        // Create async interval reporter
        let (reporter, receiver) = IntervalReporter::new();
        let reporter_task = tokio::spawn(run_reporter_task(
            receiver,
            self.config.json,
            None, // Server doesn't have callbacks
        ));

        let mut batch = UdpRecvBatch::new();
        let start = Instant::now();
        let mut last_interval = start;
        let mut interval_bytes = 0u64;
        let mut interval_packets = 0u64;

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                info!("Server shutting down gracefully");
                break;
            }

            // Receive a batch of packets
            match batch.recv(&socket).await {
                Ok(count) => {
                    if count == 0 {
                        continue;
                    }

                    debug!("Received {} packets in batch", count);

                    // Process each packet in the batch
                    for i in 0..count {
                        if let Some((packet, addr)) = batch.get(i) {
                            debug!(
                                "Processing packet {} of {} bytes from {}",
                                i,
                                packet.len(),
                                addr
                            );

                            // Parse UDP packet
                            if let Some((header, _payload)) =
                                crate::udp_packet::parse_packet(packet)
                            {
                                // Get current receive timestamp
                                let recv_timestamp_us = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .expect("Time went backwards")
                                    .as_micros()
                                    as u64;

                                // Record packet with timing information
                                self.measurements.record_udp_packet_received(
                                    header.sequence,
                                    header.timestamp_us,
                                    recv_timestamp_us,
                                );
                                self.measurements
                                    .record_bytes_received(0, packet.len() as u64);

                                interval_bytes += packet.len() as u64;
                                interval_packets += 1;
                            } else {
                                debug!("Received non-rperf3 UDP packet from {}", addr);
                            }
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
                            packets: interval_packets,
                        });

                        // Calculate UDP metrics
                        let (lost, expected) = self.measurements.calculate_udp_loss();
                        let loss_percent = if expected > 0 {
                            (lost as f64 / expected as f64) * 100.0
                        } else {
                            0.0
                        };
                        let measurements = self.measurements.get();

                        // Send to reporter task (async, non-blocking)
                        reporter.report(IntervalReport {
                            stream_id: DEFAULT_STREAM_ID,
                            interval_start,
                            interval_end: elapsed,
                            bytes: interval_bytes,
                            bits_per_second: bps,
                            packets: Some(interval_packets),
                            jitter_ms: Some(measurements.jitter_ms),
                            lost_packets: Some(lost),
                            lost_percent: Some(loss_percent),
                            retransmits: None,
                            cwnd: None,
                        });

                        interval_bytes = 0;
                        interval_packets = 0;
                        last_interval = Instant::now();
                    }
                }
                Err(e) => {
                    error!("Error receiving UDP batch: {}", e);
                }
            }
        }

        // Signal reporter completion and wait for it to finish
        reporter.complete();
        let _ = reporter_task.await;

        Ok(())
    }

    /// Retrieves the current measurements collected by the server.
    ///
    /// Returns a snapshot of the statistics collected from client tests. This
    /// includes total bytes transferred, bandwidth measurements, and UDP-specific
    /// metrics like packet loss and jitter.
    ///
    /// # Returns
    ///
    /// A `Measurements` struct containing comprehensive test statistics.
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::{Server, Config};
    ///
    /// let config = Config::server(5201);
    /// let server = Server::new(config);
    ///
    /// // After tests have run
    /// let measurements = server.get_measurements();
    /// println!("Total bytes: {}", measurements.total_bytes_received);
    /// println!("Throughput: {:.2} Mbps",
    ///          measurements.total_bits_per_second() / 1_000_000.0);
    /// ```
    pub fn get_measurements(&self) -> crate::Measurements {
        self.measurements.get()
    }
}

async fn handle_tcp_client(
    mut stream: TcpStream,
    addr: SocketAddr,
    config: Config,
    measurements: MeasurementsCollector,
    tcp_buffer_pool: Arc<BufferPool>,
    udp_buffer_pool: Arc<BufferPool>,
) -> Result<()> {
    // Configure TCP socket options for optimal performance
    configure_tcp_socket(&stream)?;

    // Read setup message
    let setup_msg = deserialize_message(&mut stream).await?;

    let (protocol, duration, reverse, _parallel, bandwidth, buffer_size) = match setup_msg {
        Message::Setup {
            version: _,
            protocol,
            duration,
            reverse,
            parallel,
            bandwidth,
            buffer_size,
            ..
        } => {
            info!(
                "Client {} setup: protocol={}, duration={}s, reverse={}, parallel={}",
                addr, protocol, duration, reverse, parallel
            );
            (
                protocol,
                Duration::from_secs(duration),
                reverse,
                parallel,
                bandwidth,
                buffer_size,
            )
        }
        _ => {
            return Err(Error::Protocol("Expected Setup message".to_string()));
        }
    };

    // Check if this is UDP mode
    if protocol == "Udp" {
        // Create a config with the client's test parameters
        let mut udp_config = config.clone();
        udp_config.duration = duration;
        udp_config.reverse = reverse;
        udp_config.bandwidth = bandwidth;
        udp_config.buffer_size = buffer_size;

        // Handle UDP test via control channel
        return handle_udp_test(stream, addr, udp_config, measurements, udp_buffer_pool).await;
    }

    // Send setup acknowledgment for TCP
    let ack = Message::setup_ack(config.port, format!("{}", addr));
    let ack_bytes = serialize_message(&ack)?;
    stream.write_all(&ack_bytes).await?;
    stream.flush().await?;

    // Send start signal
    let start_msg = Message::start(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let start_bytes = serialize_message(&start_msg)?;
    stream.write_all(&start_bytes).await?;
    stream.flush().await?;

    measurements.set_start_time(Instant::now());

    if reverse {
        // Server sends data to client
        send_data(
            &mut stream,
            0,
            duration,
            bandwidth,
            &measurements,
            &config,
            tcp_buffer_pool.clone(),
        )
        .await?;
    } else {
        // Server receives data from client
        receive_data(
            &mut stream,
            0,
            duration,
            &measurements,
            &config,
            tcp_buffer_pool.clone(),
        )
        .await?;
    }

    // Send final results
    let final_measurements = measurements.get();
    if let Some(stream_stats) = final_measurements.streams.first() {
        let result_msg = Message::result(
            0,
            stream_stats.bytes_sent,
            stream_stats.bytes_received,
            final_measurements.total_duration.as_secs_f64(),
            final_measurements.total_bits_per_second(),
            None,
        );
        let result_bytes = serialize_message(&result_msg)?;
        stream.write_all(&result_bytes).await?;
        stream.flush().await?;
    }

    // Send done signal
    let done_msg = Message::done();
    let done_bytes = serialize_message(&done_msg)?;
    stream.write_all(&done_bytes).await?;
    stream.flush().await?;

    info!(
        "Test completed for {}: {:.2} Mbps",
        addr,
        final_measurements.total_bits_per_second() / 1_000_000.0
    );

    Ok(())
}

async fn handle_udp_test(
    mut control_stream: TcpStream,
    client_addr: SocketAddr,
    config: Config,
    measurements: MeasurementsCollector,
    udp_buffer_pool: Arc<BufferPool>,
) -> Result<()> {
    let duration = config.duration;
    let reverse = config.reverse;
    let bandwidth = config.bandwidth;
    let buffer_size = config.buffer_size;
    // Send setup acknowledgment
    let ack = Message::setup_ack(config.port, format!("{}", client_addr));
    let ack_bytes = serialize_message(&ack)?;
    control_stream.write_all(&ack_bytes).await?;
    control_stream.flush().await?;

    // Send start signal
    let start_msg = Message::start(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let start_bytes = serialize_message(&start_msg)?;
    control_stream.write_all(&start_bytes).await?;
    control_stream.flush().await?;

    measurements.set_start_time(Instant::now());

    if reverse {
        // Server sends UDP data to client
        send_udp_data(
            client_addr,
            duration,
            bandwidth,
            buffer_size,
            &measurements,
            &config,
            udp_buffer_pool.clone(),
        )
        .await?;
    } else {
        // Server receives UDP data from client
        receive_udp_data(duration, &measurements, &config, udp_buffer_pool.clone()).await?;
    }

    info!(
        "UDP test completed for {}: {:.2} Mbps",
        client_addr,
        measurements.get().total_bits_per_second() / 1_000_000.0
    );

    Ok(())
}

async fn send_udp_data(
    _client_tcp_addr: SocketAddr,
    duration: Duration,
    bandwidth: Option<u64>,
    buffer_size: usize,
    measurements: &MeasurementsCollector,
    config: &Config,
    buffer_pool: Arc<BufferPool>,
) -> Result<()> {
    // Note: In UDP reverse mode, only the client prints interval reports.
    // The server just tracks measurements for the final summary.

    // Bind to the server's configured port for UDP
    let bind_addr = format!("0.0.0.0:{}", config.port);
    let socket = UdpSocket::bind(&bind_addr).await?;

    // Configure UDP socket for optimal performance
    configure_udp_socket(&socket)?;

    info!("UDP server listening on port {}", config.port);

    // Wait for first packet from client to discover their UDP port
    let mut buf = buffer_pool.get();
    let (_n, client_udp_addr) = socket.recv_from(&mut buf).await?;

    info!("UDP client address discovered: {}", client_udp_addr);

    // Now connect to client's UDP address
    socket.connect(client_udp_addr).await?;

    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;
    let mut interval_packets = 0u64;
    let mut sequence = 0u64;

    // Calculate payload size accounting for UDP packet header
    let payload_size = if buffer_size > crate::udp_packet::UdpPacketHeader::SIZE {
        buffer_size - crate::udp_packet::UdpPacketHeader::SIZE
    } else {
        1024
    };

    // Bandwidth limiting
    let target_bytes_per_sec = bandwidth.map(|bw| bw / 8);
    let mut total_bytes_sent = 0u64;
    let mut last_bandwidth_check = start;

    while start.elapsed() < duration {
        let packet = crate::udp_packet::create_packet_fast(sequence, payload_size);

        match socket.send(&packet).await {
            Ok(n) => {
                measurements.record_bytes_sent(0, n as u64);
                measurements.record_udp_packet(0);
                interval_bytes += n as u64;
                interval_packets += 1;
                sequence += 1;
                total_bytes_sent += n as u64;

                // Bandwidth limiting
                if let Some(target_bps) = target_bytes_per_sec {
                    let elapsed = last_bandwidth_check.elapsed().as_secs_f64();

                    if elapsed >= 0.001 {
                        let expected_bytes = (target_bps as f64 * elapsed) as u64;
                        let bytes_sent_in_period = total_bytes_sent;

                        if bytes_sent_in_period > expected_bytes {
                            let bytes_ahead = (bytes_sent_in_period - expected_bytes) as f64;
                            let sleep_time = bytes_ahead / target_bps as f64;
                            if sleep_time > 0.0001 {
                                time::sleep(Duration::from_secs_f64(sleep_time)).await;
                            }
                        }

                        last_bandwidth_check = Instant::now();
                        total_bytes_sent = 0;
                    }
                }

                // Track interval stats internally (no printing on server side for reverse mode)
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
                        packets: interval_packets,
                    });

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

    measurements.set_duration(start.elapsed());
    Ok(())
}

async fn receive_udp_data(
    duration: Duration,
    measurements: &MeasurementsCollector,
    config: &Config,
    buffer_pool: Arc<BufferPool>,
) -> Result<()> {
    // Bind UDP socket on the server port
    let bind_addr = format!("0.0.0.0:{}", config.port);
    let socket = UdpSocket::bind(&bind_addr).await?;

    // Configure UDP socket for optimal performance
    configure_udp_socket(&socket)?;

    info!("UDP server listening for packets on port {}", config.port);

    let start = Instant::now();
    let mut buf = buffer_pool.get();

    // Receive packets until duration expires or timeout
    while start.elapsed() < duration {
        // Set a timeout so we can check elapsed time
        let remaining = duration.saturating_sub(start.elapsed());
        let timeout = remaining.min(Duration::from_millis(100));

        match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok((n, _addr))) => {
                // Parse UDP packet
                if let Some((header, _payload)) = crate::udp_packet::parse_packet(&buf[..n]) {
                    let recv_timestamp_us = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;

                    measurements.record_bytes_received(0, n as u64);
                    measurements.record_udp_packet_received(
                        header.sequence,
                        header.timestamp_us,
                        recv_timestamp_us,
                    );
                }
            }
            Ok(Err(e)) => {
                error!("Error receiving UDP packet: {}", e);
                break;
            }
            Err(_) => {
                // Timeout - continue to check if duration expired
                continue;
            }
        }
    }

    measurements.set_duration(start.elapsed());
    Ok(())
}

async fn send_data(
    stream: &mut TcpStream,
    stream_id: usize,
    duration: Duration,
    bandwidth: Option<u64>,
    measurements: &MeasurementsCollector,
    config: &Config,
    buffer_pool: Arc<BufferPool>,
) -> Result<()> {
    // Create async interval reporter
    let (reporter, receiver) = IntervalReporter::new();
    let reporter_task = tokio::spawn(run_reporter_task(
        receiver,
        config.json,
        None, // Server doesn't have callbacks
    ));

    let buffer = buffer_pool.get();
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;
    let mut last_retransmits = 0u64;

    // Bandwidth limiting
    let target_bytes_per_sec = bandwidth.map(|bw| bw / 8);
    let mut total_bytes_sent = 0u64;
    let mut last_bandwidth_check = start;

    while start.elapsed() < duration {
        match stream.write(&buffer).await {
            Ok(n) => {
                measurements.record_bytes_sent(stream_id, n as u64);
                interval_bytes += n as u64;
                total_bytes_sent += n as u64;

                // Bandwidth limiting
                if let Some(target_bps) = target_bytes_per_sec {
                    let elapsed = last_bandwidth_check.elapsed().as_secs_f64();

                    if elapsed >= 0.001 {
                        let expected_bytes = (target_bps as f64 * elapsed) as u64;
                        let bytes_sent_in_period = total_bytes_sent;

                        if bytes_sent_in_period > expected_bytes {
                            let bytes_ahead = (bytes_sent_in_period - expected_bytes) as f64;
                            let sleep_time = bytes_ahead / target_bps as f64;
                            if sleep_time > 0.0001 {
                                time::sleep(Duration::from_secs_f64(sleep_time)).await;
                            }
                        }

                        last_bandwidth_check = Instant::now();
                        total_bytes_sent = 0;
                    }
                }

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
                        packets: u64::MAX,
                    });

                    // Get congestion window for reporting
                    let cwnd_kbytes = tcp_stats
                        .as_ref()
                        .and_then(|s| s.snd_cwnd_opt())
                        .map(|cwnd| cwnd / 1024);

                    // Send to reporter task (async, non-blocking)
                    reporter.report(IntervalReport {
                        stream_id: DEFAULT_STREAM_ID,
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
                        cwnd: cwnd_kbytes,
                    });

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

    // Signal reporter completion and wait for it to finish
    reporter.complete();
    let _ = reporter_task.await;

    measurements.set_duration(start.elapsed());
    stream.flush().await?;

    Ok(())
}

async fn receive_data(
    stream: &mut TcpStream,
    stream_id: usize,
    duration: Duration,
    measurements: &MeasurementsCollector,
    config: &Config,
    buffer_pool: Arc<BufferPool>,
) -> Result<()> {
    // Create async interval reporter
    let (reporter, receiver) = IntervalReporter::new();
    let reporter_task = tokio::spawn(run_reporter_task(
        receiver,
        config.json,
        None, // Server doesn't have callbacks
    ));

    let mut buffer = buffer_pool.get();
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;
    let mut last_retransmits = 0u64;

    while start.elapsed() < duration {
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
                        packets: u64::MAX,
                    });

                    // Send to reporter task (async, non-blocking)
                    reporter.report(IntervalReport {
                        stream_id: DEFAULT_STREAM_ID,
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
                        cwnd: None,
                    });

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
                if start.elapsed() >= duration {
                    break;
                }
            }
        }
    }

    // Signal reporter completion and wait for it to finish
    reporter.complete();
    let _ = reporter_task.await;

    measurements.set_duration(start.elapsed());

    Ok(())
}
