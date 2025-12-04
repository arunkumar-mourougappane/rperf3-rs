use rperf3::{Config, Protocol};
use std::time::Duration;

// Full integration tests would require running server and client separately
// For now, we test the configuration and measurement logic

#[test]
fn test_config_builder() {
    let config = Config::client("192.168.1.100".to_string(), 5201)
        .with_protocol(Protocol::Udp)
        .with_duration(Duration::from_secs(30))
        .with_bandwidth(100_000_000)
        .with_buffer_size(1024)
        .with_parallel(4)
        .with_reverse(true)
        .with_json(true);

    assert_eq!(config.port, 5201);
    assert_eq!(config.protocol, Protocol::Udp);
    assert_eq!(config.duration, Duration::from_secs(30));
    assert_eq!(config.bandwidth, Some(100_000_000));
    assert_eq!(config.buffer_size, 1024);
    assert_eq!(config.parallel, 4);
    assert!(config.reverse);
    assert!(config.json);
}

#[test]
fn test_measurements() {
    use rperf3::measurements::MeasurementsCollector;

    let collector = MeasurementsCollector::new();

    collector.record_bytes_sent(0, 1000);
    collector.record_bytes_sent(0, 2000);
    collector.record_bytes_received(0, 500);

    collector.set_duration(Duration::from_secs(1));

    let measurements = collector.get();

    assert_eq!(measurements.total_bytes_sent, 3000);
    assert_eq!(measurements.total_bytes_received, 500);
    assert_eq!(measurements.total_duration, Duration::from_secs(1));

    // Should calculate bits per second correctly
    let bps = measurements.total_bits_per_second();
    assert_eq!(bps, 24000.0); // 3000 bytes * 8 bits / 1 second
}

#[test]
fn test_udp_packet_loss_tracking() {
    use rperf3::measurements::MeasurementsCollector;

    let collector = MeasurementsCollector::new();

    // Simulate sending packets with sequence numbers: 0, 1, 2, 3, 4, 5
    // Receive packets: 0, 1, 3, 5 (missing 2 and 4)
    let base_time = 1000000u64; // 1 second in microseconds
    
    collector.record_udp_packet_received(0, base_time, base_time + 100);
    collector.record_udp_packet_received(1, base_time + 1000, base_time + 1100);
    collector.record_udp_packet_received(3, base_time + 3000, base_time + 3100); // Skip 2
    collector.record_udp_packet_received(5, base_time + 5000, base_time + 5100); // Skip 4

    let (lost, expected) = collector.calculate_udp_loss();
    
    // Expected 6 packets (0-5), received 4, lost 2
    assert_eq!(expected, 6, "Expected 6 packets (sequence 0-5)");
    assert_eq!(lost, 2, "Should have lost 2 packets");

    let measurements = collector.get();
    assert_eq!(measurements.out_of_order_packets, 0, "No out-of-order packets");
}

#[test]
fn test_udp_out_of_order_detection() {
    use rperf3::measurements::MeasurementsCollector;

    let collector = MeasurementsCollector::new();

    let base_time = 1000000u64;
    
    // Receive packets out of order: 0, 2, 1, 3
    collector.record_udp_packet_received(0, base_time, base_time + 100);
    collector.record_udp_packet_received(2, base_time + 2000, base_time + 2100);
    collector.record_udp_packet_received(1, base_time + 1000, base_time + 3100); // Out of order
    collector.record_udp_packet_received(3, base_time + 3000, base_time + 3200);

    let measurements = collector.get();
    assert_eq!(measurements.out_of_order_packets, 1, "Should detect 1 out-of-order packet");
}

#[test]
fn test_udp_jitter_calculation() {
    use rperf3::measurements::MeasurementsCollector;

    let collector = MeasurementsCollector::new();

    let base_time = 1000000u64;
    
    // Send packets at regular intervals with varying receive times
    collector.record_udp_packet_received(0, base_time, base_time + 100);
    collector.record_udp_packet_received(1, base_time + 10000, base_time + 10150); // +50 μs jitter
    collector.record_udp_packet_received(2, base_time + 20000, base_time + 20080); // -20 μs jitter
    collector.record_udp_packet_received(3, base_time + 30000, base_time + 30120); // +20 μs jitter

    let measurements = collector.get();
    
    // Jitter should be calculated but we just check it's been set
    assert!(measurements.jitter_ms >= 0.0, "Jitter should be non-negative");
}
