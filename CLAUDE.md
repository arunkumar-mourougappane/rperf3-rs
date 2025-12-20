# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

rperf3-rs is a high-performance network throughput measurement tool written in Rust, inspired by iperf3. It provides both a CLI tool and a library API for measuring TCP and UDP network performance with bandwidth limiting, jitter/packet loss measurement, and real-time callbacks.

**Key Differentiators:**
- Memory safety guaranteed by Rust's ownership system
- Async I/O built on Tokio runtime
- Lock-free atomic operations for measurements (15-30% performance improvement at >10 Gbps)
- Ring buffer-based interval storage prevents unbounded memory growth (30-50% memory reduction)
- Batch socket operations (sendmmsg/recvmmsg) on Linux for 30-50% UDP throughput improvement
- Achieves 40+ Gbps TCP throughput on localhost

## Build and Test Commands

### Building
```bash
# Development build
cargo build

# Release build (required for performance testing)
cargo build --release

# Binary location after build
./target/release/rperf3
```

### Testing
```bash
# Run all tests
cargo test

# Run tests in release mode (important for performance-sensitive tests)
cargo test --release

# Run specific test
cargo test test_name

# Run integration tests only
cargo test --test integration_tests

# Run doc tests only
cargo test --doc

# Run with logging enabled
RUST_LOG=debug cargo test
```

### Linting and Formatting
```bash
# Format code (must pass before committing)
cargo fmt

# Check formatting without modifying
cargo fmt --all -- --check

# Run clippy (must pass with no warnings)
cargo clippy

# Clippy with strict warnings (CI standard)
cargo clippy -- -D warnings

# Generate and check documentation
cargo doc --no-deps --all-features
```

### Running the Tool
```bash
# Start server (terminal 1)
cargo run --bin rperf3 -- server
cargo run --bin rperf3 -- server -p 8080 -J  # Custom port with JSON output

# Run client (terminal 2)
cargo run --bin rperf3 -- client 127.0.0.1
cargo run --bin rperf3 -- client 192.168.1.100 -u -b 100M  # UDP test at 100 Mbps

# Run examples
cargo run --example server
cargo run --example client
cargo run --example udp_client
```

## Architecture and Module Organization

### High-Level Architecture
The codebase is organized as both a library (src/lib.rs) and a CLI binary (src/bin/main.rs). The library exposes a clean public API while the CLI provides a command-line interface using clap.

```
┌─────────────────────────────────────┐
│        CLI Binary (bin/main.rs)     │
├─────────────────────────────────────┤
│        Library API (lib.rs)         │
├──────────────┬──────────────────────┤
│   Client     │      Server          │
│   - TCP Send │      - TCP Recv      │
│   - UDP Send │      - UDP Recv      │
│   - Stats    │      - Stats         │
├──────────────┴──────────────────────┤
│  Protocol    │  Measurements        │
│  Messages    │  Atomic Counters     │
│  JSON        │  Ring Buffers        │
├─────────────────────────────────────┤
│  Performance Optimizations          │
│  - Buffer Pool - Token Bucket       │
│  - Batch Socket - Interval Reporter │
└─────────────────────────────────────┘
```

### Core Modules

**client.rs**
- Client-side test execution for both TCP and UDP
- Progress callback system for real-time event notifications
- Stream management for parallel connections
- Integrates with measurements, buffer_pool, and protocol modules

**server.rs**
- Server-side test handling for concurrent clients
- Separate TCP control channel and data channel architecture
- Protocol negotiation and test coordination
- Server continues listening between tests

**protocol.rs**
- Client-server message protocol (Setup, Start, Result, Done, Error)
- JSON serialization for all messages
- Protocol version management (PROTOCOL_VERSION = 1)
- Stream ID generation for parallel streams (default starts at 5, matching iperf3)

**measurements.rs**
- Thread-safe statistics collection using Arc<Measurements>
- AtomicU64 for lock-free byte/packet counting (critical for >10 Gbps)
- CircularIntervalBuffer with VecDeque for bounded memory (MAX_INTERVALS = 86400 in production)
- Separate measurement tracking for TCP (retransmits, cwnd, RTT) and UDP (jitter, packet loss)
- Per-stream atomic counters for better parallel stream scaling

