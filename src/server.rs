use crate::buffer_pool::BufferPool;
use crate::config::{Config, Protocol};
use crate::measurements::{IntervalStats, MeasurementsCollector};
use crate::protocol::{deserialize_message, serialize_message, Message};
use crate::{Error, Result};
use log::{debug, error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time;

/// Network performance test server.
///
/// The `Server` listens for incoming client connections and handles performance
/// test requests. It supports both TCP and UDP protocols, reverse mode testing,
/// bandwidth limiting, and can handle multiple concurrent clients.
///
/// # Features
///
/// - **TCP and UDP**: Handle both reliable (TCP) and unreliable (UDP) protocol tests
/// - **Reverse Mode**: Send data to client for reverse throughput testing
/// - **Bandwidth Limiting**: Control send rate in reverse mode tests
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
        }
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

        match self.config.protocol {
            Protocol::Tcp => self.run_tcp(&bind_addr).await,
            Protocol::Udp => self.run_udp(&bind_addr).await,
        }
    }

    async fn run_tcp(&self, bind_addr: &str) -> Result<()> {
        let listener = TcpListener::bind(bind_addr).await?;
        info!("TCP server listening on {}", bind_addr);

        loop {
            match listener.accept().await {
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
    }

    async fn run_udp(&self, bind_addr: &str) -> Result<()> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("UDP server listening on {}", bind_addr);

        let mut buf = self.udp_buffer_pool.get();
        let start = Instant::now();
        let mut last_interval = start;
        let mut interval_bytes = 0u64;
        let mut interval_packets = 0u64;

        loop {
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
                            packets: Some(interval_packets),
                        });

                        // Calculate current loss statistics
                        let (lost, expected) = self.measurements.calculate_udp_loss();
                        let loss_percent = if expected > 0 {
                            (lost as f64 / expected as f64) * 100.0
                        } else {
                            0.0
                        };

                        let measurements = self.measurements.get();

                        if !self.config.json {
                            println!(
                                "[{:4.1}-{:4.1} sec] {} bytes  {:.2} Mbps  ({} packets, {:.2}% loss, {:.3} ms jitter)",
                                interval_start.as_secs_f64(),
                                elapsed.as_secs_f64(),
                                interval_bytes,
                                bps / 1_000_000.0,
                                interval_packets,
                                loss_percent,
                                measurements.jitter_ms
                            );
                        }

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
    // Bind to the server's configured port for UDP
    let bind_addr = format!("0.0.0.0:{}", config.port);
    let socket = UdpSocket::bind(&bind_addr).await?;

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
        let packet = crate::udp_packet::create_packet(sequence, payload_size);

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
                        packets: Some(interval_packets),
                    });

                    if !config.json {
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
    let buffer = buffer_pool.get();
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;

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

                    measurements.add_interval(IntervalStats {
                        start: interval_start,
                        end: elapsed,
                        bytes: interval_bytes,
                        bits_per_second: bps,
                        packets: None,
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
    let mut buffer = buffer_pool.get();
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;

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

                    measurements.add_interval(IntervalStats {
                        start: interval_start,
                        end: elapsed,
                        bytes: interval_bytes,
                        bits_per_second: bps,
                        packets: None,
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

    measurements.set_duration(start.elapsed());

    Ok(())
}
