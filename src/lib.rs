//! # rperf3-rs
//!
//! A network throughput measurement tool written in Rust, inspired by iperf3.
//!
//! This library provides high-performance network bandwidth testing capabilities for both
//! TCP and UDP protocols. Built on Tokio's async runtime, it offers memory safety guarantees
//! and modern async/await patterns.
//!
//! ## Features
//!
//! - **TCP and UDP Testing**: Measure throughput for both TCP and UDP protocols
//! - **Bidirectional Testing**: Normal mode (client sends) and reverse mode (server sends)
//! - **Real-time Statistics**: Periodic interval reporting with bandwidth measurements
//! - **Progress Callbacks**: Get real-time updates during test execution via callbacks
//! - **Multiple Streams**: Support for parallel stream testing
//! - **JSON Output**: Machine-readable output format for automation
//! - **Dual Interface**: Use as a Rust library or standalone CLI tool
//! - **Async I/O**: Built on Tokio for high-performance async operations
//! - **Platform-Specific Features**: Detailed TCP statistics on Linux (retransmits, RTT, congestion window)
//!
//! ## Quick Start
//!
//! ### Client Example
//!
//! ```no_run
//! use rperf3::{Client, Config, Protocol};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure the test
//!     let config = Config::client("127.0.0.1".to_string(), 5201)
//!         .with_protocol(Protocol::Tcp)
//!         .with_duration(Duration::from_secs(10))
//!         .with_buffer_size(128 * 1024);
//!
//!     // Run the test
//!     let client = Client::new(config)?;
//!     client.run().await?;
//!
//!     // Get results
//!     let measurements = client.get_measurements();
//!     println!("Bandwidth: {:.2} Mbps",
//!              measurements.total_bits_per_second() / 1_000_000.0);
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
//!     server.run().await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Client with Progress Callback
//!
//! ```no_run
//! use rperf3::{Client, Config, ProgressEvent, Protocol};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::client("127.0.0.1".to_string(), 5201)
//!         .with_protocol(Protocol::Tcp)
//!         .with_duration(Duration::from_secs(10));
//!
//!     let client = Client::new(config)?
//!         .with_callback(|event: ProgressEvent| {
//!             match event {
//!                 ProgressEvent::TestStarted => {
//!                     println!("Test started!");
//!                 }
//!                 ProgressEvent::IntervalUpdate { interval_end, bytes, bits_per_second, .. } => {
//!                     println!("{:.1}s: {} bytes @ {:.2} Mbps",
//!                         interval_end.as_secs_f64(),
//!                         bytes,
//!                         bits_per_second / 1_000_000.0);
//!                 }
//!                 ProgressEvent::TestCompleted { total_bytes, duration, bits_per_second, .. } => {
//!                     println!("Completed: {} bytes in {:.2}s @ {:.2} Mbps",
//!                         total_bytes,
//!                         duration.as_secs_f64(),
//!                         bits_per_second / 1_000_000.0);
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
//! ## Architecture
//!
//! The library is organized into the following modules:
//!
//! - [`client`]: Client implementation for initiating tests and collecting results
//! - [`server`]: Server implementation for handling connections and running tests
//! - [`config`]: Configuration structures with builder pattern
//! - [`measurements`]: Thread-safe statistics collection and calculation
//! - [`protocol`]: Message format and serialization for client-server communication
//! - [`error`]: Custom error types and result aliases

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
