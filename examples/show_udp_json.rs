use rperf3::measurements::{
    get_system_info, ConnectionInfo, TestConfig, MeasurementsCollector, IntervalStats,
};
use std::time::Duration;

fn main() {
    // Create a mock measurements collector for UDP
    let collector = MeasurementsCollector::new();
    
    // Simulate UDP test data
    collector.record_bytes_sent(0, 131768);
    collector.record_udp_packet(0);
    collector.record_udp_packet(0);
    collector.record_udp_packet(0);
    
    // Add mock intervals
    for i in 0..3 {
        collector.add_interval(IntervalStats {
            start: Duration::from_secs(i),
            end: Duration::from_secs(i + 1),
            bytes: 131768,
            bits_per_second: 1054144.0,
            packets: Some(91),
        });
    }
    
    collector.set_duration(Duration::from_secs(3));
    collector.update_jitter(1.5);
    
    // Create mock connection info
    let conn_info = Some(ConnectionInfo {
        socket_fd: Some(5),
        local_host: "192.168.1.12".to_string(),
        local_port: 34497,
        remote_host: "192.168.1.100".to_string(),
        remote_port: 5201,
    });
    
    // Get system info
    let sys_info = Some(get_system_info());
    
    // Create test config for UDP
    let test_config = TestConfig {
        protocol: "UDP".to_string(),
        num_streams: 1,
        blksize: 1448,
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
