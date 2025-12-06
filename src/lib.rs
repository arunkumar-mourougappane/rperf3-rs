//! # rperf3-rs
//!
//! A high-performance network throughput measurement tool written in Rust, inspired by iperf3.
//!
//! This library provides accurate bandwidth testing capabilities for both TCP and UDP protocols.
//! Built on Tokio's async runtime with Rust's memory safety guarantees, rperf3-rs eliminates
//! entire classes of bugs (buffer overflows, use-after-free, data races) while achieving
//! 25-30 Gbps throughput on localhost tests.
//!
//! ## Features
//!
//! - **TCP & UDP Testing**: Measure throughput for both reliable and unreliable protocols
//! - **Bidirectional Testing**: Normal mode (client → server) and reverse mode (server → client)
//! - **Bandwidth Limiting**: Control send rate for both TCP and UDP with K/M/G notation (e.g., 100M = 100 Mbps)
//! - **UDP Metrics**: Packet loss percentage, jitter (RFC 3550), and out-of-order packet detection
//! - **TCP Statistics**: Retransmits, congestion window (cwnd), and real-time interval reporting (Linux only)
//! - **Interval Reporting**: Configurable interval updates with iperf3-style formatted output (default: 1 second)
//! - **Real-time Callbacks**: Monitor test progress programmatically with event-driven callbacks
//! - **Parallel Streams**: Multiple concurrent connections for aggregate testing
//! - **JSON Output**: Machine-readable output compatible with automation systems
//! - **Dual Interface**: Use as a Rust library or standalone CLI tool
//! - **Async I/O**: Built on Tokio for high-performance non-blocking operations
//! - **Cross-Platform**: Linux, macOS, and Windows support
//!
//! ## Quick Start
//!
//! ### Basic TCP Test
//!
//! ```no_run
//! use rperf3::{Client, Config, Protocol};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::client("192.168.1.100".to_string(), 5201)
//!         .with_protocol(Protocol::Tcp)
//!         .with_duration(Duration::from_secs(10))
//!         .with_interval(Duration::from_secs(1)); // Report every second
//!
//!     let client = Client::new(config)?;
//!     client.run().await?;
//!
//!     let measurements = client.get_measurements();
//!     println!("Bandwidth: {:.2} Mbps",
//!              measurements.total_bits_per_second() / 1_000_000.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ### UDP Test with Metrics
//!
//! ```no_run
//! use rperf3::{Client, Config, Protocol};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::client("192.168.1.100".to_string(), 5201)
//!         .with_protocol(Protocol::Udp)
//!         .with_bandwidth(100_000_000) // 100 Mbps
//!         .with_duration(Duration::from_secs(10));
//!
//!     let client = Client::new(config)?;
//!     client.run().await?;
//!
//!     let measurements = client.get_measurements();
//!     println!("Bandwidth: {:.2} Mbps",
//!              measurements.total_bits_per_second() / 1_000_000.0);
//!     println!("Packets: {}, Loss: {} ({:.2}%), Jitter: {:.3} ms",
//!              measurements.total_packets,
//!              measurements.lost_packets,
//!              (measurements.lost_packets as f64 / measurements.total_packets as f64) * 100.0,
//!              measurements.jitter_ms);
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Server Example
//!
//! ```no_run
//! use rperf3::{Server, Config};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::server(5201);
//!     let server = Server::new(config);
//!     
//!     println!("Server listening on port 5201");
//!     server.run().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Progress Callbacks
//!
//! Monitor test progress in real-time:
//!
//! ```no_run
//! use rperf3::{Client, Config, ProgressEvent};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::client("192.168.1.100".to_string(), 5201)
//!         .with_duration(Duration::from_secs(10));
//!
//!     let client = Client::new(config)?
//!         .with_callback(|event: ProgressEvent| {
//!             match event {
//!                 ProgressEvent::TestStarted => {
//!                     println!("Test started");
//!                 }
//!                 ProgressEvent::IntervalUpdate { bits_per_second, .. } => {
//!                     println!("Current: {:.2} Mbps", bits_per_second / 1_000_000.0);
//!                 }
//!                 ProgressEvent::TestCompleted { bits_per_second, .. } => {
//!                     println!("Average: {:.2} Mbps", bits_per_second / 1_000_000.0);
//!                 }
//!                 ProgressEvent::Error(msg) => {
//!                     eprintln!("Error: {}", msg);
//!                 }
//!             }
//!         });
//!
//!     client.run().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Bandwidth Notation
//!
//! When specifying bandwidth limits, use K/M/G suffixes:
//! - `100K` = 100,000 bits/second
//! - `100M` = 100,000,000 bits/second
//! - `1G` = 1,000,000,000 bits/second
//!
//! The bandwidth limiting applies to both TCP (in reverse mode) and UDP tests.
//!
//! ## Interval Reporting
//!
//! Real-time interval reports show throughput statistics at regular intervals (default: 1 second).
//! Reports use iperf3-compatible formatting with proper alignment:
//!
//! ```text
//! [ ID] Interval           Transfer        Bitrate            Retr  Cwnd
//! [  5]   0.00-1.00  sec    7.23  GBytes    58.1  Gbits/sec     0
//! [  5]   1.00-2.00  sec    7.42  GBytes    59.4  Gbits/sec     0   1215 KBytes
//! ```
//!
//! Configure intervals with the `-i` flag or `.with_interval()` method:
//!
//! ```no_run
//! use rperf3::{Client, Config};
//! use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = Config::client("192.168.1.100".to_string(), 5201)
//!     .with_duration(Duration::from_secs(10))
//!     .with_interval(Duration::from_secs(2)); // Report every 2 seconds
//!
//! let client = Client::new(config)?;
//! client.run().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! The library is organized into the following modules:
//!
//! - [`client`]: Client implementation for initiating tests and collecting results
//! - [`server`]: Server implementation for handling connections and running tests
//! - [`config`]: Configuration structures with builder pattern
//! - [`measurements`]: Thread-safe statistics collection and calculation
//! - [`protocol`]: Message format and serialization for client-server communication
//! - [`udp_packet`]: UDP packet format with sequence numbers and timestamps
//! - [`error`]: Custom error types and result aliases
//!
//! ## Performance
//!
//! Typical performance on modern hardware:
//! - **TCP localhost**: 25-30 Gbps
//! - **UDP with limiting**: Accurate rate control within 2-3% of target
//! - **Packet loss detection**: Sub-millisecond precision
//! - **Jitter measurement**: RFC 3550 compliant algorithm

pub mod buffer_pool;
pub mod client;
pub mod config;
pub mod error;
pub mod measurements;
pub mod protocol;
pub mod server;
pub mod udp_packet;

pub use client::{Client, ProgressCallback, ProgressEvent};
pub use config::{Config, Protocol};
pub use error::{Error, Result};
pub use measurements::Measurements;
pub use protocol::{stream_id_for_index, DEFAULT_STREAM_ID};
pub use server::Server;

/// Library version string.
///
/// This constant contains the version of the rperf3-rs library, automatically
/// extracted from the package version in `Cargo.toml`.
///
/// # Examples
///
/// ```
/// use rperf3::VERSION;
///
/// println!("rperf3-rs version: {}", VERSION);
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
