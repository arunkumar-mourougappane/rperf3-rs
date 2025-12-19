use parking_lot::Mutex;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_INTERVALS: usize = 86400; // 24 hours at 1s interval

/// Connection information for a test stream.
///
/// Contains details about the local and remote endpoints of a TCP connection.
/// This information is collected at test start and included in detailed results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub socket_fd: Option<i32>,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

/// Test configuration parameters at test start.
///
/// Records the test parameters that were negotiated between client and server.
/// This is included in detailed test results for reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub protocol: String,
    pub num_streams: usize,
    pub blksize: usize,
    pub omit: u64,
    pub duration: u64,
    pub reverse: bool,
}

/// System information for test environment.
///
/// Contains version information and system details about the host running the test.
/// Useful for correlating performance with hardware/software configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub system_info: String,
    pub timestamp: i64,
    pub timestamp_str: String,
}

const TCP_SND_CWND_PRESENT: u8 = 1 << 0;
const TCP_RTT_PRESENT: u8 = 1 << 1;
const TCP_RTTVAR_PRESENT: u8 = 1 << 2;
const TCP_PMTU_PRESENT: u8 = 1 << 3;

mod option_u64_max {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if *value == u64::MAX {
            serializer.serialize_none()
        } else {
            serializer.serialize_some(value)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<u64> = Option::deserialize(deserializer)?;
        Ok(opt.unwrap_or(u64::MAX))
    }
}

/// TCP-specific statistics for a measurement interval.
///
/// Contains TCP protocol information collected from the socket during the test.
/// These statistics are platform-specific and may not be available on all systems.
/// On Linux, these values are read from `/proc/net/tcp` and socket options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(from = "TcpStatsDto")]
pub struct TcpStats {
    pub retransmits: u64,
    pub snd_cwnd: u64,
    pub rtt: u64,
    pub rttvar: u64,
    pub pmtu: u64,
    #[serde(skip)]
    pub flags: u8,
}

#[derive(Deserialize)]
struct TcpStatsDto {
    #[serde(default)]
    retransmits: u64,
    snd_cwnd: Option<u64>,
    rtt: Option<u64>,
    rttvar: Option<u64>,
    pmtu: Option<u64>,
}

impl From<TcpStatsDto> for TcpStats {
    fn from(dto: TcpStatsDto) -> Self {
        let mut flags = 0;
        let snd_cwnd = if let Some(v) = dto.snd_cwnd {
            flags |= TCP_SND_CWND_PRESENT;
            v
        } else {
            0
        };
        let rtt = if let Some(v) = dto.rtt {
            flags |= TCP_RTT_PRESENT;
            v
        } else {
            0
        };
        let rttvar = if let Some(v) = dto.rttvar {
            flags |= TCP_RTTVAR_PRESENT;
            v
        } else {
            0
        };
        let pmtu = if let Some(v) = dto.pmtu {
            flags |= TCP_PMTU_PRESENT;
            v
        } else {
            0
        };

        Self {
            retransmits: dto.retransmits,
            snd_cwnd,
            rtt,
            rttvar,
            pmtu,
            flags,
        }
    }
}

impl Serialize for TcpStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("TcpStats", 5)?;
        state.serialize_field("retransmits", &self.retransmits)?;

        if self.flags & TCP_SND_CWND_PRESENT != 0 {
            state.serialize_field("snd_cwnd", &self.snd_cwnd)?;
        } else {
            state.serialize_field("snd_cwnd", &None::<u64>)?;
        }

        if self.flags & TCP_RTT_PRESENT != 0 {
            state.serialize_field("rtt", &self.rtt)?;
        } else {
            state.serialize_field("rtt", &None::<u64>)?;
        }

        if self.flags & TCP_RTTVAR_PRESENT != 0 {
            state.serialize_field("rttvar", &self.rttvar)?;
        } else {
            state.serialize_field("rttvar", &None::<u64>)?;
        }

        if self.flags & TCP_PMTU_PRESENT != 0 {
            state.serialize_field("pmtu", &self.pmtu)?;
        } else {
            state.serialize_field("pmtu", &None::<u64>)?;
        }

        state.end()
    }
}

impl TcpStats {
    pub fn snd_cwnd_opt(&self) -> Option<u64> {
        if self.flags & TCP_SND_CWND_PRESENT != 0 {
            Some(self.snd_cwnd)
        } else {
            None
        }
    }
    pub fn rtt_opt(&self) -> Option<u64> {
        if self.flags & TCP_RTT_PRESENT != 0 {
            Some(self.rtt)
        } else {
            None
        }
    }
    pub fn rttvar_opt(&self) -> Option<u64> {
        if self.flags & TCP_RTTVAR_PRESENT != 0 {
            Some(self.rttvar)
        } else {
            None
        }
    }
    pub fn pmtu_opt(&self) -> Option<u64> {
        if self.flags & TCP_PMTU_PRESENT != 0 {
            Some(self.pmtu)
        } else {
            None
        }
    }
}