**config.rs**
- Builder pattern for test configuration
- Validation of parameters (bandwidth, buffer sizes, duration)
- Protocol enum (Tcp/Udp) with default behaviors
- Server and client configuration constructors

**udp_packet.rs**
- UDP packet format with sequence numbers and timestamps
- Used for packet loss, out-of-order, and jitter measurement
- RFC 3550 compliant jitter calculation

**buffer_pool.rs**
- Thread-safe buffer pooling using Arc and Mutex
- Pre-allocated buffers reduce allocation overhead by 10-20%
- Separate pools for TCP (128KB) and UDP (8KB default) buffers

**token_bucket.rs**
- Bandwidth limiting using token bucket algorithm
- Integer arithmetic for efficiency (not floating-point)
- Used for both TCP reverse mode and UDP send rate limiting

**batch_socket.rs**
- Linux-only optimization using sendmmsg/recvmmsg
- Batches up to 64 UDP packets per system call (MAX_BATCH_SIZE)
- Automatic fallback to standard send/recv on non-Linux platforms
- 30-50% UDP throughput improvement on Linux

**interval_reporter.rs**
- Async interval reporting moved off critical data path
- Formats iperf3-compatible output with proper alignment
- Spawned as separate Tokio task to avoid blocking send/receive loops
- 5-10% throughput improvement by eliminating formatting overhead

**error.rs**
- Custom error types using thiserror
- Result type alias for consistent error handling

### Important Implementation Details

**TCP vs UDP Control Channel:**
- Even for UDP tests, the client-server control channel uses TCP
- Data channel uses the protocol specified in configuration (TCP or UDP)
- This matches iperf3's architecture

**Reverse Mode:**
- Normal mode: client sends data to server
- Reverse mode (-R): server sends data to client
- Bandwidth limiting applies to the sender (client in normal, server in reverse)

**Parallel Streams:**
- Each stream gets a unique ID: DEFAULT_STREAM_ID + (index * 2)
- Measurements tracked per-stream with separate atomic counters
- Final results aggregate all streams

**Memory Safety Critical Sections:**
- Atomic operations (AtomicU64) used instead of Mutex for hot paths
- Arc<Measurements> shared across threads/tasks
- Buffer pool uses parking_lot::Mutex for better performance than std::sync::Mutex

**Platform-Specific Code:**
- Linux: Uses sendmmsg/recvmmsg for batch operations (batch_socket.rs)
- Linux: Extracts TCP_INFO for retransmits, cwnd, RTT, PMTU
- musl libc: Special handling for MSG_DONTWAIT compatibility
- Non-Linux: Falls back to standard socket operations

## Testing Strategy

### Test Organization
- `tests/integration_tests.rs`: Full client-server integration tests
- `tests/callback_tests.rs`: Progress callback functionality
- Inline doc-tests in each module (73 total doc-tests as of v0.5.0)
- Examples in `examples/` serve as both demos and integration tests

### Critical Test Scenarios
When modifying performance-critical code, test:
1. High-throughput TCP (should achieve 40+ Gbps localhost)
2. UDP with bandwidth limiting (should be within 2-3% of target)
3. Parallel streams (test with -P 4)
4. Reverse mode for both TCP and UDP
5. Long-running tests to verify ring buffer bounds (10+ minutes)

### Test Reliability
- 100% test success rate target (122/122 tests passing as of v0.6.0)
- Tests use #[tokio::test] for async testing
- Integration tests start actual server/client instances
- Tests should clean up resources (ports, tasks) on completion

## Common Development Workflows

### Adding a New Feature
1. Update Config struct if new configuration needed
2. Modify protocol.rs if new messages required
3. Implement in client.rs and/or server.rs
4. Update measurements.rs for new metrics
5. Add tests in integration_tests.rs
6. Update README.md and lib.rs documentation
7. Add example in examples/ if public API changes

### Performance Optimization
1. Profile with `cargo bench` or manual performance tests
2. Focus on hot paths: send/recv loops, measurement recording
3. Consider atomic operations over Mutex for counters
4. Benchmark before/after with release builds
5. Test on Linux (sendmmsg/recvmmsg), macOS, and Windows
6. Document performance improvement in CHANGELOG.md

