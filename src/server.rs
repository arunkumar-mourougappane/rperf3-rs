use crate::config::{Config, Protocol};
use crate::measurements::{IntervalStats, MeasurementsCollector};
use crate::protocol::{deserialize_message, serialize_message, Message};
use crate::{Error, Result};
use log::{debug, error, info};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time;

/// Network performance test server.
///
/// The `Server` listens for incoming client connections and handles performance
/// test requests. It supports both TCP and UDP protocols and can handle multiple
/// concurrent clients.
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
/// ## UDP Server on Custom Port
///
/// ```no_run
/// use rperf3::{Server, Config, Protocol};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::server(8080)
///     .with_protocol(Protocol::Udp);
///
/// let server = Server::new(config);
/// server.run().await?;
/// # Ok(())
/// # }
/// ```
pub struct Server {
    config: Config,
    measurements: MeasurementsCollector,
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
        Self {
            config,
            measurements: MeasurementsCollector::new(),
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

                    tokio::spawn(async move {
                        if let Err(e) = handle_tcp_client(stream, addr, config, measurements).await
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

        let mut buf = vec![0u8; 65536];
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
    /// Returns a snapshot of the statistics collected from client tests.
    ///
    /// # Returns
    ///
    /// A `Measurements` struct containing test statistics.
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
    /// println!("Total bytes: {}", measurements.total_bytes_sent);
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
) -> Result<()> {
    // Read setup message
    let setup_msg = deserialize_message(&mut stream).await?;

    let (duration, reverse, _parallel) = match setup_msg {
        Message::Setup {
            version: _,
            protocol,
            duration,
            reverse,
            parallel,
            ..
        } => {
            info!(
                "Client {} setup: protocol={}, duration={}s, reverse={}, parallel={}",
                addr, protocol, duration, reverse, parallel
            );
            (Duration::from_secs(duration), reverse, parallel)
        }
        _ => {
            return Err(Error::Protocol("Expected Setup message".to_string()));
        }
    };

    // Send setup acknowledgment
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
        send_data(&mut stream, 0, duration, &measurements, &config).await?;
    } else {
        // Server receives data from client
        receive_data(&mut stream, 0, duration, &measurements, &config).await?;
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

async fn send_data(
    stream: &mut TcpStream,
    stream_id: usize,
    duration: Duration,
    measurements: &MeasurementsCollector,
    config: &Config,
) -> Result<()> {
    let buffer = vec![0u8; config.buffer_size];
    let start = Instant::now();
    let mut last_interval = start;
    let mut interval_bytes = 0u64;

    while start.elapsed() < duration {
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
) -> Result<()> {
    let mut buffer = vec![0u8; config.buffer_size];
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
