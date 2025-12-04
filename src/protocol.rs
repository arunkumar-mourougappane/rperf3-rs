use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Current protocol version for client-server communication.
///
/// This version number is used to ensure compatibility between clients and servers.
/// If the protocol changes in a breaking way, this version should be incremented.
pub const PROTOCOL_VERSION: u32 = 1;

/// Message types in the rperf3 protocol.
///
/// These messages are exchanged between the client and server during test execution.
/// All messages are serialized as JSON with a `type` field discriminator.
///
/// # Protocol Flow
///
/// 1. Client sends `Setup` with test parameters
/// 2. Server responds with `SetupAck`
/// 3. Server sends `Start` to begin test
/// 4. `Interval` messages are sent periodically during test
/// 5. `Result` message contains final statistics
/// 6. `Done` signals test completion
/// 7. `Error` can be sent at any point to indicate failure
///
/// # Examples
///
/// ```
/// use rperf3::protocol::Message;
/// use std::time::Duration;
///
/// // Create a setup message
/// let setup = Message::setup(
///     "TCP".to_string(),
///     Duration::from_secs(10),
///     None,
///     128 * 1024,
///     1,
///     false,
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    /// Initial handshake from client with test parameters
    Setup {
        version: u32,
        protocol: String,
        duration: u64,
        bandwidth: Option<u64>,
        buffer_size: usize,
        parallel: usize,
        reverse: bool,
    },

    /// Server acknowledgment of setup
    SetupAck { port: u16, cookie: String },

    /// Test start signal
    Start { timestamp: u64 },

    /// Interval results
    Interval {
        stream_id: usize,
        start: f64,
        end: f64,
        bytes: u64,
        bits_per_second: f64,
    },

    /// Final results
    Result {
        stream_id: usize,
        bytes_sent: u64,
        bytes_received: u64,
        duration: f64,
        bits_per_second: f64,
        retransmits: Option<u64>,
    },

    /// Test completion signal
    Done,

    /// Error message
    Error { message: String },
}

impl Message {
    /// Creates a Setup message for test initialization.
    ///
    /// # Arguments
    ///
    /// * `protocol` - Protocol name ("TCP" or "UDP")
    /// * `duration` - Test duration
    /// * `bandwidth` - Target bandwidth for UDP (None for TCP)
    /// * `buffer_size` - Buffer size in bytes
    /// * `parallel` - Number of parallel streams
    /// * `reverse` - Whether to use reverse mode
    pub fn setup(
        protocol: String,
        duration: Duration,
        bandwidth: Option<u64>,
        buffer_size: usize,
        parallel: usize,
        reverse: bool,
    ) -> Self {
        Message::Setup {
            version: PROTOCOL_VERSION,
            protocol,
            duration: duration.as_secs(),
            bandwidth,
            buffer_size,
            parallel,
            reverse,
        }
    }

    /// Creates a SetupAck message.
    ///
    /// # Arguments
    ///
    /// * `port` - Server port number
    /// * `cookie` - Session identifier string
    pub fn setup_ack(port: u16, cookie: String) -> Self {
        Message::SetupAck { port, cookie }
    }

    /// Creates a Start message to begin the test.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - Unix timestamp when test starts
    pub fn start(timestamp: u64) -> Self {
        Message::Start { timestamp }
    }

    /// Creates an Interval message with periodic statistics.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - Stream identifier
    /// * `start` - Interval start time in seconds
    /// * `end` - Interval end time in seconds
    /// * `bytes` - Bytes transferred during interval
    /// * `bits_per_second` - Throughput during interval
    pub fn interval(
        stream_id: usize,
        start: f64,
        end: f64,
        bytes: u64,
        bits_per_second: f64,
    ) -> Self {
        Message::Interval {
            stream_id,
            start,
            end,
            bytes,
            bits_per_second,
        }
    }

    /// Creates a Result message with final test statistics.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - Stream identifier
    /// * `bytes_sent` - Total bytes sent
    /// * `bytes_received` - Total bytes received
    /// * `duration` - Test duration in seconds
    /// * `bits_per_second` - Average throughput
    /// * `retransmits` - TCP retransmit count (None for UDP)
    pub fn result(
        stream_id: usize,
        bytes_sent: u64,
        bytes_received: u64,
        duration: f64,
        bits_per_second: f64,
        retransmits: Option<u64>,
    ) -> Self {
        Message::Result {
            stream_id,
            bytes_sent,
            bytes_received,
            duration,
            bits_per_second,
            retransmits,
        }
    }

    /// Creates a Done message to signal test completion.
    pub fn done() -> Self {
        Message::Done
    }

    /// Creates an Error message.
    ///
    /// # Arguments
    ///
    /// * `message` - Error description
    pub fn error(message: String) -> Self {
        Message::Error { message }
    }
}

/// Serialize a message to JSON bytes
/// Serializes a protocol message to JSON bytes.
///
/// The serialized format is a length-prefixed JSON message:
/// - First 4 bytes: message length as big-endian u32
/// - Remaining bytes: UTF-8 encoded JSON
///
/// # Arguments
///
/// * `msg` - The message to serialize
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
///
/// # Examples
///
/// ```
/// use rperf3::protocol::{Message, serialize_message};
///
/// let msg = Message::done();
/// let bytes = serialize_message(&msg).expect("Serialization failed");
/// assert!(bytes.len() >= 4); // At least length prefix
/// ```
pub fn serialize_message(msg: &Message) -> crate::Result<Vec<u8>> {
    let json = serde_json::to_vec(msg)?;
    let len = json.len() as u32;
    let mut result = Vec::with_capacity(4 + json.len());
    result.extend_from_slice(&len.to_be_bytes());
    result.extend_from_slice(&json);
    Ok(result)
}

/// Deserializes a protocol message from an async reader.
///
/// Reads a length-prefixed JSON message from the stream:
/// - First 4 bytes: message length as big-endian u32
/// - Next N bytes: UTF-8 encoded JSON message
///
/// # Arguments
///
/// * `reader` - An async reader to deserialize from
///
/// # Errors
///
/// Returns an error if:
/// - Reading from the stream fails
/// - JSON deserialization fails
/// - Message format is invalid
///
/// # Examples
///
/// ```no_run
/// use rperf3::protocol::deserialize_message;
/// use tokio::net::TcpStream;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut stream = TcpStream::connect("127.0.0.1:5201").await?;
/// let message = deserialize_message(&mut stream).await?;
/// # Ok(())
/// # }
/// ```
pub async fn deserialize_message<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> crate::Result<Message> {
    use tokio::io::AsyncReadExt;

    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    let mut json_bytes = vec![0u8; len];
    reader.read_exact(&mut json_bytes).await?;

    let msg = serde_json::from_slice(&json_bytes)?;
    Ok(msg)
}
