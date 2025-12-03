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

pub mod client;
pub mod config;
pub mod error;
pub mod measurements;
pub mod protocol;
pub mod server;

pub use client::{Client, ProgressCallback, ProgressEvent};
pub use config::{Config, Protocol};
pub use error::{Error, Result};
pub use measurements::Measurements;
pub use server::Server;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
