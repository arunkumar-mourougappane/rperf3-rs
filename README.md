# rperf3-rs

[![CI](https://github.com/arunkumar-mourougappane/rperf3-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/arunkumar-mourougappane/rperf3-rs/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

**rperf3-rs** is a network throughput measurement tool written in Rust, inspired by iperf3. It aims to provide bandwidth testing for TCP and UDP protocols with a focus on memory safety, async I/O, and ease of use both as a command-line tool and as a library.

## What is rperf3-rs?

rperf3-rs allows you to measure the maximum achievable bandwidth between two network endpoints. Whether you're diagnosing network performance issues, validating infrastructure upgrades, or benchmarking network equipment, rperf3-rs provides detailed, real-time statistics about your network's capabilities.

Built from the ground up in Rust, rperf3-rs leverages modern async I/O (via Tokio) to achieve high throughput while maintaining memory safety guarantees. Unlike traditional C-based tools, rperf3-rs eliminates entire classes of bugs (buffer overflows, use-after-free, data races) through Rust's compile-time checks.

### Key Capabilities

- **Accurate Bandwidth Measurement**: Measure TCP and UDP throughput with sub-second interval reporting
- **Bidirectional Testing**: Test both normal mode (client → server) and reverse mode (server → client)
- **Detailed TCP Statistics**: On Linux, get comprehensive TCP metrics including retransmits, RTT, congestion window, and PMTU
- **UDP Packet Loss & Jitter**: Measure packet loss percentage, jitter, and detect out-of-order packets for UDP flows
- **Dual Interface**: Use as a standalone CLI tool or integrate as a Rust library with full async support
- **Real-time Callbacks**: Monitor test progress programmatically with event-driven callbacks
- **JSON Output**: Machine-readable output compatible with automation and monitoring systems
- **Cross-Platform**: Native support for Linux, macOS, and Windows across multiple architectures

### Why rperf3-rs?

**Performance**: Built on Tokio's async runtime with zero-copy buffer management, rperf3-rs achieves 25-30 Gbps throughput on localhost tests.

**Safety**: Rust's ownership system eliminates memory safety bugs at compile time, making rperf3-rs more reliable than C-based alternatives.

**Developer-Friendly**: Clean API design with builder patterns, comprehensive error handling, and extensive documentation make integration straightforward.

**Modern Architecture**: Async/await syntax, modular design, and thread-safe statistics collection provide a solid foundation for network testing applications.

## Features

- **TCP and UDP Testing**: Measure throughput for both TCP and UDP protocols
- **Bidirectional Testing**: Normal mode (client sends) and reverse mode (server sends)
- **Real-time Statistics**: Periodic interval reporting with bandwidth measurements
- **Progress Callbacks**: Get real-time updates during test execution via callbacks
- **Multiple Streams**: Support for parallel stream testing
- **JSON Output**: Machine-readable output format for automation
- **Library and Binary**: Use as a Rust library or standalone CLI tool
- **Async I/O**: Built on Tokio for high-performance async operations

## Quick Start

### Installation

Build from source:

```bash
git clone https://github.com/arunkumar-mourougappane/rperf3-rs.git
cd rperf3-rs
cargo build --release
```

The binary will be available at `target/release/rperf3`.

### Basic Usage

**Start a server:**

```bash
rperf3 server
```

**Run a client test:**

```bash
rperf3 client <server-address>
```

**Example test:**

```bash
# Terminal 1 - Start server
./target/release/rperf3 server

# Terminal 2 - Run 10-second test
./target/release/rperf3 client 127.0.0.1 --time 10
```

## Usage

### Server Mode

Start a server on the default port (5201):

```bash
rperf3 server
```

Custom port and UDP:

```bash
# TCP server on port 8080
rperf3 server --port 8080

# UDP server
rperf3 server --udp

# Bind to specific address
rperf3 server --bind 192.168.1.100
```

### Client Mode

Basic TCP test:

```bash
rperf3 client <server-address>
```

Common options:

```bash
# 30-second test
rperf3 client 192.168.1.100 --time 30

# UDP test with 100 Mbps target bandwidth
rperf3 client 192.168.1.100 --udp --bandwidth 100

# Reverse mode (server sends data)
rperf3 client 192.168.1.100 --reverse

# Custom buffer size and parallel streams
rperf3 client 192.168.1.100 --length 262144 --parallel 4

# JSON output for automation
rperf3 client 192.168.1.100 --json

# Custom interval reporting (every 2 seconds)
rperf3 client 192.168.1.100 --interval 2
```

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
rperf3 = { git = "https://github.com/arunkumar-mourougappane/rperf3-rs" }
tokio = { version = "1", features = ["full"] }
```

### Client Example

```rust
use rperf3::{Client, Config, Protocol};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the test
    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_protocol(Protocol::Tcp)
        .with_duration(Duration::from_secs(10))
        .with_buffer_size(128 * 1024);

    // Run the test
    let client = Client::new(config)?;
    client.run().await?;

    // Get results
    let measurements = client.get_measurements();
    println!("Bandwidth: {:.2} Mbps",
             measurements.total_bits_per_second() / 1_000_000.0);

    Ok(())
}
```

### Client with Progress Callback

Monitor test progress in real-time using callbacks:

```rust
use rperf3::{Client, Config, ProgressEvent, Protocol};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_protocol(Protocol::Tcp)
        .with_duration(Duration::from_secs(10));

    // Create client with callback
    let client = Client::new(config)?
        .with_callback(|event: ProgressEvent| {
            match event {
                ProgressEvent::TestStarted => {
                    println!("Test started!");
                }
                ProgressEvent::IntervalUpdate { interval_end, bytes, bits_per_second, .. } => {
                    println!("{:.1}s: {} bytes @ {:.2} Mbps",
                        interval_end.as_secs_f64(),
                        bytes,
                        bits_per_second / 1_000_000.0);
                }
                ProgressEvent::TestCompleted { total_bytes, duration, bits_per_second } => {
                    println!("Completed: {} bytes in {:.2}s @ {:.2} Mbps",
                        total_bytes,
                        duration.as_secs_f64(),
                        bits_per_second / 1_000_000.0);
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
use rperf3::{Server, Config, Protocol};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::server(5201)
        .with_protocol(Protocol::Tcp);

    let server = Server::new(config);
    server.run().await?;

    Ok(())
}
```

## Detailed JSON Output

When using the `--json` flag, rperf3 outputs comprehensive test results in a structured JSON format similar to iperf3. This includes:

**TCP Mode:**

- **Connection Information**: Socket FD, local/remote addresses and ports
- **System Information**: OS version, hostname, timestamp
- **Test Configuration**: Protocol, stream count, buffer size, duration, direction
- **Interval Statistics**: Per-second measurements with bytes and throughput
- **TCP Statistics**: Retransmits, congestion window, RTT, RTT variance, PMTU (Linux only)
- **Summary Results**: Sender/receiver totals with min/max/mean statistics
- **CPU Utilization**: Host and remote CPU usage percentages (when available)
- **Congestion Algorithm**: TCP congestion control algorithm in use

**UDP Mode:**

- **Connection Information**: Socket FD, local/remote addresses and ports
- **System Information**: OS version, hostname, timestamp
- **Test Configuration**: Protocol, stream count, buffer size, duration, direction
- **Interval Statistics**: Per-second measurements with bytes, throughput, and packet counts
- **UDP Statistics**: Jitter (ms), lost packets, packet count, loss percentage, out-of-order packets
- **Summary Results**: Aggregated statistics with jitter and packet loss metrics

### Example JSON Output

**TCP Mode:**

```bash
rperf3 client 127.0.0.1 -t 5 --json
```

Sample output structure:

```json
{
  "start": {
    "connected": [
      {
        "socket_fd": 3,
        "local_host": "127.0.0.1",
        "local_port": 45678,
        "remote_host": "127.0.0.1",
        "remote_port": 5201
      }
    ],
    "version": "rperf3 0.1.0",
    "system_info": "linux x86_64 hostname",
    "timestamp": {
      "time": "Wed, 3 Dec 2025 03:16:18 +0000",
      "timesecs": 1764731778
    },
    "test_start": {
      "protocol": "Tcp",
      "num_streams": 1,
      "blksize": 131072,
      "duration": 5,
      "reverse": false
    }
  },
  "intervals": [
    {
      "streams": [
        {
          "socket": 3,
          "start": 0.0,
          "end": 1.0,
          "seconds": 1.0,
          "bytes": 1000000000,
          "bits_per_second": 8000000000.0,
          "retransmits": 0,
          "snd_cwnd": 43680,
          "rtt": 123,
          "omitted": false
        }
      ],
      "sum": {
        /* aggregate stats */
      }
    }
  ],
  "end": {
    "sum_sent": {
      "bytes": 5000000000,
      "bits_per_second": 8000000000.0,
      "retransmits": 5,
      "max_snd_cwnd": 87360
    },
    "cpu_utilization_percent": 2.5,
    "sender_tcp_congestion": "cubic"
  }
}
```

**UDP Mode:**

```bash
rperf3 client 127.0.0.1 -t 10 --udp --json
```

Sample UDP output:

```json
{
  "start": {
    "test_start": {
      "protocol": "UDP",
      "num_streams": 1,
      "blksize": 1448,
      "duration": 10
    }
  },
  "intervals": [
    {
      "streams": [
        {
          "socket": 5,
          "start": 0.0,
          "end": 1.0,
          "seconds": 1.0,
          "bytes": 131768,
          "bits_per_second": 1054144.0,
          "packets": 91,
          "omitted": false,
          "sender": true
        }
      ],
      "sum": {
        /* aggregate with packets */
      }
    }
  ],
  "end": {
    "streams": [
      {
        "udp": {
          "bytes": 1311888,
          "bits_per_second": 1049500.0,
          "jitter_ms": 1.54,
          "lost_packets": 0,
          "packets": 906,
          "lost_percent": 0.0,
          "sender": true
        }
      }
    ],
    "sum": {
      "jitter_ms": 1.54,
      "lost_packets": 0,
      "packets": 906,
      "lost_percent": 0.0
    }
  }
}
```

### Platform-Specific Features

**Linux**: Full TCP statistics including retransmits, RTT, congestion window, and PMTU via `TCP_INFO` socket option.

**Other Platforms**: Basic statistics without TCP-specific metrics.

## Command-Line Reference

### Server Options

| Option                 | Description              | Default        |
| ---------------------- | ------------------------ | -------------- |
| `-p, --port <PORT>`    | Port to listen on        | 5201           |
| `-b, --bind <ADDRESS>` | Bind to specific address | All interfaces |
| `-u, --udp`            | Use UDP instead of TCP   | TCP            |

### Client Options

| Option                     | Description                  | Default        |
| -------------------------- | ---------------------------- | -------------- |
| `<SERVER>`                 | Server address to connect to | Required       |
| `-p, --port <PORT>`        | Port to connect to           | 5201           |
| `-u, --udp`                | Use UDP instead of TCP       | TCP            |
| `-t, --time <SECONDS>`     | Test duration                | 10             |
| `-b, --bandwidth <MBPS>`   | Target bandwidth (UDP only)  | Unlimited      |
| `-l, --length <BYTES>`     | Buffer size                  | 131072         |
| `-P, --parallel <NUM>`     | Number of parallel streams   | 1              |
| `-R, --reverse`            | Reverse mode (server sends)  | Normal mode    |
| `-J, --json`               | JSON output format           | Human-readable |
| `-i, --interval <SECONDS>` | Report interval              | 1              |

## Architecture

The project uses a modular design with clear separation of concerns:

```
┌─────────────────────────────────────────┐
│           rperf3-rs                     │
├─────────────────────────────────────────┤
│  CLI Binary        │  Library API       │
│  (clap + main)     │  (public modules)  │
├─────────────────────────────────────────┤
│  Client   │  Server   │  Protocol       │
│  Module   │  Module   │  Module         │
├─────────────────────────────────────────┤
│  Config   │  Measurements │  Error      │
│  Module   │  Module       │  Module     │
├─────────────────────────────────────────┤
│         Tokio Runtime (Async I/O)       │
└─────────────────────────────────────────┘
```

### Module Responsibilities

- **protocol**: Message format and serialization for client-server communication
- **client**: Client implementation for initiating tests and collecting results
- **server**: Server implementation for handling connections and running tests
- **config**: Configuration structures with builder pattern
- **measurements**: Thread-safe statistics collection and calculation
- **error**: Custom error types and conversions

## Performance

Built with performance in mind:

- **Async I/O**: Non-blocking operations using Tokio
- **Zero-copy**: Efficient buffer management
- **Thread-safe**: Lock-free where possible, using Arc/Mutex when needed
- **Optimized builds**: Release builds with full optimizations

Typical throughput on localhost: **25-30 Gbps** for TCP tests.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development

```bash
# Clone and build
git clone https://github.com/arunkumar-mourougappane/rperf3-rs.git
cd rperf3-rs
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy

# Run examples
cargo run --example server
cargo run --example client
```

## Comparison with iperf3

| Feature          | iperf3  | rperf3-rs     |
| ---------------- | ------- | ------------- |
| TCP Testing      | ✅      | ✅            |
| UDP Testing      | ✅      | ✅            |
| Reverse Mode     | ✅      | ✅            |
| JSON Output      | ✅      | ✅            |
| Parallel Streams | ✅      | ✅            |
| Library API      | Limited | Full-featured |
| Language         | C       | Rust          |
| Memory Safety    | Manual  | Guaranteed    |
| Async I/O        | No      | Yes (Tokio)   |

## Roadmap

### Completed Features

- [x] UDP packet loss and jitter measurement (v0.4.0)
- [x] Sequence-based packet tracking
- [x] RFC 3550 jitter calculation
- [x] Out-of-order packet detection

### Planned Features

- [ ] Enhanced parallel stream support
- [ ] IPv6 improvements
- [ ] SCTP protocol support
- [ ] TCP retransmission statistics
- [ ] CPU utilization monitoring
- [ ] Additional output formats (CSV, XML)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

Inspired by [iperf3](https://github.com/esnet/iperf) - the industry-standard network performance testing tool.

---

**Author**: Arunkumar Mourougappane  
**Repository**: https://github.com/arunkumar-mourougappane/rperf3-rs
