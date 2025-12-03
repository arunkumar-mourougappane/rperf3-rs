use thiserror::Error;

/// Error types for rperf3 operations.
///
/// This enum covers all error cases that can occur during network performance testing,
/// from I/O errors to protocol violations.
///
/// # Examples
///
/// ```
/// use rperf3::Error;
///
/// fn check_config(server_addr: Option<String>) -> Result<(), Error> {
///     match server_addr {
///         Some(_) => Ok(()),
///         None => Err(Error::Config("Server address required".to_string())),
///     }
/// }
/// ```
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error occurred during network operations.
    ///
    /// This wraps standard `std::io::Error` and is used for all I/O-related failures
    /// such as socket errors, connection failures, and read/write errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization error.
    ///
    /// Occurs when encoding or decoding JSON data for protocol messages or output.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Connection-related error.
    ///
    /// Used for errors specific to establishing or maintaining network connections,
    /// such as connection refused, timeout, or unexpected disconnection.
    #[error("Connection error: {0}")]
    Connection(String),

    /// Protocol violation or parsing error.
    ///
    /// Occurs when the client and server exchange malformed or unexpected messages.
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Configuration error.
    ///
    /// Used when the provided configuration is invalid or incomplete.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Test execution error.
    ///
    /// Covers errors that occur during test execution that don't fit other categories.
    #[error("Test error: {0}")]
    Test(String),
}

/// Result type alias for rperf3 operations.
///
/// This is a convenience type alias that uses [`enum@Error`] as the error type.
/// Most functions in this library return this type.
///
/// # Examples
///
/// ```
/// use rperf3::{Result, Error};
///
/// fn validate_port(port: u16) -> Result<()> {
///     if port < 1024 {
///         Err(Error::Config("Port must be >= 1024".to_string()))
///     } else {
///         Ok(())
///     }
/// }
/// ```
pub type Result<T> = std::result::Result<T, Error>;
