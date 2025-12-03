use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;

/// Connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub socket_fd: Option<i32>,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

/// Test start configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub protocol: String,
    pub num_streams: usize,
    pub blksize: usize,
    pub omit: u64,
    pub duration: u64,
    pub reverse: bool,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub system_info: String,
    pub timestamp: i64,
    pub timestamp_str: String,
}

/// TCP statistics for an interval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpStats {
    pub retransmits: u64,
    pub snd_cwnd: Option<u64>,
    pub rtt: Option<u64>,
    pub rttvar: Option<u64>,
    pub pmtu: Option<u64>,
}

impl Default for TcpStats {
    fn default() -> Self {
        Self {
            retransmits: 0,
            snd_cwnd: None,
            rtt: None,
            rttvar: None,
            pmtu: None,
        }
    }
}

/// UDP statistics for an interval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpStats {
    pub jitter_ms: f64,
    pub lost_packets: u64,
    pub packets: u64,
    pub lost_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out_of_order: Option<u64>,
}

impl Default for UdpStats {
    fn default() -> Self {
        Self {
            jitter_ms: 0.0,
            lost_packets: 0,
            packets: 0,
            lost_percent: 0.0,
            out_of_order: None,
        }
    }
}

/// Enhanced interval statistics with TCP info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedIntervalStats {
    pub socket: Option<i32>,
    pub start: f64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: u64,
    pub bits_per_second: f64,
    #[serde(flatten)]
    pub tcp_stats: TcpStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packets: Option<u64>,
    pub omitted: bool,
    pub sender: bool,
}

/// UDP-specific interval statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpIntervalStats {
    pub socket: Option<i32>,
    pub start: f64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: u64,
    pub bits_per_second: f64,
    pub packets: u64,
    pub omitted: bool,
    pub sender: bool,
}

/// Stream summary for end results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSummary {
    pub socket: Option<i32>,
    pub start: f64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: u64,
    pub bits_per_second: f64,
    pub retransmits: u64,
    pub max_snd_cwnd: Option<u64>,
    pub max_rtt: Option<u64>,
    pub min_rtt: Option<u64>,
    pub mean_rtt: Option<u64>,
    pub sender: bool,
}

/// UDP stream summary for end results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpStreamSummary {
    pub socket: Option<i32>,
    pub start: f64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: u64,
    pub bits_per_second: f64,
    pub jitter_ms: f64,
    pub lost_packets: u64,
    pub packets: u64,
    pub lost_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out_of_order: Option<u64>,
    pub sender: bool,
}

/// UDP sum for end results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpSum {
    pub start: f64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: u64,
    pub bits_per_second: f64,
    pub jitter_ms: f64,
    pub lost_packets: u64,
    pub packets: u64,
    pub lost_percent: f64,
    pub sender: bool,
}

/// CPU utilization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuUtilization {
    pub host_total: f64,
    pub host_user: f64,
    pub host_system: f64,
    pub remote_total: f64,
    pub remote_user: f64,
    pub remote_system: f64,
}

