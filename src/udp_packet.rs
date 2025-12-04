//! UDP packet format with sequence numbers and timestamps for loss/jitter measurement.
//!
//! This module implements a custom packet format for UDP testing that includes:
//! - Sequence numbers for packet loss detection
//! - Timestamps for jitter measurement
//! - Magic marker for packet identification
//!
//! # Packet Format
//!
//! ```text
//! ┌──────────────┬──────────────┬──────────────┬──────────────┐
//! │    Magic     │  Sequence    │  Timestamp   │   Payload    │
//! │   (4 bytes)  │  (8 bytes)   │  (8 bytes)   │  (variable)  │
//! └──────────────┴──────────────┴──────────────┴──────────────┘
//! ```
//!
//! - **Magic**: 0x52504633 ("RPF3" in ASCII) - identifies rperf3 packets
//! - **Sequence**: 64-bit monotonically increasing packet number (big-endian)
//! - **Timestamp**: Send time in microseconds since UNIX epoch (big-endian)
//! - **Payload**: Zero-filled data for throughput testing
//!
//! # Packet Loss Measurement
//!
//! Packet loss is detected by tracking sequence number gaps. If the receiver sees
//! sequences [0, 1, 3, 5], it knows packets 2 and 4 were lost.
//!
//! # Jitter Measurement
//!
//! Jitter is calculated using RFC 3550 (RTP) algorithm:
//! ```text
//! J(i) = J(i-1) + (|D(i-1,i)| - J(i-1)) / 16
//! ```
//! where D(i-1,i) is the difference in relative transit times between packets.
//!
//! # Examples
//!
//! ```
//! use rperf3::udp_packet::{create_packet, parse_packet};
//!
//! // Create a packet with sequence 42 and 1024 bytes of payload
//! let packet = create_packet(42, 1024);
//! assert_eq!(packet.len(), 20 + 1024); // header + payload
//!
//! // Parse the packet
//! let (header, payload) = parse_packet(&packet).expect("Invalid packet");
//! assert_eq!(header.sequence, 42);
//! assert_eq!(payload.len(), 1024);
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

/// Magic marker to identify rperf3 UDP packets
const RPERF3_UDP_MAGIC: u32 = 0x52504633; // "RPF3" in ASCII

/// UDP packet header with sequence number and timing information
///
/// This header is prepended to UDP payload to enable packet loss and jitter measurement.
/// The format is:
/// ```text
/// | Magic (4 bytes) | Sequence (8 bytes) | Timestamp (8 bytes) | Payload (variable) |
/// ```
#[derive(Debug, Clone, Copy)]
pub struct UdpPacketHeader {
    /// Magic marker to identify rperf3 packets
    pub magic: u32,
    /// Packet sequence number (monotonically increasing)
    pub sequence: u64,
    /// Send timestamp in microseconds since UNIX epoch
    pub timestamp_us: u64,
}

impl UdpPacketHeader {
    /// Size of the header in bytes
    pub const SIZE: usize = 20; // 4 (magic) + 8 (sequence) + 8 (timestamp)

    /// Creates a new UDP packet header
    ///
    /// # Arguments
    ///
    /// * `sequence` - Packet sequence number
    /// * `timestamp_us` - Send timestamp in microseconds
    pub fn new(sequence: u64, timestamp_us: u64) -> Self {
        Self {
            magic: RPERF3_UDP_MAGIC,
            sequence,
            timestamp_us,
        }
    }

    /// Creates a header with the current timestamp
    ///
    /// # Arguments
    ///
    /// * `sequence` - Packet sequence number
    pub fn with_current_time(sequence: u64) -> Self {
        let timestamp_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_micros() as u64;
        Self::new(sequence, timestamp_us)
    }

    /// Serializes the header to bytes (big-endian)
    ///
    /// # Returns
    ///
    /// 20-byte array containing the serialized header
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..4].copy_from_slice(&self.magic.to_be_bytes());
        bytes[4..12].copy_from_slice(&self.sequence.to_be_bytes());
        bytes[12..20].copy_from_slice(&self.timestamp_us.to_be_bytes());
        bytes
    }

    /// Deserializes a header from bytes
    ///
    /// # Arguments
    ///
    /// * `bytes` - Byte slice containing at least 20 bytes
    ///
    /// # Returns
    ///
    /// `Some(header)` if magic marker matches, `None` otherwise
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }

        let magic = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
        if magic != RPERF3_UDP_MAGIC {
            return None;
        }

        let sequence = u64::from_be_bytes(bytes[4..12].try_into().ok()?);
        let timestamp_us = u64::from_be_bytes(bytes[12..20].try_into().ok()?);

        Some(Self {
            magic,
            sequence,
            timestamp_us,
        })
    }
}

/// Creates a UDP packet with header and payload.
///
/// Constructs a complete UDP packet with the current timestamp and zero-filled payload.
/// The packet format is: `[header (20 bytes)][payload (payload_size bytes)]`.
///
/// # Arguments
///
/// * `sequence` - Packet sequence number (should be monotonically increasing)
/// * `payload_size` - Size of payload in bytes (excluding 20-byte header)
///
/// # Returns
///
/// A vector containing the complete packet (header + payload)
///
/// # Examples
///
/// ```
/// use rperf3::udp_packet::create_packet;
///
/// // Create a packet with sequence 0 and 1024 bytes of data
/// let packet = create_packet(0, 1024);
/// assert_eq!(packet.len(), 20 + 1024); // header + payload
/// ```
///
/// # Returns
///
/// Vector containing serialized header followed by zero-filled payload
pub fn create_packet(sequence: u64, payload_size: usize) -> Vec<u8> {
    let header = UdpPacketHeader::with_current_time(sequence);
    let mut packet = Vec::with_capacity(UdpPacketHeader::SIZE + payload_size);
    packet.extend_from_slice(&header.to_bytes());
    packet.resize(UdpPacketHeader::SIZE + payload_size, 0);
    packet
}

/// Parses a UDP packet into header and payload
///
/// # Arguments
///
/// * `packet` - Received packet bytes
///
/// # Returns
///
/// `Some((header, payload))` if packet has valid header, `None` otherwise
pub fn parse_packet(packet: &[u8]) -> Option<(UdpPacketHeader, &[u8])> {
    let header = UdpPacketHeader::from_bytes(packet)?;
    let payload = &packet[UdpPacketHeader::SIZE..];
    Some((header, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_serialization() {
        let header = UdpPacketHeader::new(42, 1234567890);
        let bytes = header.to_bytes();
        let parsed = UdpPacketHeader::from_bytes(&bytes).expect("Failed to parse header");

        assert_eq!(parsed.magic, RPERF3_UDP_MAGIC);
        assert_eq!(parsed.sequence, 42);
        assert_eq!(parsed.timestamp_us, 1234567890);
    }

    #[test]
    fn test_invalid_magic() {
        let mut bytes = [0u8; UdpPacketHeader::SIZE];
        bytes[0..4].copy_from_slice(&0x12345678u32.to_be_bytes());
        assert!(UdpPacketHeader::from_bytes(&bytes).is_none());
    }

    #[test]
    fn test_packet_creation() {
        let packet = create_packet(100, 1024);
        assert_eq!(packet.len(), UdpPacketHeader::SIZE + 1024);

        let (header, payload) = parse_packet(&packet).expect("Failed to parse packet");
        assert_eq!(header.sequence, 100);
        assert_eq!(payload.len(), 1024);
    }

    #[test]
    fn test_short_packet() {
        let short_packet = vec![0u8; 10];
        assert!(parse_packet(&short_packet).is_none());
    }
}
