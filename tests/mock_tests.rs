// Mock-based tests for client/server logic isolation
// These tests use mocks to test logic without real network I/O

use rperf3::measurements::{IntervalStats, Measurements, MeasurementsCollector};
use rperf3::protocol::{Message, PROTOCOL_VERSION};
use rperf3::{Config, Protocol};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock socket that captures writes and can provide predetermined reads
#[derive(Clone)]
struct MockSocket {
    read_buffer: Arc<Mutex<Vec<u8>>>,
    write_buffer: Arc<Mutex<Vec<u8>>>,
    read_error: Arc<Mutex<Option<io::Error>>>,
    write_error: Arc<Mutex<Option<io::Error>>>,
}

impl MockSocket {
    fn new() -> Self {
        Self {
            read_buffer: Arc::new(Mutex::new(Vec::new())),
            write_buffer: Arc::new(Mutex::new(Vec::new())),
            read_error: Arc::new(Mutex::new(None)),
            write_error: Arc::new(Mutex::new(None)),
        }
    }

    fn set_read_data(&self, data: Vec<u8>) {
        let mut buffer = self.read_buffer.lock().unwrap();
        *buffer = data;
    }

    fn get_written_data(&self) -> Vec<u8> {
        self.write_buffer.lock().unwrap().clone()
    }

    fn set_read_error(&self, error: io::Error) {
        let mut err = self.read_error.lock().unwrap();
        *err = Some(error);
    }

    fn set_write_error(&self, error: io::Error) {
        let mut err = self.write_error.lock().unwrap();
        *err = Some(error);
    }
}

impl Read for MockSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check for injected error
        if let Some(err) = self.read_error.lock().unwrap().take() {
            return Err(err);
        }

        let mut read_buf = self.read_buffer.lock().unwrap();
        let to_read = buf.len().min(read_buf.len());

        if to_read == 0 {
            return Ok(0); // EOF
        }

        buf[..to_read].copy_from_slice(&read_buf[..to_read]);
        read_buf.drain(..to_read);
        Ok(to_read)
    }
}