### Fixing Bugs
1. Add failing test that reproduces the bug
2. Fix the bug
3. Verify test passes
4. Check if fix affects performance (run benchmarks)
5. Update documentation if behavior changes

### Platform-Specific Development
- Use `#[cfg(target_os = "linux")]` for Linux-only code
- Use `#[cfg(not(target_os = "linux"))]` for fallback implementations
- Test on multiple platforms via CI (GitHub Actions runs ubuntu, macos, windows)
- For musl libc compatibility, check libc feature detection

## Code Patterns and Conventions

### Async/Await
- All I/O operations use Tokio async runtime
- Use `tokio::spawn` for concurrent tasks
- Client/Server run methods are async: `async fn run(&self) -> Result<()>`

### Error Handling
- Return `Result<T, Error>` from fallible functions
- Use `?` operator for error propagation
- Create descriptive error messages with context

### Builder Pattern
- Config uses builder pattern: `Config::client(...).with_duration(...)`
- Allows flexible configuration with sane defaults

### Measurement Collection
- Always use atomic operations (AtomicU64) for counters in hot paths
- Take snapshots for interval reporting to avoid lock contention
- Use parking_lot::Mutex when lock is unavoidable (not in hot path)

### Documentation
- Public API items require doc comments with examples
- Use `///` for public items, `//` for internal comments
- Include `# Examples` section in doc comments
- Prefer doc-tests over just example code blocks

## Dependencies and External Libraries

**Core Dependencies:**
- `tokio`: Async runtime (version 1, features = ["full"])
- `serde`, `serde_json`: Serialization for protocol messages and JSON output
- `clap`: CLI argument parsing (version 4, derive feature)
- `anyhow`, `thiserror`: Error handling

**Performance Dependencies:**
- `parking_lot`: Faster Mutex implementation
- `socket2`: Low-level socket configuration (buffer sizes, TCP_NODELAY)
- `byteorder`: Efficient binary serialization for UDP packets

**Platform-Specific:**
- `libc`: Linux-only for sendmmsg/recvmmsg and TCP_INFO

## CI/CD and Quality Standards

### GitHub Actions Workflows
- `.github/workflows/ci.yml`: Build, test, format, clippy on ubuntu/macos/windows
- `.github/workflows/security.yml`: Security audits
- `.github/workflows/release.yml`: Release automation

### Pre-commit Checklist
- [ ] `cargo fmt` passes
- [ ] `cargo clippy -- -D warnings` passes (no warnings allowed)
- [ ] `cargo test` passes (all tests)
- [ ] `cargo test --release` passes (performance-sensitive tests)
- [ ] `cargo doc --no-deps` builds without warnings
- [ ] Manual testing with `cargo run --bin rperf3`

### Release Process
- Version in Cargo.toml follows semver
- Update CHANGELOG.md with changes
- Tag release with `v0.x.y` format
- CI automatically builds binaries for releases

## Known Limitations and Gotchas

1. **musl libc compatibility**: MSG_DONTWAIT constant differs between glibc and musl, special handling in batch_socket.rs
2. **Windows TCP_INFO**: Not available on Windows, TCP stats are Linux-only
3. **Ring buffer size**: MAX_INTERVALS = 86400 allows 24 hours at 1s intervals, tests use 100 for speed
4. **Parallel streams**: Each stream gets odd-numbered IDs (5, 7, 9...) to match iperf3
5. **UDP packet size**: Default 1460 bytes to avoid fragmentation, TCP uses 128KB buffers
6. **Atomic overhead**: AtomicU64 has ~5ns latency vs ~50ns for Mutex, critical at >1M packets/sec

## Performance Targets and Benchmarks

- **TCP localhost**: 40+ Gbps (achieved: 27-28 Gbps typical, 40+ with optimizations)
- **UDP throughput**: 90-95 Mbps sustained (achieved: 94.70 Mbps)
- **Bandwidth limiting accuracy**: Within 2-3% of target
- **Memory overhead**: 30-50% reduction from ring buffers vs unbounded storage
- **Atomic operations**: 15-30% improvement at >10 Gbps vs Mutex-based counters