/// UDP-specific statistics for a measurement interval.
///
/// Contains UDP protocol information including packet loss and jitter measurements.
/// Jitter is calculated using the RFC 3550 algorithm. Packet loss is determined by
/// tracking sequence number gaps in received packets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpStats {
    pub jitter_ms: f64,
    pub lost_packets: u64,
    pub packets: u64,
    pub lost_percent: f64,
    #[serde(default, with = "option_u64_max")]
    pub out_of_order: u64,
}

impl Default for UdpStats {
    fn default() -> Self {
        Self {
            jitter_ms: 0.0,
            lost_packets: 0,
            packets: 0,
            lost_percent: 0.0,
            out_of_order: u64::MAX,
        }
    }
}

/// Enhanced interval statistics with TCP-specific information.
///
/// Extends basic interval statistics with detailed TCP metrics like retransmits,
/// congestion window, and RTT measurements. Used for detailed JSON output format.
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

/// UDP-specific interval statistics for detailed reporting.
///
/// Contains per-interval UDP measurements without the TCP-specific fields.
/// Used in detailed JSON output format for UDP tests.
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

/// Stream summary statistics for TCP test completion.
///
/// Aggregates all statistics for a single TCP stream over the entire test duration.
/// Includes cumulative values (total bytes, retransmits) and statistical values
/// (min/max/mean RTT, max congestion window).
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

/// UDP stream summary statistics for test completion.
///
/// Aggregates all UDP-specific statistics for a stream over the entire test duration,
/// including packet counts, loss percentage, jitter, and out-of-order packets.
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

/// Aggregate UDP statistics across all streams.
///
/// Sums up statistics from all parallel UDP streams to provide overall test results.
/// Used in detailed JSON output format.
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

/// CPU utilization statistics for performance analysis.
///
/// Tracks CPU usage on both local and remote hosts during the test.
/// Helps identify whether CPU is a bottleneck for network performance.
/// Values are percentages (0-100 per core, can exceed 100 on multi-core systems).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuUtilization {
    pub host_total: f64,
    pub host_user: f64,
    pub host_system: f64,
    pub remote_total: f64,
    pub remote_user: f64,
    pub remote_system: f64,
}

/// Complete test results in iperf3-compatible JSON format.
///
/// This structure provides detailed test results that can be exported to JSON
/// for compatibility with iperf3 tooling and parsers. It includes all test
/// parameters, interval data, and final statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedTestResults {
    pub start: TestStartInfo,
    pub intervals: Vec<IntervalData>,
    pub end: TestEndInfo,
}

/// Test start information including all configuration and connection details.
///
/// Captures the complete state at test initialization, including negotiated parameters,
/// connection information, and system details. This is the `start` section in detailed
/// JSON output.
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

/// Timestamp information for test events.
///
/// Provides both human-readable and machine-readable timestamp formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampInfo {
    pub time: String,
    pub timesecs: i64,
}

/// Server connection target information.
///
/// Records the server address and port that the client connected to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectingTo {
    pub host: String,
    pub port: u16,
}

/// Interval measurement data, protocol-specific.
///
/// Discriminated union containing either TCP or UDP interval statistics.
/// The format varies based on the protocol used for the test.
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

/// Test completion information, protocol-specific.
///
/// Discriminated union containing final test statistics for either TCP or UDP.
/// TCP results include separate sender/receiver statistics, while UDP includes
/// aggregated loss and jitter information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TestEndInfo {
    Tcp {
        streams: Vec<EndStreamInfo>,
        sum_sent: Box<StreamSummary>,
        sum_received: Box<StreamSummary>,
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

/// Per-stream final statistics for TCP.
///
/// Contains both sender-side and receiver-side statistics for a single TCP stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndStreamInfo {
    pub sender: StreamSummary,
    pub receiver: StreamSummary,
}

/// Per-stream final statistics for UDP.
///
/// Wraps UDP stream summary in a structure compatible with detailed JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdpEndStreamInfo {
    pub udp: UdpStreamSummary,
}

/// Statistics for a single stream (simplified format).
///
/// Provides basic per-stream statistics in a simplified format. This is used
/// for the default output format and backwards compatibility.
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
            let total_bytes = self.bytes_sent + self.bytes_received;
            (total_bytes as f64 * 8.0) / self.duration.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Interval measurement in simplified format.
