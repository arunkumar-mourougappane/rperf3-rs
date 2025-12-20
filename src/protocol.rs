use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Current protocol version for client-server communication.
///
/// This version number is used to ensure compatibility between clients and servers.
/// If the protocol changes in a breaking way, this version should be incremented.
pub const PROTOCOL_VERSION: u32 = 1;

/// Default stream ID, matching iperf3's behavior.
///
/// iperf3 uses stream ID 5 as the default for single stream tests.
/// For parallel streams, IDs are typically sequential starting from this base.
pub const DEFAULT_STREAM_ID: usize = 5;

/// Generates a stream ID for parallel streams.
///
/// Returns a stream ID based on the stream index, matching iperf3's behavior
/// where stream IDs are sequential starting from the default.
///
/// # Arguments
///
/// * `index` - Zero-based stream index (0 for first stream, 1 for second, etc.)
///
/// # Examples
///
/// ```
/// use rperf3::protocol::stream_id_for_index;
///
/// assert_eq!(stream_id_for_index(0), 5);  // First stream
/// assert_eq!(stream_id_for_index(1), 7);  // Second stream
/// assert_eq!(stream_id_for_index(2), 9);  // Third stream
/// ```
pub fn stream_id_for_index(index: usize) -> usize {
    DEFAULT_STREAM_ID + (index * 2)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn test_default_stream_id() {
        assert_eq!(DEFAULT_STREAM_ID, 5);
    }

    #[test]
    fn test_stream_id_for_index() {
        assert_eq!(stream_id_for_index(0), 5);
        assert_eq!(stream_id_for_index(1), 7);
        assert_eq!(stream_id_for_index(2), 9);
        assert_eq!(stream_id_for_index(10), 25);
    }

    #[test]
    fn test_message_setup() {
        let msg = Message::setup(
            "TCP".to_string(),
            Duration::from_secs(10),
            Some(100_000_000),
            128 * 1024,
            2,
            false,
        );

        match msg {
            Message::Setup {
                version,
                protocol,
                duration,
                bandwidth,
                buffer_size,
                parallel,
                reverse,
            } => {
                assert_eq!(version, PROTOCOL_VERSION);
                assert_eq!(protocol, "TCP");
                assert_eq!(duration, 10);
                assert_eq!(bandwidth, Some(100_000_000));
                assert_eq!(buffer_size, 128 * 1024);
                assert_eq!(parallel, 2);
                assert!(!reverse);
            }
            _ => panic!("Expected Setup message"),
        }
    }

    #[test]
    fn test_message_setup_ack() {
        let msg = Message::setup_ack(5201, "cookie123".to_string());

        match msg {
            Message::SetupAck { port, cookie } => {
                assert_eq!(port, 5201);
                assert_eq!(cookie, "cookie123");
            }
            _ => panic!("Expected SetupAck message"),
        }
    }

    #[test]
    fn test_message_start() {
        let msg = Message::start(1234567890);

        match msg {
            Message::Start { timestamp } => {
                assert_eq!(timestamp, 1234567890);
            }
            _ => panic!("Expected Start message"),
        }
    }

    #[test]
    fn test_message_done() {
        let msg = Message::Done;
        assert!(matches!(msg, Message::Done));
    }

    #[test]
    fn test_message_error() {
        let msg = Message::Error {
            message: "Test failed".to_string(),
        };

        match msg {
            Message::Error { message } => {
                assert_eq!(message, "Test failed");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_serialize_deserialize_setup() {
        let msg = Message::setup(
            "UDP".to_string(),
            Duration::from_secs(30),
            Some(50_000_000),
            65536,
            1,
            true,
        );

        let serialized = serialize_message(&msg).unwrap();

        // Verify serialized format: length prefix (4 bytes) + JSON
        assert!(serialized.len() > 4);
        let len = u32::from_be_bytes([serialized[0], serialized[1], serialized[2], serialized[3]]);
        assert_eq!(len as usize, serialized.len() - 4);

        // Deserialize JSON directly (without length prefix)
        let json_bytes = &serialized[4..];
        let deserialized: Message = serde_json::from_slice(json_bytes).unwrap();

        match deserialized {
            Message::Setup {
                version,
                protocol,
                duration,
                bandwidth,
                buffer_size,
                parallel,
                reverse,
            } => {
                assert_eq!(version, PROTOCOL_VERSION);
                assert_eq!(protocol, "UDP");
                assert_eq!(duration, 30);
                assert_eq!(bandwidth, Some(50_000_000));
                assert_eq!(buffer_size, 65536);
                assert_eq!(parallel, 1);
                assert!(reverse);
            }
            _ => panic!("Expected Setup message"),
        }
    }

    #[test]
    fn test_serialize_deserialize_done() {
        let msg = Message::Done;
        let serialized = serialize_message(&msg).unwrap();

        // Deserialize JSON directly
        let json_bytes = &serialized[4..];
        let deserialized: Message = serde_json::from_slice(json_bytes).unwrap();
        assert!(matches!(deserialized, Message::Done));
    }

    #[test]
    fn test_serialize_format() {
        let msg = Message::Done;
        let serialized = serialize_message(&msg).unwrap();

        // Check length prefix
        assert!(serialized.len() >= 4);
        let len = u32::from_be_bytes([serialized[0], serialized[1], serialized[2], serialized[3]]);
        assert_eq!(len as usize + 4, serialized.len());
    }

    #[test]
    fn test_deserialize_invalid_json() {
        let invalid_json = b"{invalid json}";
        let result: Result<Message, _> = serde_json::from_slice(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_json_roundtrip() {
        let msg = Message::Error {
            message: "Test error".to_string(),
        };

        let json = serde_json::to_vec(&msg).unwrap();
        let deserialized: Message = serde_json::from_slice(&json).unwrap();

        match deserialized {
            Message::Error { message } => {
                assert_eq!(message, "Test error");
            }
            _ => panic!("Expected Error message"),
        }
    }
}
