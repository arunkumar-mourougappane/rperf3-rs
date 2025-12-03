use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Protocol version
pub const PROTOCOL_VERSION: u32 = 1;

/// Message types in the rperf3 protocol
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

    pub fn setup_ack(port: u16, cookie: String) -> Self {
        Message::SetupAck { port, cookie }
    }

    pub fn start(timestamp: u64) -> Self {
        Message::Start { timestamp }
    }

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

    pub fn done() -> Self {
        Message::Done
    }

    pub fn error(message: String) -> Self {
        Message::Error { message }
    }
}

/// Serialize a message to JSON bytes
pub fn serialize_message(msg: &Message) -> crate::Result<Vec<u8>> {
    let json = serde_json::to_vec(msg)?;
    let len = json.len() as u32;
    let mut result = Vec::with_capacity(4 + json.len());
    result.extend_from_slice(&len.to_be_bytes());
    result.extend_from_slice(&json);
    Ok(result)
}

/// Deserialize a message from JSON bytes
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