/// Complete test results in iperf3 format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedTestResults {
    pub start: TestStartInfo,
    pub intervals: Vec<IntervalData>,
    pub end: TestEndInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStartInfo {
    pub connected: Vec<ConnectionInfo>,
    pub version: String,
    pub system_info: String,
    pub timestamp: TimestampInfo,
    pub connecting_to: ConnectingTo,
    pub cookie: String,
    pub tcp_mss_default: Option<u32>,
    pub sock_bufsize: u32,
    pub sndbuf_actual: u32,
    pub rcvbuf_actual: u32,
    pub test_start: TestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampInfo {
    pub time: String,
    pub timesecs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectingTo {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntervalData {
    Tcp {
        streams: Vec<DetailedIntervalStats>,
        sum: DetailedIntervalStats,
    },
    Udp {
        streams: Vec<UdpIntervalStats>,
        sum: UdpIntervalStats,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TestEndInfo {
    Tcp {
        streams: Vec<EndStreamInfo>,
        sum_sent: StreamSummary,
        sum_received: StreamSummary,
        #[serde(skip_serializing_if = "Option::is_none")]
        cpu_utilization_percent: Option<CpuUtilization>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sender_tcp_congestion: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        receiver_tcp_congestion: Option<String>,
    },
    Udp {
        streams: Vec<UdpEndStreamInfo>,
        sum: UdpSum,
        #[serde(skip_serializing_if = "Option::is_none")]
        cpu_utilization_percent: Option<CpuUtilization>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndStreamInfo {
    pub sender: StreamSummary,
    pub receiver: StreamSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpEndStreamInfo {
    pub udp: UdpStreamSummary,
}

/// Statistics for a single stream (legacy support)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub stream_id: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub duration: Duration,
    pub retransmits: Option<u64>,
}

impl StreamStats {
    pub fn new(stream_id: usize) -> Self {
        Self {
            stream_id,
            bytes_sent: 0,
            bytes_received: 0,
            duration: Duration::ZERO,
            retransmits: None,
        }
    }

    pub fn bits_per_second(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            (self.bytes_sent as f64 * 8.0) / self.duration.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Interval measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntervalStats {
    pub start: Duration,
    pub end: Duration,
    pub bytes: u64,
    pub bits_per_second: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packets: Option<u64>,
}

/// Complete test measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurements {
    pub streams: Vec<StreamStats>,
    pub intervals: Vec<IntervalStats>,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub total_duration: Duration,
    pub total_packets: u64,
    pub lost_packets: u64,
    pub out_of_order_packets: u64,
    pub jitter_ms: f64,
    #[serde(skip)]
    pub start_time: Option<Instant>,
}

impl Measurements {
    pub fn new() -> Self {
        Self {
            streams: Vec::new(),
            intervals: Vec::new(),
            total_bytes_sent: 0,
            total_bytes_received: 0,
            total_duration: Duration::ZERO,
            total_packets: 0,
            lost_packets: 0,
            out_of_order_packets: 0,
            jitter_ms: 0.0,
            start_time: None,
        }
    }

    pub fn total_bits_per_second(&self) -> f64 {
        if self.total_duration.as_secs_f64() > 0.0 {
            (self.total_bytes_sent as f64 * 8.0) / self.total_duration.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn add_stream(&mut self, stats: StreamStats) {
        self.total_bytes_sent += stats.bytes_sent;
        self.total_bytes_received += stats.bytes_received;
        self.streams.push(stats);
    }

    pub fn add_interval(&mut self, interval: IntervalStats) {
        self.intervals.push(interval);
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.total_duration = duration;
    }

    pub fn set_start_time(&mut self, time: Instant) {
        self.start_time = Some(time);
    }
}

impl Default for Measurements {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe measurements collector
#[derive(Debug, Clone)]
pub struct MeasurementsCollector {
    inner: Arc<Mutex<Measurements>>,
}

impl MeasurementsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Measurements::new())),
        }
    }

    pub fn record_bytes_sent(&self, stream_id: usize, bytes: u64) {
        let mut m = self.inner.lock();
        if let Some(stream) = m.streams.iter_mut().find(|s| s.stream_id == stream_id) {
            stream.bytes_sent += bytes;
        } else {
            let mut stats = StreamStats::new(stream_id);
            stats.bytes_sent = bytes;
            m.streams.push(stats);
        }
        m.total_bytes_sent += bytes;
    }

    pub fn record_bytes_received(&self, stream_id: usize, bytes: u64) {
        let mut m = self.inner.lock();
        if let Some(stream) = m.streams.iter_mut().find(|s| s.stream_id == stream_id) {
            stream.bytes_received += bytes;
        } else {
            let mut stats = StreamStats::new(stream_id);
            stats.bytes_received = bytes;
            m.streams.push(stats);
        }
        m.total_bytes_received += bytes;
    }

    pub fn add_interval(&self, interval: IntervalStats) {
        self.inner.lock().add_interval(interval);
    }

    pub fn record_udp_packet(&self, _stream_id: usize) {
        let mut m = self.inner.lock();
        m.total_packets += 1;
    }

    pub fn record_udp_loss(&self, lost: u64) {
        let mut m = self.inner.lock();
        m.lost_packets += lost;
    }

    pub fn update_jitter(&self, jitter: f64) {
        let mut m = self.inner.lock();
        // Simple exponential moving average
        m.jitter_ms = if m.jitter_ms == 0.0 {
            jitter
        } else {
            m.jitter_ms * 0.875 + jitter * 0.125
        };
    }

    pub fn set_duration(&self, duration: Duration) {
        self.inner.lock().set_duration(duration);
    }

    pub fn set_start_time(&self, time: Instant) {
        self.inner.lock().set_start_time(time);
    }

    pub fn get(&self) -> Measurements {
        self.inner.lock().clone()
    }

    pub fn get_stream_stats(&self, stream_id: usize) -> Option<StreamStats> {
        self.inner
            .lock()
            .streams
            .iter()
            .find(|s| s.stream_id == stream_id)
            .cloned()
    }

    /// Get detailed test results in iperf3 format
    pub fn get_detailed_results(
        &self,
        connection_info: Option<ConnectionInfo>,
        system_info: Option<SystemInfo>,
        test_config: TestConfig,
    ) -> DetailedTestResults {
        let m = self.inner.lock();
        let is_udp = test_config.protocol.to_uppercase() == "UDP";
        
        // Build start info
        let start_info = TestStartInfo {
            connected: connection_info.clone().into_iter().collect(),
            version: format!("rperf3 {}", env!("CARGO_PKG_VERSION")),
            system_info: system_info.as_ref()
                .map(|s| s.system_info.clone())
                .unwrap_or_else(|| {
                    format!("{} {}", 
                        std::env::consts::OS,
                        std::env::consts::ARCH)
                }),
            timestamp: TimestampInfo {
                time: system_info.as_ref()
                    .map(|s| s.timestamp_str.clone())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc2822()),
                timesecs: system_info.as_ref()
                    .map(|s| s.timestamp)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            },
            connecting_to: ConnectingTo {
                host: connection_info.as_ref()
                    .map(|c| c.remote_host.clone())
                    .unwrap_or_default(),
                port: connection_info.as_ref()
                    .map(|c| c.remote_port)
                    .unwrap_or(5201),
            },
            cookie: format!("{:x}", rand::random::<u128>()),
            tcp_mss_default: if is_udp { None } else { Some(1448) },
            sock_bufsize: 0,
            sndbuf_actual: if is_udp { 212992 } else { 16384 },
            rcvbuf_actual: if is_udp { 212992 } else { 131072 },
            test_start: test_config.clone(),
        };

        // Build intervals based on protocol
        let intervals = if is_udp {
            self.build_udp_intervals(&m, &connection_info)
        } else {
            self.build_tcp_intervals(&m, &connection_info)
        };

        // Build end info based on protocol
        let end_info = if is_udp {
            self.build_udp_end_info(&m, &connection_info)
        } else {
            self.build_tcp_end_info(&m, &connection_info)
        };

        DetailedTestResults {
            start: start_info,
            intervals,
            end: end_info,
        }
    }

    fn build_tcp_intervals(
        &self,
        m: &Measurements,
        connection_info: &Option<ConnectionInfo>,
    ) -> Vec<IntervalData> {
        let mut intervals = Vec::new();
        for interval in &m.intervals {
            let stream_stat = DetailedIntervalStats {
                socket: connection_info.as_ref().and_then(|c| c.socket_fd),
                start: interval.start.as_secs_f64(),
                end: interval.end.as_secs_f64(),
                seconds: (interval.end - interval.start).as_secs_f64(),
                bytes: interval.bytes,
                bits_per_second: interval.bits_per_second,
                tcp_stats: TcpStats::default(),
                packets: None,
                omitted: false,
                sender: true,
            };
            
            intervals.push(IntervalData::Tcp {
                streams: vec![stream_stat.clone()],
                sum: stream_stat,
            });
        }
        intervals
    }

    fn build_udp_intervals(
        &self,
        m: &Measurements,
        connection_info: &Option<ConnectionInfo>,
    ) -> Vec<IntervalData> {
        let mut intervals = Vec::new();
        for interval in &m.intervals {
            let stream_stat = UdpIntervalStats {
                socket: connection_info.as_ref().and_then(|c| c.socket_fd),
                start: interval.start.as_secs_f64(),
                end: interval.end.as_secs_f64(),
                seconds: (interval.end - interval.start).as_secs_f64(),
                bytes: interval.bytes,
                bits_per_second: interval.bits_per_second,
                packets: interval.packets.unwrap_or(0),
                omitted: false,
                sender: true,
            };
            
            intervals.push(IntervalData::Udp {
                streams: vec![stream_stat.clone()],
                sum: stream_stat,
            });
        }
        intervals
    }

    fn build_tcp_end_info(
        &self,
        m: &Measurements,
        connection_info: &Option<ConnectionInfo>,
    ) -> TestEndInfo {
        let total_duration = m.total_duration.as_secs_f64();
        let sender_summary = StreamSummary {
            socket: connection_info.as_ref().and_then(|c| c.socket_fd),
            start: 0.0,
            end: total_duration,
            seconds: total_duration,
            bytes: m.total_bytes_sent,
            bits_per_second: m.total_bits_per_second(),
            retransmits: 0,
            max_snd_cwnd: None,
            max_rtt: None,
            min_rtt: None,
            mean_rtt: None,
            sender: true,
        };

        let receiver_summary = StreamSummary {
            socket: connection_info.as_ref().and_then(|c| c.socket_fd),
            start: 0.0,
            end: total_duration,
            seconds: total_duration,
            bytes: m.total_bytes_received,
            bits_per_second: if total_duration > 0.0 {
                (m.total_bytes_received as f64 * 8.0) / total_duration
            } else {
                0.0
            },
            retransmits: 0,
            max_snd_cwnd: None,
            max_rtt: None,
            min_rtt: None,
            mean_rtt: None,
            sender: true,
        };

        TestEndInfo::Tcp {
            streams: vec![EndStreamInfo {
                sender: sender_summary.clone(),
                receiver: receiver_summary.clone(),
            }],
            sum_sent: sender_summary,
            sum_received: receiver_summary,
            cpu_utilization_percent: None,
            sender_tcp_congestion: Some("cubic".to_string()),
            receiver_tcp_congestion: Some("cubic".to_string()),
        }
    }

    fn build_udp_end_info(
        &self,
        m: &Measurements,
        connection_info: &Option<ConnectionInfo>,
    ) -> TestEndInfo {
        let total_duration = m.total_duration.as_secs_f64();
        let lost_percent = if m.total_packets > 0 {
            (m.lost_packets as f64 / m.total_packets as f64) * 100.0
        } else {
            0.0
        };

        let udp_summary = UdpStreamSummary {
            socket: connection_info.as_ref().and_then(|c| c.socket_fd),
            start: 0.0,
            end: total_duration,
            seconds: total_duration,
            bytes: m.total_bytes_sent,
            bits_per_second: m.total_bits_per_second(),
            jitter_ms: m.jitter_ms,
            lost_packets: m.lost_packets,
            packets: m.total_packets,
            lost_percent,
            out_of_order: if m.out_of_order_packets > 0 {
                Some(m.out_of_order_packets)
            } else {
                None
            },
            sender: true,
        };

        let udp_sum = UdpSum {
            start: 0.0,
            end: total_duration,
            seconds: total_duration,
            bytes: m.total_bytes_sent,
            bits_per_second: m.total_bits_per_second(),
            jitter_ms: m.jitter_ms,
            lost_packets: m.lost_packets,
            packets: m.total_packets,
            lost_percent,
            sender: true,
        };

        TestEndInfo::Udp {
            streams: vec![UdpEndStreamInfo {
                udp: udp_summary,
            }],
            sum: udp_sum,
            cpu_utilization_percent: None,
        }
    }
}

impl Default for MeasurementsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions to gather system and connection information

/// Get system information
pub fn get_system_info() -> SystemInfo {
    SystemInfo {
        version: format!("rperf3 {}", env!("CARGO_PKG_VERSION")),
        system_info: format!("{} {} {}", 
            std::env::consts::OS,
            std::env::consts::ARCH,
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string())
        ),
        timestamp_str: chrono::Utc::now().to_rfc2822(),
        timestamp: chrono::Utc::now().timestamp(),
    }
}

/// Get connection information from a TcpStream
#[cfg(target_os = "linux")]
pub fn get_connection_info(stream: &tokio::net::TcpStream) -> std::io::Result<ConnectionInfo> {
    use std::os::unix::io::AsRawFd;
    
    let local_addr = stream.local_addr()?;
    let remote_addr = stream.peer_addr()?;
    let fd = stream.as_raw_fd();
    
    Ok(ConnectionInfo {
        socket_fd: Some(fd),
        local_host: local_addr.ip().to_string(),
        local_port: local_addr.port(),
        remote_host: remote_addr.ip().to_string(),
        remote_port: remote_addr.port(),
    })
}

#[cfg(not(target_os = "linux"))]
pub fn get_connection_info(stream: &tokio::net::TcpStream) -> std::io::Result<ConnectionInfo> {
    let local_addr = stream.local_addr()?;
    let remote_addr = stream.peer_addr()?;
    
    Ok(ConnectionInfo {
        socket_fd: None,
        local_host: local_addr.ip().to_string(),
        local_port: local_addr.port(),
        remote_host: remote_addr.ip().to_string(),
        remote_port: remote_addr.port(),
    })
}

/// Get TCP statistics from a socket (Linux only)
#[cfg(target_os = "linux")]
pub fn get_tcp_stats(stream: &tokio::net::TcpStream) -> std::io::Result<TcpStats> {
    use std::os::unix::io::AsRawFd;
    use std::mem;
    
    let fd = stream.as_raw_fd();
    
    // TCP_INFO structure (simplified, Linux-specific)
    #[repr(C)]
    struct TcpInfo {
        state: u8,
        ca_state: u8,
        retransmits: u8,
        probes: u8,
        backoff: u8,
        options: u8,
        snd_wscale: u8,
        rcv_wscale: u8,
        
        rto: u32,
        ato: u32,
        snd_mss: u32,
        rcv_mss: u32,
        
        unacked: u32,
        sacked: u32,
        lost: u32,
        retrans: u32,
        fackets: u32,
        
        last_data_sent: u32,
        last_ack_sent: u32,
        last_data_recv: u32,
        last_ack_recv: u32,
        
        pmtu: u32,
        rcv_ssthresh: u32,
        rtt: u32,
        rttvar: u32,
        snd_ssthresh: u32,
        snd_cwnd: u32,
        advmss: u32,
        reordering: u32,
        
        rcv_rtt: u32,
        rcv_space: u32,
        
        total_retrans: u32,
    }
    
    const TCP_INFO: i32 = 11;
    const SOL_TCP: i32 = 6;
    
    let mut info: TcpInfo = unsafe { mem::zeroed() };
    let mut len = mem::size_of::<TcpInfo>() as u32;
    
    let result = unsafe {
        libc::getsockopt(
            fd,
            SOL_TCP,
            TCP_INFO,
            &mut info as *mut _ as *mut libc::c_void,
            &mut len as *mut u32,
        )
    };
    
    if result == 0 {
        Ok(TcpStats {
            retransmits: info.total_retrans as u64,
            snd_cwnd: Some(info.snd_cwnd as u64),
            rtt: Some(info.rtt as u64),
            rttvar: Some(info.rttvar as u64),
            pmtu: Some(info.pmtu as u64),
        })
    } else {
        Ok(TcpStats::default())
    }
}

#[cfg(not(target_os = "linux"))]
pub fn get_tcp_stats(_stream: &tokio::net::TcpStream) -> std::io::Result<TcpStats> {
    Ok(TcpStats::default())
}
