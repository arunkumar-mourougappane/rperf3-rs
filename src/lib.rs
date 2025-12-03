//! rperf3 - A Rust implementation of iperf3
//!
//! This library provides network performance testing capabilities similar to iperf3.
//! It supports both TCP and UDP performance measurements in client and server modes.
//!
//! # Features
//!
//! - TCP and UDP throughput testing
//! - Configurable test duration and data rates
//! - Real-time bandwidth measurement
//! - JSON output format
//! - Asynchronous I/O using tokio

pub mod protocol;
pub mod server;
pub mod client;
pub mod config;
pub mod measurements;
pub mod error;

pub use error::{Error, Result};
pub use config::{Config, Protocol};
pub use measurements::Measurements;
pub use client::{Client, ProgressCallback, ProgressEvent};
pub use server::Server;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
