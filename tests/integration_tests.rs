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
