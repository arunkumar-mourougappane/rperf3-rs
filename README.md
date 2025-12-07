# rperf3-rs

[![CI](https://github.com/arunkumar-mourougappane/rperf3-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/arunkumar-mourougappane/rperf3-rs/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

A high-performance network throughput measurement tool written in Rust, inspired by iperf3. Provides accurate bandwidth testing for TCP and UDP protocols with memory safety, async I/O, and comprehensive metrics.

## What is rperf3-rs?

rperf3-rs is a modern network performance measurement tool that allows you to measure the maximum achievable bandwidth between two network endpoints. Whether you're diagnosing network performance issues, validating infrastructure upgrades, or benchmarking network equipment, rperf3-rs provides detailed, real-time statistics about your network's capabilities.

Built from the ground up in Rust, rperf3-rs leverages modern async I/O (via Tokio) to achieve high throughput while maintaining memory safety guarantees. Unlike traditional C-based tools, rperf3-rs eliminates entire classes of bugs (buffer overflows, use-after-free, data races) through Rust's compile-time checks.

### Why rperf3-rs?

**Memory Safety**: Rust's ownership system eliminates memory safety bugs at compile time, making rperf3-rs more reliable than C-based alternatives. No buffer overflows, no use-after-free, no data races.

**High Performance**: Built on Tokio's async runtime with optimized buffer management, rperf3-rs achieves 25-30 Gbps throughput on localhost tests, matching or exceeding traditional tools.

**Developer-Friendly**: Clean API design with builder patterns, comprehensive error handling, and extensive documentation make integration straightforward. Use it as a CLI tool or embed it as a library.

**Modern Architecture**: Async/await syntax, modular design, and thread-safe statistics collection provide a solid foundation for building network testing applications.

**Full-Featured**: Supports TCP and UDP testing, bidirectional tests, bandwidth limiting, packet loss and jitter measurement, real-time callbacks, and JSON output for automation.

## Features

- **TCP & UDP Testing**: Measure throughput for both reliable and unreliable protocols
- **Bidirectional Testing**: Normal mode (client → server) and reverse mode (server → client)
- **Bandwidth Limiting**: Control send rate with K/M/G notation (e.g., 100M = 100 Mbps)
- **UDP Metrics**: Packet loss percentage, jitter (RFC 3550), and out-of-order detection
- **TCP Statistics**: Retransmits, RTT, congestion window, and PMTU (Linux)
- **Batch Socket Operations**: 30-50% UDP performance boost on Linux via sendmmsg/recvmmsg
- **Real-time Callbacks**: Monitor test progress programmatically with event-driven callbacks
- **JSON Output**: Machine-readable output compatible with automation systems
- **Parallel Streams**: Multiple concurrent connections for aggregate testing
- **Buffer Pooling**: Optimized memory allocation for 10-20% performance improvement
- **Library & CLI**: Use as a standalone tool or integrate as a Rust library
- **Cross-Platform**: Linux, macOS, and Windows support

## Quick Start

### Installation

**From crates.io** (when published):

```bash
cargo install rperf3
```

**From source**:

```bash
git clone https://github.com/arunkumar-mourougappane/rperf3-rs.git
cd rperf3-rs
cargo build --release
```

Binary available at `target/release/rperf3`.

### Basic Usage

```bash
# Terminal 1 - Start server
./target/release/rperf3 server

# Terminal 2 - Run client test
./target/release/rperf3 client 127.0.0.1
```

## Usage Examples

### TCP Tests

```bash
# Basic TCP test (10 seconds)
rperf3 client 192.168.1.100

# 30-second test with custom interval
rperf3 client 192.168.1.100 -t 30 -i 2

# Reverse mode (server sends data)
rperf3 client 192.168.1.100 -R

# Reverse mode with bandwidth limiting
rperf3 client 192.168.1.100 -R -b 200M

# Parallel streams
rperf3 client 192.168.1.100 -P 4
```

### UDP Tests

```bash
# UDP test with 100 Mbps target
rperf3 client 192.168.1.100 -u -b 100M

# UDP reverse mode with bandwidth limit
rperf3 client 192.168.1.100 -u -R -b 50M

# UDP with custom buffer size
rperf3 client 192.168.1.100 -u -b 1G -l 8192
```

### Server Options

```bash
# Default server (port 5201)
rperf3 server

# Custom port
rperf3 server -p 8080

# Bind to specific address
rperf3 server -b 192.168.1.100

# JSON output
rperf3 server -J
```

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
# From crates.io (when published)
rperf3 = "0.5"

# Or from git
# rperf3 = { git = "https://github.com/arunkumar-mourougappane/rperf3-rs" }

tokio = { version = "1", features = ["full"] }
```

### Basic Client Example

```rust
use rperf3::{Client, Config, Protocol};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::client("192.168.1.100".to_string(), 5201)
        .with_protocol(Protocol::Tcp)
        .with_duration(Duration::from_secs(10));

    let client = Client::new(config)?;
    client.run().await?;

    let measurements = client.get_measurements();
    println!("Bandwidth: {:.2} Mbps",
             measurements.total_bits_per_second() / 1_000_000.0);

    Ok(())
}
```

### UDP Test with Metrics

```rust
use rperf3::{Client, Config, Protocol};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::client("192.168.1.100".to_string(), 5201)
        .with_protocol(Protocol::Udp)
        .with_bandwidth(100_000_000) // 100 Mbps
        .with_duration(Duration::from_secs(10));

    let client = Client::new(config)?;
    client.run().await?;

    let measurements = client.get_measurements();
    println!("Bandwidth: {:.2} Mbps", 
             measurements.total_bits_per_second() / 1_000_000.0);
    println!("Packets: {}, Loss: {} ({:.2}%), Jitter: {:.3} ms",
             measurements.total_packets,
             measurements.lost_packets,
             (measurements.lost_packets as f64 / measurements.total_packets as f64) * 100.0,
             measurements.jitter_ms);

    Ok(())
}
```

### Progress Callbacks

```rust
use rperf3::{Client, Config, ProgressEvent};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::client("192.168.1.100".to_string(), 5201)
        .with_duration(Duration::from_secs(10));

    let client = Client::new(config)?
        .with_callback(|event: ProgressEvent| {
            match event {
                ProgressEvent::TestStarted => {
                    println!("Test started");
                }
                ProgressEvent::IntervalUpdate { bits_per_second, .. } => {
                    println!("Current: {:.2} Mbps", bits_per_second / 1_000_000.0);
                }
                ProgressEvent::TestCompleted { bits_per_second, .. } => {
                    println!("Average: {:.2} Mbps", bits_per_second / 1_000_000.0);
                }
                ProgressEvent::Error(msg) => {
                    eprintln!("Error: {}", msg);
                }
            }
        });

    client.run().await?;
    Ok(())
}
```

### Server Example

```rust
use rperf3::{Server, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::server(5201);
    let server = Server::new(config);
    
    println!("Server listening on port 5201");
    server.run().await?;
    
    Ok(())
}
```

## Command-Line Options

### Server

| Option              | Short | Description              | Default |
|---------------------|-------|--------------------------|---------|
| `--port <PORT>`     | `-p`  | Port to listen on        | 5201    |
| `--bind <ADDRESS>`  | `-b`  | Bind to specific address | 0.0.0.0 |
| `--udp`             | `-u`  | UDP mode                 | TCP     |
| `--json`            | `-J`  | JSON output              | false   |

### Client

| Option                 | Short | Description                     | Default                   |
| ---------------------- | ----- | ------------------------------- | ------------------------- |
| `<SERVER>`             |       | Server address (required)       | -                         |
| `--port <PORT>`        | `-p`  | Server port                     | 5201                      |
| `--udp`                | `-u`  | UDP mode                        | TCP                       |
| `--time <SECONDS>`     | `-t`  | Test duration                   | 10                        |
| `--bandwidth <RATE>`   | `-b`  | Target bandwidth (K/M/G suffix) | unlimited (TCP), 1M (UDP) |
| `--length <BYTES>`     | `-l`  | Buffer/packet size              | 131072 (TCP), 1460 (UDP)  |
| `--parallel <NUM>`     | `-P`  | Number of parallel streams      | 1                         |
| `--reverse`            | `-R`  | Reverse mode (server sends)     | false                     |
| `--json`               | `-J`  | JSON output                     | false                     |
| `--interval <SECONDS>` | `-i`  | Report interval                 | 1                         |

### Bandwidth Notation

Use K/M/G suffixes for bandwidth values:

- `100K` = 100,000 bits/second
- `100M` = 100,000,000 bits/second  
- `1G` = 1,000,000,000 bits/second

## Performance

Typical performance on modern hardware:

- **TCP localhost**: 27-28 Gbps (tested: 27.98 Gbps)
- **UDP throughput**: 90-95 Mbps (tested: 94.70 Mbps)
- **UDP with limiting**: Accurate rate control within 2-3% of target
- **Packet loss detection**: Sub-millisecond precision
- **Jitter measurement**: RFC 3550 compliant algorithm

### Performance Optimizations

rperf3-rs includes several performance optimizations:

- **Batch Socket Operations** (v0.5.0): sendmmsg/recvmmsg on Linux
  - Reduces system call overhead by batching up to 64 UDP packets
  - 30-50% UDP throughput improvement at high packet rates
  - Adaptive batch sizing for optimal bandwidth control
  - Automatic fallback to standard operations on non-Linux platforms
- **Atomic Counters** (v0.5.0): Lock-free byte and packet counting using AtomicU64
  - Eliminates mutex contention in measurement hot path
  - 15-30% performance improvement at >10 Gbps throughput
  - Reduces per-operation latency from ~50ns to ~5ns
- **UDP Timestamp Caching** (v0.5.0): Thread-local timestamp cache
  with 1ms update interval
  - Avoids expensive SystemTime::now() calls in UDP send loops
  - 20-30% UDP throughput improvement
  - Reduces system calls by ~99% (1 call per 1000 packets at 1Mbps)
- **Buffer Pooling** (v0.4.0): Pre-allocated buffer reuse reduces allocation
  overhead by 10-20% for UDP and 5-10% for TCP
- **Async I/O**: Built on Tokio for efficient non-blocking operations
- **Zero-copy where possible**: Minimizes data movement during I/O operations

Built on Tokio's async runtime with optimized buffer management for maximum
throughput.

## JSON Output Format

Use `--json` flag for machine-readable output compatible with automation:

```bash
rperf3 client 192.168.1.100 -u -b 100M --json
```

Output includes:

- **Start**: Connection info, system info, test configuration
- **Intervals**: Per-second measurements with bytes, throughput, packets
- **End**: Summary statistics with jitter, packet loss, retransmits (TCP)

## Architecture

```text
┌─────────────────────────────────────┐
│        rperf3-rs Application        │
├─────────────────────────────────────┤
│  CLI (main.rs)  │  Library API      │
├─────────────────────────────────────┤
│  Client Module  │  Server Module    │
│  - TCP/UDP Send │  - TCP/UDP Recv   │
│  - Statistics   │  - Statistics     │
├─────────────────────────────────────┤
│  Buffer Pool    │  Measurements     │
│  - Reusable     │  - Metrics        │
│  - Thread-safe  │  - Calculations   │
├─────────────────────────────────────┤
│  Protocol       │  UDP Packet       │
│  - Messages     │  - Sequence #s    │
│  - Serialization│  - Timestamps     │
├─────────────────────────────────────┤
│        Tokio Async Runtime          │
└─────────────────────────────────────┘
```

### Key Modules

- **`buffer_pool`**: Thread-safe buffer pooling for efficient memory reuse
- **`client`**: Client-side test execution with progress callbacks
- **`server`**: Server-side test handling for concurrent clients
- **`measurements`**: Thread-safe statistics collection and calculations
- **`protocol`**: Message serialization for client-server communication
- **`udp_packet`**: UDP packet format with sequence numbers and timestamps
- **`config`**: Configuration builder with validation

## Recent Updates

### v0.5.0 (Current)

- ✅ **Atomic counters**: Lock-free measurement recording with 15-30% performance gain at >10 Gbps
- ✅ **UDP timestamp caching**: Thread-local cache reduces system calls by ~99%, 20-30% UDP improvement
- ✅ **Enhanced documentation**: 73 doc-tests including comprehensive UDP packet examples
- ✅ Achieved 27.98 Gbps TCP and 94.70 Mbps UDP throughput in testing
- ✅ Per-operation measurement latency reduced from ~50ns to ~5ns

### v0.4.0

- ✅ **Buffer pooling**: 10-20% performance improvement through memory reuse
- ✅ UDP reverse mode implementation
- ✅ Bandwidth limiting for TCP and UDP
- ✅ Bidirectional bandwidth calculations
- ✅ UDP packet loss and jitter measurement (RFC 3550)
- ✅ Out-of-order packet detection
- ✅ All clippy warnings resolved

## Roadmap

### Planned Features

- [ ] Enhanced parallel stream support with aggregation
- [ ] IPv6 testing and dual-stack support
- [ ] CPU utilization monitoring
- [ ] Additional output formats (CSV)
- [ ] Configurable congestion control algorithms
- [ ] SCTP protocol support
- [ ] Further performance optimizations (batch operations, SIMD)

See [PERFORMANCE_IMPROVEMENTS.md](PERFORMANCE_IMPROVEMENTS.md) for detailed performance roadmap.

## Comparison with iperf3

| Feature              | iperf3  | rperf3-rs |
|----------------------|---------|-----------|
| TCP Testing          | ✅      | ✅        |
| UDP Testing          | ✅      | ✅        |
| Bandwidth Limiting   | ✅      | ✅        |
| Reverse Mode         | ✅      | ✅        |
| JSON Output          | ✅      | ✅        |
| Parallel Streams     | ✅      | ✅        |
| UDP Loss/Jitter      | ✅      | ✅        |
| Library API          | Limited | Full      |
| Language             | C       | Rust      |
| Memory Safety        | Manual  | Guaranteed|
| Async I/O            | No      | Yes       |
| Progress Callbacks   | No      | Yes       |

## Contributing

Contributions welcome! Please ensure:

1. Code passes `cargo fmt` and `cargo clippy`
2. All tests pass: `cargo test`
3. Add tests for new features
4. Update documentation

```bash
# Development workflow
git clone https://github.com/arunkumar-mourougappane/rperf3-rs.git
cd rperf3-rs
cargo build
cargo test
cargo clippy
cargo fmt
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

Inspired by [iperf3](https://github.com/esnet/iperf) - the industry-standard network testing tool.

---

**Author**: Arunkumar Mourougappane  
**Repository**: <https://github.com/arunkumar-mourougappane/rperf3-rs>