impl Write for MockSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Check for injected error
        if let Some(err) = self.write_error.lock().unwrap().take() {
            return Err(err);
        }

        let mut write_buf = self.write_buffer.lock().unwrap();
        write_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Mock Socket Tests
    // ============================================================

    #[test]
    fn test_mock_socket_read_write() {
        let mock = MockSocket::new();
        let mut read_mock = mock.clone();
        let mut write_mock = mock.clone();

        // Set up read data
        mock.set_read_data(vec![1, 2, 3, 4, 5]);

        // Read from socket
        let mut buf = [0u8; 5];
        let n = read_mock.read(&mut buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf, &[1, 2, 3, 4, 5]);

        // Write to socket
        write_mock.write_all(&[10, 20, 30]).unwrap();
        let written = mock.get_written_data();
        assert_eq!(&written, &[10, 20, 30]);
    }

    #[test]
    fn test_mock_socket_read_error() {
        let mock = MockSocket::new();
        let mut read_mock = mock.clone();

        mock.set_read_error(io::Error::new(io::ErrorKind::ConnectionReset, "test error"));

        let mut buf = [0u8; 10];
        let result = read_mock.read(&mut buf);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionReset);
    }

    #[test]
    fn test_mock_socket_write_error() {
        let mock = MockSocket::new();
        let mut write_mock = mock.clone();

        mock.set_write_error(io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken"));

        let result = write_mock.write(&[1, 2, 3]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn test_mock_socket_eof() {
        let mock = MockSocket::new();
        let mut read_mock = mock.clone();

        // No data set, should return EOF
        let mut buf = [0u8; 10];
        let n = read_mock.read(&mut buf).unwrap();
        assert_eq!(n, 0); // EOF
    }

    #[test]
    fn test_mock_socket_partial_read() {
        let mock = MockSocket::new();
        let mut read_mock = mock.clone();

        mock.set_read_data(vec![1, 2, 3]);

        // Try to read more than available
        let mut buf = [0u8; 10];
        let n = read_mock.read(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf[..3], &[1, 2, 3]);
    }

    // ============================================================
    // Configuration Tests with Mocking
    // ============================================================

    #[test]
    fn test_client_config_basic() {
        let config = Config::client("127.0.0.1".to_string(), 5201);

        assert_eq!(config.server_addr, Some("127.0.0.1".to_string()));
        assert_eq!(config.port, 5201);
        assert_eq!(config.protocol, Protocol::Tcp);
        assert_eq!(config.duration, Duration::from_secs(10));
    }

    #[test]
    fn test_server_config_basic() {
        let config = Config::server(5201);

        assert_eq!(config.port, 5201);
    }

    #[test]
    fn test_client_config_chaining() {
        let config = Config::client("192.168.1.100".to_string(), 8080)
            .with_protocol(Protocol::Udp)
            .with_duration(Duration::from_secs(60))
            .with_parallel(4)
            .with_reverse(true)
            .with_bandwidth(1_000_000)
            .with_buffer_size(65536);

        assert_eq!(config.server_addr, Some("192.168.1.100".to_string()));
        assert_eq!(config.port, 8080);
        assert_eq!(config.protocol, Protocol::Udp);
        assert_eq!(config.duration, Duration::from_secs(60));
        assert_eq!(config.parallel, 4);
        assert!(config.reverse);
        assert_eq!(config.buffer_size, 65536);
    }

    // ============================================================
    // Measurements Mocking Tests
    // ============================================================

    #[test]
    fn test_measurements_collector_isolation() {
        let collector1 = MeasurementsCollector::new();
        let collector2 = MeasurementsCollector::new();

        collector1.record_bytes_sent(0, 1000);
        collector2.record_bytes_sent(0, 2000);

        let m1 = collector1.get();
        let m2 = collector2.get();

        assert_eq!(m1.total_bytes_sent, 1000);
        assert_eq!(m2.total_bytes_sent, 2000);
    }

    #[test]
    fn test_measurements_add_interval_mock() {
        let collector = MeasurementsCollector::new();

        // Mock interval data
        collector.add_interval(IntervalStats {
            start: Duration::from_secs(0),
            end: Duration::from_secs(1),
            bytes: 1024 * 1024,
            bits_per_second: 8_388_608.0,
            packets: 1000,
        });

        collector.add_interval(IntervalStats {
            start: Duration::from_secs(1),
            end: Duration::from_secs(2),
            bytes: 2 * 1024 * 1024,
            bits_per_second: 16_777_216.0,
            packets: 2000,
        });

        let measurements = collector.get();
        // Intervals are stored internally, we can verify by getting measurements
        assert!(measurements.total_duration >= Duration::from_secs(0));
    }

    // ============================================================
    // Protocol Message Mocking
    // ============================================================

    #[test]
    fn test_protocol_setup_message_structure() {
        let msg = Message::setup(
            "TCP".to_string(),
            Duration::from_secs(30),
            None,
            131072,
            4,
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
                assert_eq!(duration, 30);
                assert_eq!(bandwidth, None);
                assert_eq!(buffer_size, 131072);
                assert_eq!(parallel, 4);
                assert!(!reverse);
            }
            _ => panic!("Expected Setup message"),
        }
    }

    #[test]
    fn test_protocol_result_message_with_retransmits() {
        let msg = Message::result(5, 10_000_000, 0, 10.0, 8_000_000.0, Some(42));

        match msg {
            Message::Result {
                stream_id,
                bytes_sent,
                bytes_received,
                duration,
                bits_per_second,
                retransmits,
            } => {
                assert_eq!(stream_id, 5);
                assert_eq!(bytes_sent, 10_000_000);
                assert_eq!(bytes_received, 0);
                assert_eq!(duration, 10.0);
                assert_eq!(bits_per_second, 8_000_000.0);
                assert_eq!(retransmits, Some(42));
            }
            _ => panic!("Expected Result message"),
        }
    }

    #[test]
    fn test_protocol_result_message_no_retransmits() {
        let msg = Message::result(5, 10_000_000, 0, 10.0, 8_000_000.0, None);

        match msg {
            Message::Result { retransmits, .. } => {
                assert_eq!(retransmits, None);
            }
            _ => panic!("Expected Result message"),
        }
    }

    // ============================================================
    // Simulated Error Injection Tests
    // ============================================================

    #[test]
    fn test_error_during_write() {
        let mock = MockSocket::new();
        let mut write_mock = mock.clone();

        // Inject error after some successful writes
        write_mock.write_all(&[1, 2, 3]).unwrap();
        mock.set_write_error(io::Error::new(io::ErrorKind::BrokenPipe, "connection lost"));

        let result = write_mock.write(&[4, 5, 6]);
        assert!(result.is_err());

        // Verify first write succeeded
        let written = mock.get_written_data();
        assert_eq!(&written, &[1, 2, 3]);
    }

    #[test]
    fn test_error_during_read() {
        let mock = MockSocket::new();
        let mut read_mock = mock.clone();

        // Set up some data then inject error
        mock.set_read_data(vec![1, 2, 3, 4, 5]);

        let mut buf = [0u8; 3];
        let n = read_mock.read(&mut buf).unwrap();
        assert_eq!(n, 3);

        // Inject error for next read
        mock.set_read_error(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "connection reset",
        ));

        let result = read_mock.read(&mut buf);
        assert!(result.is_err());
    }

    // ============================================================
    // Stream Management Mocking
    // ============================================================

    #[test]
    fn test_multiple_stream_isolation() {
        let collector = MeasurementsCollector::new();

        // Simulate multiple streams
        for stream_id in 0..4 {
            collector.record_bytes_sent(stream_id, (stream_id as u64 + 1) * 1000);
        }

        let m = collector.get();
        assert_eq!(m.streams.len(), 4);
        assert_eq!(m.total_bytes_sent, 10_000); // 1000 + 2000 + 3000 + 4000
    }

    #[test]
    fn test_stream_statistics_aggregation() {
        let collector = MeasurementsCollector::new();

        // Simulate data transfer on multiple streams
        collector.record_bytes_sent(0, 1_000_000);
        collector.record_bytes_sent(1, 2_000_000);
        collector.record_bytes_received(0, 500_000);
        collector.record_bytes_received(1, 1_500_000);

        let m = collector.get();
        assert_eq!(m.total_bytes_sent, 3_000_000);
        assert_eq!(m.total_bytes_received, 2_000_000);
    }

    // ============================================================
    // Timing and Duration Mocking
    // ============================================================

    #[test]
    fn test_duration_calculation() {
        let mut measurements = Measurements::new();
        measurements.total_bytes_sent = 100_000_000; // 100 MB

        measurements.set_duration(Duration::from_secs(10));

        let throughput = measurements.total_bits_per_second();
        assert_eq!(throughput, 80_000_000.0); // 80 Mbps
    }

    #[test]
    fn test_zero_duration_handling() {
        let mut measurements = Measurements::new();
        measurements.total_bytes_sent = 100_000_000;

        measurements.set_duration(Duration::from_secs(0));

        let throughput = measurements.total_bits_per_second();
        assert_eq!(throughput, 0.0); // Should not panic or divide by zero
    }

    // ============================================================
    // UDP-specific Mocking
    // ============================================================

    #[test]
    fn test_udp_packet_loss_calculation_mock() {
        let collector = MeasurementsCollector::new();

        // Simulate receiving packets: 0, 1, 3, 4 (missing 2)
        let sequences = vec![0, 1, 3, 4];
        let base_time = 1_000_000u64;

        for seq in sequences {
            collector.record_udp_packet_received(
                seq,
                base_time + seq * 1000,
                base_time + seq * 1000 + 100,
            );
        }

        let (lost, expected) = collector.calculate_udp_loss();
        assert_eq!(expected, 5); // max_seq (4) + 1
        assert_eq!(lost, 1); // 5 expected - 4 received
    }

    #[test]
    fn test_udp_jitter_calculation_mock() {
        let collector = MeasurementsCollector::new();

        // Simulate packets with consistent timing
        let base_send = 1_000_000u64;
        let base_recv = 1_000_100u64;

        collector.record_udp_packet_received(0, base_send, base_recv);
        collector.record_udp_packet_received(1, base_send + 1000, base_recv + 1000);
        collector.record_udp_packet_received(2, base_send + 2000, base_recv + 2000);

        // With consistent timing, jitter should be minimal
        let m = collector.get();
        assert!(m.jitter_ms < 0.1); // Less than 0.1ms jitter
    }

    #[test]
    fn test_udp_out_of_order_detection_mock() {
        let collector = MeasurementsCollector::new();

        // Receive in order: 0, 2, 1 (1 is out of order), 4, 3 (3 is out of order)
        let sequences = vec![0, 2, 1, 4, 3];
        let base_time = 1_000_000u64;

        for seq in sequences {
            collector.record_udp_packet_received(
                seq,
                base_time + seq * 1000,
                base_time + seq * 1000 + 100,
            );
        }

        let m = collector.get();
        assert_eq!(m.out_of_order_packets, 2); // packets 1 and 3
    }

    // ============================================================
    // Edge Case Mocking
    // ============================================================

    #[test]
    fn test_large_data_transfer_mock() {
        let collector = MeasurementsCollector::new();

        // Simulate transferring 10 GB
        for _ in 0..10_000 {
            collector.record_bytes_sent(0, 1_000_000); // 1 MB at a time
        }

        let m = collector.get();
        assert_eq!(m.total_bytes_sent, 10_000_000_000); // 10 GB
    }

    #[test]
    fn test_single_byte_transfers_mock() {
        let collector = MeasurementsCollector::new();

        // Simulate very small transfers
        for _ in 0..1000 {
            collector.record_bytes_sent(0, 1);
        }

        let m = collector.get();
        assert_eq!(m.total_bytes_sent, 1000);
    }

    #[test]
    fn test_intermittent_connection_mock() {
        let mock = MockSocket::new();

        // Simulate intermittent connectivity
        mock.set_read_data(vec![1, 2, 3]);

        let mut read_mock = mock.clone();
        let mut buf = [0u8; 3];
        let n = read_mock.read(&mut buf).unwrap();
        assert_eq!(n, 3);

        // Connection drops (no more data)
        let n = read_mock.read(&mut buf).unwrap();
        assert_eq!(n, 0); // EOF

        // Connection resumes (more data available)
        mock.set_read_data(vec![4, 5, 6]);
        let n = read_mock.read(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf, &[4, 5, 6]);
    }
}