///
/// Contains basic statistics for a single reporting interval. Used for
/// standard (non-JSON) output and progress callbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntervalStats {
    pub start: Duration,
    pub end: Duration,
    pub bytes: u64,
    pub bits_per_second: f64,
    #[serde(default, with = "option_u64_max")]
    pub packets: u64,
}

/// Complete test measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Performance test measurements and statistics.
///
/// This structure holds all collected statistics from a network performance test,
/// including per-stream data, interval statistics, aggregate totals, and UDP-specific
/// metrics like packet loss and jitter.
///
/// # UDP Metrics
///
/// For UDP tests, additional metrics are tracked:
/// - `total_packets`: Number of packets sent or expected based on sequence numbers
/// - `lost_packets`: Number of packets lost (receiver-side measurement)
/// - `out_of_order_packets`: Packets received out of sequence
/// - `jitter_ms`: Network jitter in milliseconds (RFC 3550 algorithm)
///
/// # Examples
///
/// ```
/// use rperf3::Measurements;
///
/// let measurements = Measurements::new();
///
/// // After test completion
/// let throughput_mbps = measurements.total_bits_per_second() / 1_000_000.0;
/// println!("Average throughput: {:.2} Mbps", throughput_mbps);
/// println!("Total transferred: {} bytes",
///          measurements.total_bytes_sent + measurements.total_bytes_received);
///
/// // UDP-specific metrics
/// if measurements.total_packets > 0 {
///     let loss_percent = (measurements.lost_packets as f64 /
///                         measurements.total_packets as f64) * 100.0;
///     println!("Packet loss: {:.2}%", loss_percent);
///     println!("Jitter: {:.3} ms", measurements.jitter_ms);
/// }
/// ```
pub struct Measurements {
    pub streams: Vec<StreamStats>,
    pub intervals: VecDeque<IntervalStats>,
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
    /// Creates a new empty measurements collection.
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Measurements;
    ///
    /// let measurements = Measurements::new();
    /// assert_eq!(measurements.total_bytes_sent, 0);
    /// ```
    pub fn new() -> Self {
        Self {
            streams: Vec::new(),
            intervals: VecDeque::new(),
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

    /// Creates a new measurements collection with pre-allocated capacity.
    ///
    /// Pre-allocating capacity reduces memory allocations during test execution.
    /// Use this when you know the expected number of streams and intervals.
    ///
    /// # Arguments
    ///
    /// * `expected_streams` - Expected number of parallel streams
    /// * `expected_intervals` - Expected number of interval reports
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Measurements;
    ///
    /// // For a 10-second test with 1-second intervals and 4 streams
    /// let measurements = Measurements::with_capacity(4, 10);
    /// ```
    pub fn with_capacity(expected_streams: usize, expected_intervals: usize) -> Self {
        Self {
            streams: Vec::with_capacity(expected_streams),
            intervals: VecDeque::with_capacity(expected_intervals),
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

    /// Calculates the average throughput in bits per second.
    ///
    /// Returns the total throughput based on both bytes sent and received,
    /// accounting for both normal and reverse mode tests.
    ///
    /// # Returns
    ///
    /// Throughput in bits per second, or 0.0 if duration is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::Measurements;
    /// use std::time::Duration;
    ///
    /// let mut measurements = Measurements::new();
    /// measurements.total_bytes_sent = 100_000_000; // 100 MB sent
    /// measurements.total_bytes_received = 25_000_000; // 25 MB received
    /// measurements.set_duration(Duration::from_secs(10));
    ///
    /// let throughput = measurements.total_bits_per_second();
    /// assert_eq!(throughput, 100_000_000.0); // 100 Mbps total
    /// ```
    pub fn total_bits_per_second(&self) -> f64 {
        if self.total_duration.as_secs_f64() > 0.0 {
            let total_bytes = self.total_bytes_sent + self.total_bytes_received;
            (total_bytes as f64 * 8.0) / self.total_duration.as_secs_f64()
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
        if self.intervals.len() >= MAX_INTERVALS {
            self.intervals.pop_front();
        }
        self.intervals.push_back(interval);
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.total_duration = duration;
        // Also update all stream durations
        for stream in &mut self.streams {
            stream.duration = duration;
        }
    }

    pub fn set_start_time(&mut self, time: Instant) {
        self.start_time = Some(time);
    }

    /// Calculates UDP packet loss based on sequence numbers.
    ///
    /// This should only be called when receiving UDP packets.
    /// Returns (lost_packets, expected_packets).
    pub fn calculate_udp_loss(&self) -> (u64, u64) {
        // For the snapshot, just return the pre-calculated values
        // The actual calculation happens in MeasurementsCollector
        (self.lost_packets, self.total_packets)
    }
}

impl Default for Measurements {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-stream measurements with atomic counters for lock-free updates.
///
/// This structure maintains counters for a single stream using atomic operations,
/// eliminating lock contention when multiple streams update in parallel.
#[derive(Debug)]
struct PerStreamMeasurements {
    stream_id: usize,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    packets: AtomicU64,
}

impl PerStreamMeasurements {
    fn new(stream_id: usize) -> Self {
        Self {
            stream_id,
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            packets: AtomicU64::new(0),
        }
    }

    fn to_stream_stats(&self, duration: Duration) -> StreamStats {
        StreamStats {
            stream_id: self.stream_id,
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            duration,
            retransmits: None,
        }
    }
}

/// Thread-safe measurements collector for concurrent access.
///
/// This collector wraps `Measurements` in thread-safe structures to allow multiple
/// streams or async tasks to record data concurrently. It also maintains UDP packet
/// tracking state for jitter and loss calculations.
///
/// Per-stream measurements use atomic counters for lock-free updates, reducing
/// contention in parallel stream scenarios.
///
/// # Examples
///
/// ```
/// use rperf3::measurements::MeasurementsCollector;
///
/// let collector = MeasurementsCollector::new();
/// collector.record_bytes_sent(0, 1024);
///
/// let measurements = collector.get();
/// assert_eq!(measurements.total_bytes_sent, 1024);
/// ```
#[derive(Debug)]
pub struct MeasurementsCollector {
    inner: Arc<Mutex<Measurements>>,
    udp_state: Arc<Mutex<UdpPacketState>>,
    // Per-stream atomic counters for lock-free updates
    per_stream: Arc<Mutex<Vec<Arc<PerStreamMeasurements>>>>,
    // Atomic counters for high-frequency updates
    atomic_bytes_sent: Arc<AtomicU64>,
    atomic_bytes_received: Arc<AtomicU64>,
    atomic_packets: Arc<AtomicU64>,
}

impl Clone for MeasurementsCollector {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            udp_state: Arc::clone(&self.udp_state),
            per_stream: Arc::clone(&self.per_stream),
            atomic_bytes_sent: Arc::clone(&self.atomic_bytes_sent),
            atomic_bytes_received: Arc::clone(&self.atomic_bytes_received),
            atomic_packets: Arc::clone(&self.atomic_packets),
        }
    }
}

/// UDP packet tracking state
#[derive(Debug, Clone)]
struct UdpPacketState {
    /// Last received sequence number
    last_sequence: Option<u64>,
    /// Highest sequence number seen (None if no packets received yet)
    max_sequence: Option<u64>,
    /// Count of received packets
    received_count: u64,
    /// Last packet arrival time in microseconds
    last_arrival_us: Option<u64>,
    /// Last packet send timestamp in microseconds
    last_send_timestamp_us: Option<u64>,
    /// Current jitter estimate in milliseconds (RFC 3550)
    jitter_ms: f64,
    /// Count of out-of-order packets
    out_of_order: u64,
}

impl Default for UdpPacketState {
    fn default() -> Self {
        Self {
            last_sequence: None,
            max_sequence: None,
            received_count: 0,
            last_arrival_us: None,
            last_send_timestamp_us: None,
            jitter_ms: 0.0,
            out_of_order: 0,
        }
    }
}

impl MeasurementsCollector {
    /// Creates a new measurements collector.
    ///
    /// Initializes an empty collector with no recorded data.
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::measurements::MeasurementsCollector;
    ///
    /// let collector = MeasurementsCollector::new();
    /// ```
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Measurements::new())),
            udp_state: Arc::new(Mutex::new(UdpPacketState::default())),
            per_stream: Arc::new(Mutex::new(Vec::new())),
            atomic_bytes_sent: Arc::new(AtomicU64::new(0)),
            atomic_bytes_received: Arc::new(AtomicU64::new(0)),
            atomic_packets: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Creates a new measurements collector with pre-allocated capacity.
    ///
    /// Pre-allocates capacity for measurements to reduce allocations during test execution.
    /// This is particularly beneficial for tests with known parameters.
    ///
    /// # Arguments
    ///
    /// * `expected_streams` - Expected number of parallel streams
    /// * `expected_intervals` - Expected number of interval reports
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::measurements::MeasurementsCollector;
    ///
    /// // For a 30-second test with 1-second intervals and 4 streams
    /// let collector = MeasurementsCollector::with_capacity(4, 30);
    /// ```
    pub fn with_capacity(expected_streams: usize, expected_intervals: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Measurements::with_capacity(
                expected_streams,
                expected_intervals,
            ))),
            udp_state: Arc::new(Mutex::new(UdpPacketState::default())),
            per_stream: Arc::new(Mutex::new(Vec::with_capacity(expected_streams))),
            atomic_bytes_sent: Arc::new(AtomicU64::new(0)),
            atomic_bytes_received: Arc::new(AtomicU64::new(0)),
            atomic_packets: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Gets or creates per-stream measurements for a specific stream ID.
    ///
    /// Returns an Arc to the per-stream measurements, allowing lock-free updates.
    fn get_or_create_stream(&self, stream_id: usize) -> Arc<PerStreamMeasurements> {
        let mut streams = self.per_stream.lock();

        // Check if stream already exists
        if let Some(stream) = streams.iter().find(|s| s.stream_id == stream_id) {
            return Arc::clone(stream);
        }

        // Create new stream
        let stream = Arc::new(PerStreamMeasurements::new(stream_id));
        streams.push(Arc::clone(&stream));
        stream
    }

    /// Records bytes sent on a specific stream.
    ///
    /// Updates both per-stream and total byte counts using lock-free atomic operations.
    /// This eliminates lock contention when multiple streams send data in parallel.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    /// * `bytes` - Number of bytes sent
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::measurements::MeasurementsCollector;
    ///
    /// let collector = MeasurementsCollector::new();
    /// collector.record_bytes_sent(0, 1024);
    /// ```
    pub fn record_bytes_sent(&self, stream_id: usize, bytes: u64) {
        // Use atomic for total count (lock-free, high performance)
        self.atomic_bytes_sent.fetch_add(bytes, Ordering::Relaxed);

        // Use atomic for per-stream count (lock-free)
        let stream = self.get_or_create_stream(stream_id);
        stream.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Records bytes received on a specific stream.
    ///
    /// Updates both per-stream and total byte counts using lock-free atomic operations.
    /// This eliminates lock contention when multiple streams receive data in parallel.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    /// * `bytes` - Number of bytes received
    pub fn record_bytes_received(&self, stream_id: usize, bytes: u64) {
        // Use atomic for total count (lock-free, high performance)
        self.atomic_bytes_received
            .fetch_add(bytes, Ordering::Relaxed);

        // Use atomic for per-stream count (lock-free)
        let stream = self.get_or_create_stream(stream_id);
        stream.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Adds an interval measurement.
    ///
    /// Records statistics for a specific time interval during the test.
    ///
    /// # Arguments
    ///
    /// * `interval` - The interval statistics to record
    pub fn add_interval(&self, interval: IntervalStats) {
        self.inner.lock().add_interval(interval);
    }

    /// Records that a UDP packet was sent.
    ///
    /// Uses lock-free atomic operations for high-performance packet counting.
    /// This eliminates lock contention at high packet rates, providing significant
    /// performance benefits for UDP throughput tests.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    ///
    /// # Performance
    ///
    /// This is a lock-free operation using `AtomicU64::fetch_add`, which is
    /// critical for UDP tests at high packet rates (>1M packets/sec).
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::measurements::MeasurementsCollector;
    ///
    /// let collector = MeasurementsCollector::new();
    /// collector.record_udp_packet(0);
    /// collector.record_udp_packet(0);
    /// collector.sync_atomic_counters();
    ///
    /// let measurements = collector.get();
    /// assert_eq!(measurements.total_packets, 2);
    /// ```
    pub fn record_udp_packet(&self, stream_id: usize) {
        // Use atomic for packet count (lock-free, very high frequency operation)
        self.atomic_packets.fetch_add(1, Ordering::Relaxed);

        // Use atomic for per-stream packet count (lock-free)
        let stream = self.get_or_create_stream(stream_id);
        stream.packets.fetch_add(1, Ordering::Relaxed);
    }

    /// Syncs atomic counters to the internal measurements struct.
    ///
    /// This should be called periodically (e.g., during interval reporting)
    /// to ensure the locked measurements struct reflects current atomic values.
    /// The atomic counters provide lock-free updates for high-frequency operations,
    /// while the locked struct is used for less frequent reads and complex operations.
    pub fn sync_atomic_counters(&self) {
        let mut m = self.inner.lock();
        m.total_bytes_sent = self.atomic_bytes_sent.load(Ordering::Relaxed);
        m.total_bytes_received = self.atomic_bytes_received.load(Ordering::Relaxed);
        m.total_packets = self.atomic_packets.load(Ordering::Relaxed);
    }

    /// Records a received UDP packet with sequence number and timestamp
    ///
    /// Tracks packet loss, out-of-order delivery, and calculates jitter
    /// according to RFC 3550 (RTP).
    ///
    /// # Arguments
    ///
    /// * `sequence` - Packet sequence number
    /// * `send_timestamp_us` - Send timestamp from packet header (microseconds)
    /// * `recv_timestamp_us` - Receive timestamp (microseconds)
    pub fn record_udp_packet_received(
        &self,
        sequence: u64,
        send_timestamp_us: u64,
        recv_timestamp_us: u64,
    ) {
        // Ignore initialization packets (sequence == u64::MAX)
        if sequence == u64::MAX {
            return;
        }

        let mut state = self.udp_state.lock();
        let mut m = self.inner.lock();

        // Track received count
        state.received_count += 1;

        // Track highest sequence number seen
        match state.max_sequence {
            None => state.max_sequence = Some(sequence),
            Some(max) if sequence > max => state.max_sequence = Some(sequence),
            _ => {}
        }

        // Detect out-of-order packets
        if let Some(last_seq) = state.last_sequence {
            if sequence < last_seq {
                state.out_of_order += 1;
                m.out_of_order_packets += 1;
            }
        }

        // Calculate jitter using RFC 3550 algorithm
        // J(i) = J(i-1) + (|D(i-1,i)| - J(i-1))/16
        // where D(i-1,i) is the difference in relative transit times
        // Transit time = receive_time - send_time
        if let (Some(last_arrival), Some(last_send)) =
            (state.last_arrival_us, state.last_send_timestamp_us)
        {
            // Current transit time
            let current_transit = recv_timestamp_us.saturating_sub(send_timestamp_us);
            // Previous transit time
            let previous_transit = last_arrival.saturating_sub(last_send);
            // Transit time difference
            let transit_delta = current_transit.abs_diff(previous_transit);

            // Update jitter using RFC 3550 formula (in microseconds)
            state.jitter_ms = state.jitter_ms + (transit_delta as f64 - state.jitter_ms) / 16.0;
            // Store as milliseconds in measurements
            m.jitter_ms = state.jitter_ms / 1000.0;
        }

        state.last_sequence = Some(sequence);
        state.last_arrival_us = Some(recv_timestamp_us);
        state.last_send_timestamp_us = Some(send_timestamp_us);
    }

    /// Calculates packet loss statistics
    ///
    /// Returns (lost_packets, total_expected_packets)
    /// Calculates UDP packet loss based on sequence numbers.
    ///
    /// Compares received packets against expected sequence range to determine loss.
    ///
    /// # Returns
    ///
    /// A tuple of `(lost_packets, expected_packets)`:
    /// - `lost_packets` - Number of packets that were not received
    /// - `expected_packets` - Total number of packets that should have been received
    pub fn calculate_udp_loss(&self) -> (u64, u64) {
        let state = self.udp_state.lock();

        // If no packets received yet, return 0 loss
        let max_seq = match state.max_sequence {
            Some(max) => max,
            None => return (0, 0),
        };

        // Expected packets is max sequence + 1 (sequences start at 0)
        let expected = max_seq + 1;

        // Lost packets = expected - received
        let received = state.received_count;
        let lost = expected.saturating_sub(received);

        (lost, expected)
    }

    /// Records UDP packet loss count.
    ///
    /// # Arguments
    ///
    /// * `lost` - Number of lost packets
    pub fn record_udp_loss(&self, lost: u64) {
        let mut m = self.inner.lock();
        m.lost_packets += lost;
    }

    /// Updates the jitter measurement.
    ///
    /// # Arguments
    ///
    /// * `jitter` - Jitter value in milliseconds (calculated via RFC 3550)
    pub fn update_jitter(&self, jitter: f64) {
        let mut m = self.inner.lock();
        // Simple exponential moving average
        m.jitter_ms = if m.jitter_ms == 0.0 {
            jitter
        } else {
            m.jitter_ms * 0.875 + jitter * 0.125
        };
    }

    /// Sets the total test duration.
    ///
    /// # Arguments
    ///
    /// * `duration` - The actual test duration
    pub fn set_duration(&self, duration: Duration) {
        self.inner.lock().set_duration(duration);
    }

    /// Sets the test start time.
    ///
    /// # Arguments
    ///
    /// * `time` - The instant when the test started
    pub fn set_start_time(&self, time: Instant) {
        self.inner.lock().set_start_time(time);
    }

    /// Returns a snapshot of the current measurements.
    ///
    /// Creates a copy of all collected statistics at the current point in time.
    /// Syncs all atomic counters (both global and per-stream) into the result.
    ///
    /// # Returns
    ///
    /// A `Measurements` struct containing all collected data.
    ///
    /// # Examples
    ///
    /// ```
    /// use rperf3::measurements::MeasurementsCollector;
    ///
    /// let collector = MeasurementsCollector::new();
    /// collector.record_bytes_sent(0, 1024);
    ///
    /// let measurements = collector.get();
    /// assert_eq!(measurements.total_bytes_sent, 1024);
    /// ```
    pub fn get(&self) -> Measurements {
        let mut m = self.inner.lock().clone();

        // Sync atomic counters into the measurements struct
        m.total_bytes_sent = self.atomic_bytes_sent.load(Ordering::Relaxed);
        m.total_bytes_received = self.atomic_bytes_received.load(Ordering::Relaxed);
        m.total_packets = self.atomic_packets.load(Ordering::Relaxed);

        // Sync per-stream atomic counters
        let streams = self.per_stream.lock();
        m.streams.clear();
        for stream in streams.iter() {
            m.streams.push(stream.to_stream_stats(m.total_duration));
        }
        drop(streams);

        // Calculate UDP loss if we received packets
        if m.total_bytes_received > 0 {
            let (lost, expected) = self.calculate_udp_loss();
            m.lost_packets = lost;
            m.total_packets = expected;
        }

        m
    }

    /// Gets statistics for a specific stream.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier to retrieve
    ///
    /// # Returns
    ///
    /// `Some(StreamStats)` if the stream exists, `None` otherwise.
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
            system_info: system_info
                .as_ref()
                .map(|s| s.system_info.clone())
                .unwrap_or_else(|| format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)),
            timestamp: TimestampInfo {
                time: system_info
                    .as_ref()
                    .map(|s| s.timestamp_str.clone())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc2822()),
                timesecs: system_info
                    .as_ref()
                    .map(|s| s.timestamp)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            },
            connecting_to: ConnectingTo {
                host: connection_info
                    .as_ref()
                    .map(|c| c.remote_host.clone())
                    .unwrap_or_default(),
                port: connection_info
                    .as_ref()
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
        let mut intervals = Vec::with_capacity(m.intervals.len());
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
        let mut intervals = Vec::with_capacity(m.intervals.len());
        for interval in &m.intervals {
            let stream_stat = UdpIntervalStats {
                socket: connection_info.as_ref().and_then(|c| c.socket_fd),
                start: interval.start.as_secs_f64(),
                end: interval.end.as_secs_f64(),
                seconds: (interval.end - interval.start).as_secs_f64(),
                bytes: interval.bytes,
                bits_per_second: interval.bits_per_second,
                packets: if interval.packets == u64::MAX {
                    0
                } else {
                    interval.packets
                },
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
            sum_sent: Box::new(sender_summary),
            sum_received: Box::new(receiver_summary),
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

        // Calculate packet loss from sequence tracking
        let (lost_packets, expected_packets) = self.calculate_udp_loss();

        let lost_percent = if expected_packets > 0 {
            (lost_packets as f64 / expected_packets as f64) * 100.0
        } else {
            0.0
        };

        let udp_summary = UdpStreamSummary {
            socket: connection_info.as_ref().and_then(|c| c.socket_fd),
            start: 0.0,
            end: total_duration,
            seconds: total_duration,
            bytes: m.total_bytes_sent + m.total_bytes_received,
            bits_per_second: if total_duration > 0.0 {
                ((m.total_bytes_sent + m.total_bytes_received) as f64 * 8.0) / total_duration
            } else {
                0.0
            },
            jitter_ms: m.jitter_ms,
            lost_packets,
            packets: expected_packets,
            lost_percent,
            out_of_order: if m.out_of_order_packets > 0 {
                Some(m.out_of_order_packets)
            } else {
                None
            },
            sender: m.total_bytes_sent > m.total_bytes_received,
        };

        let udp_sum = UdpSum {
            start: 0.0,
            end: total_duration,
            seconds: total_duration,
            bytes: m.total_bytes_sent + m.total_bytes_received,
            bits_per_second: if total_duration > 0.0 {
                ((m.total_bytes_sent + m.total_bytes_received) as f64 * 8.0) / total_duration
            } else {
                0.0
            },
            jitter_ms: m.jitter_ms,
            lost_packets,
            packets: expected_packets,
            lost_percent,
            sender: m.total_bytes_sent > m.total_bytes_received,
        };

        TestEndInfo::Udp {
            streams: vec![UdpEndStreamInfo { udp: udp_summary }],
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

/// Retrieves system information for the current host.
///
/// Collects version, OS, architecture, hostname, and timestamp information.
/// This is used in test output to document the environment where tests were run.
///
/// # Returns
///
/// A `SystemInfo` struct containing version, system details, and timestamp.
///
/// # Examples
///
/// ```
/// use rperf3::measurements::get_system_info;
///
/// let info = get_system_info();
/// println!("Running on: {}", info.system_info);
/// println!("Version: {}", info.version);
/// ```
pub fn get_system_info() -> SystemInfo {
    SystemInfo {
        version: format!("rperf3 {}", env!("CARGO_PKG_VERSION")),
        system_info: format!(
            "{} {} {}",
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

/// Retrieves connection information from a TCP stream (Linux).
///
/// On Linux, this function extracts the file descriptor and connection details.
/// The file descriptor is used for retrieving TCP statistics via socket options.
///
/// # Arguments
///
/// * `stream` - The TCP stream to extract connection information from
///
/// # Returns
///
/// A `ConnectionInfo` struct with socket FD, local/remote addresses and ports.
///
/// # Errors
///
/// Returns an error if socket addresses cannot be retrieved.
///
/// # Examples
///
/// ```no_run
/// use tokio::net::TcpStream;
/// use rperf3::measurements::get_connection_info;
///
/// # #[tokio::main]
/// # async fn main() -> std::io::Result<()> {
/// let stream = TcpStream::connect("127.0.0.1:5201").await?;
/// let info = get_connection_info(&stream)?;
/// println!("Connected: {}:{} -> {}:{}",
///          info.local_host, info.local_port,
///          info.remote_host, info.remote_port);
/// # Ok(())
/// # }
/// ```
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

/// Retrieves connection information from a TCP stream (non-Linux platforms).
///
/// On non-Linux platforms, this function extracts connection details but does not
/// provide the file descriptor (returned as `None`).
///
/// # Arguments
///
/// * `stream` - The TCP stream to extract connection information from
///
/// # Returns
///
/// A `ConnectionInfo` struct with local/remote addresses and ports.
/// The `socket_fd` field will be `None`.
///
/// # Errors
///
/// Returns an error if socket addresses cannot be retrieved.
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

/// Retrieves TCP statistics from a socket (Linux only).
///
/// Uses the Linux `TCP_INFO` socket option to extract detailed TCP statistics
/// including retransmits, congestion window, RTT, RTT variance, and PMTU.
///
/// This information is valuable for diagnosing network performance issues and
/// understanding TCP behavior during tests.
///
/// # Arguments
///
/// * `stream` - The TCP stream to extract statistics from
///
/// # Returns
///
/// A `TcpStats` struct with retransmit count, congestion window, RTT metrics,
/// and path MTU. Returns default (zero) values if statistics cannot be retrieved.
///
/// # Platform Support
///
/// This function only provides meaningful data on Linux. On other platforms,
/// use the non-Linux version which returns default values.
///
/// # Examples
///
/// ```no_run
/// use tokio::net::TcpStream;
/// use rperf3::measurements::get_tcp_stats;
///
/// # #[tokio::main]
/// # async fn main() -> std::io::Result<()> {
/// let stream = TcpStream::connect("127.0.0.1:5201").await?;
/// let stats = get_tcp_stats(&stream)?;
///
/// if let Some(cwnd) = stats.snd_cwnd_opt() {
///     println!("Congestion window: {} bytes", cwnd);
/// }
/// if let Some(rtt) = stats.rtt_opt() {
///     println!("RTT: {} μs", rtt);
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(target_os = "linux")]
pub fn get_tcp_stats(stream: &tokio::net::TcpStream) -> std::io::Result<TcpStats> {
    use std::mem;
    use std::os::unix::io::AsRawFd;

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
        // Convert snd_cwnd from segments to bytes by multiplying by MSS
        let snd_cwnd_bytes = (info.snd_cwnd as u64) * (info.snd_mss as u64);

        let mut flags = 0;
        flags |= TCP_SND_CWND_PRESENT;
        flags |= TCP_RTT_PRESENT;
        flags |= TCP_RTTVAR_PRESENT;
        flags |= TCP_PMTU_PRESENT;

        Ok(TcpStats {
            retransmits: info.total_retrans as u64,
            snd_cwnd: snd_cwnd_bytes,
            rtt: info.rtt as u64,
            rttvar: info.rttvar as u64,
            pmtu: info.pmtu as u64,
            flags,
        })
    } else {
        Ok(TcpStats::default())
    }
}

/// Retrieves TCP statistics from a socket (non-Linux platforms).
///
/// On platforms other than Linux, detailed TCP statistics are not available.
/// This function returns a default `TcpStats` struct with all fields set to
/// zero or `None`.
///
/// # Arguments
///
/// * `_stream` - The TCP stream (unused on non-Linux platforms)
///
/// # Returns
///
/// A default `TcpStats` struct with no statistics.
///
/// # Platform Support
///
/// For detailed TCP statistics, use Linux. On macOS, Windows, and other
/// platforms, this function provides no useful data.
#[cfg(not(target_os = "linux"))]
pub fn get_tcp_stats(_stream: &tokio::net::TcpStream) -> std::io::Result<TcpStats> {
    Ok(TcpStats::default())
}
