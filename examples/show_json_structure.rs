use rperf3::measurements::{
    get_system_info, ConnectionInfo, IntervalStats, MeasurementsCollector, TestConfig,
};
use std::time::Duration;

fn main() {
    // Create a mock measurements collector
    let collector = MeasurementsCollector::new();

    // Add some mock data
    collector.record_bytes_sent(0, 1000000000); // 1GB
    collector.add_interval(IntervalStats {
        start: Duration::from_secs(0),
        end: Duration::from_secs(1),
        bytes: 1000000000,
        bits_per_second: 8000000000.0,
        packets: u64::MAX,
    });
    collector.set_duration(Duration::from_secs(3));

    // Create mock connection info
    let conn_info = Some(ConnectionInfo {
        socket_fd: Some(3),
        local_host: "127.0.0.1".to_string(),
        local_port: 45678,
        remote_host: "127.0.0.1".to_string(),
        remote_port: 5201,
    });

    // Get system info
    let sys_info = Some(get_system_info());

    // Create test config
    let test_config = TestConfig {
        protocol: "Tcp".to_string(),
        num_streams: 1,
        blksize: 131072,
        omit: 0,
        duration: 3,
        reverse: false,
    };

    // Get detailed results
    let results = collector.get_detailed_results(conn_info, sys_info, test_config);

    // Print as JSON
    let json = serde_json::to_string_pretty(&results).unwrap();
    println!("{}", json);
}
