# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned

- Enhanced parallel stream support
- IPv6 improvements
- SCTP protocol support
- CPU utilization monitoring
- Additional output formats (CSV, XML)
- Bidirectional simultaneous testing
- Historical data storage
- Web UI for results visualization

## [0.3.0] - 2025-12-02

### Added

- **Documentation Improvements**:
  - Comprehensive CHANGELOG.md with proper version history
  - All versions now documented in reverse chronological order
  - Detailed release notes for each version with categorized changes

### Changed

- **Version**: Bumped from 0.2.0 to 0.3.0
- **CHANGELOG**: Restructured to follow Keep a Changelog format strictly
  - Moved v0.2.0 features from Unreleased to proper release section
  - Added all commits and changes since initial release
  - Organized changes into Added, Changed, Fixed, Removed categories

### Documentation

- Updated CHANGELOG with complete version history
- Proper semantic versioning documentation
- Clear separation between releases and unreleased features

## [0.2.0] - 2025-12-02

### Added

- **Progress Callbacks**: Real-time test progress monitoring via callback interface
  - `ProgressEvent` enum with TestStarted, IntervalUpdate, TestCompleted, and Error events
  - `ProgressCallback` trait supporting both closures and custom structs
  - `Client::with_callback()` builder method for easy callback integration
  - Callback notifications throughout TCP and UDP test execution
  - Examples: `client_with_callback.rs` and `client_closure_callback.rs`

- **Detailed JSON Output**: Comprehensive test results matching iperf3 format
  - **TCP Mode**: Connection info, TCP stats (retransmits, cwnd, RTT, PMTU), congestion algorithm
  - **UDP Mode**: Packet counts, jitter measurement, packet loss stats, out-of-order detection
  - System information (OS version, hostname, timestamps)
  - Per-interval statistics with bytes, throughput, and packet counts (UDP)
  - Platform-specific TCP statistics via `TCP_INFO` socket option (Linux)
  - Helper functions: `get_connection_info()`, `get_system_info()`, `get_tcp_stats()`

- **Enhanced Measurements**: New comprehensive data structures
  - `DetailedTestResults` with start/intervals/end sections
  - `ConnectionInfo` for socket-level details
  - `TestConfig` for test parameters
  - `SystemInfo` for OS and timestamp data
  - `TcpStats` for TCP-specific metrics (with `Default` derive)
  - `UdpStats` for UDP-specific metrics (jitter, packet loss, out-of-order)
  - `DetailedIntervalStats` / `UdpIntervalStats` for per-interval data
  - `StreamSummary` / `UdpStreamSummary` for aggregated results
  - `MeasurementsCollector::get_detailed_results()` method
  - UDP tracking methods: `record_udp_packet()`, `record_udp_loss()`, `update_jitter()`

- **UDP Statistics**: Complete UDP performance metrics
  - Packet counting (sent and received)
  - Jitter measurement using exponential moving average (EMA: 0.875 * old + 0.125 * new)
  - Packet loss calculation and percentage
  - Out-of-order packet detection
  - Per-interval packet statistics in JSON output

- **Examples**: 8 comprehensive example programs
  - `server.rs` - Basic server example
  - `client.rs` - Basic client example
  - `udp_client.rs` - UDP-specific testing
  - `client_with_callback.rs` - Custom callback struct example
  - `client_closure_callback.rs` - Closure-based callback example
  - `show_json_structure.rs` - TCP JSON output demonstration
  - `show_udp_json.rs` - UDP JSON output demonstration
  - `test_json_output.rs` - JSON serialization testing

- **GitHub Actions CI/CD**:
  - Automated testing on Linux, macOS, and Windows
  - Multi-version Rust testing (stable and beta)
  - Code formatting checks with `rustfmt`
  - Linting with `clippy` enforcing zero warnings
  - Code coverage reporting with `cargo-tarpaulin` and Codecov
  - Automated release workflow for tagged versions
  - Multi-architecture release builds (x86_64, aarch64 for Linux/macOS, x86_64 for Windows)
  - Security audit workflow with scheduled weekly runs
  - Dependency vulnerability checking with `cargo-audit`
  - Build artifact caching for faster CI runs

### Removed

- **iperf3 Protocol Compatibility**: Removed incompatible iperf3 client implementation
  - Deleted `iperf3_client.rs` (~400 lines)
  - Deleted `iperf3_protocol.rs` (~215 lines)
  - Removed `--iperf3` CLI flag
  - Cleaned up all iperf3 references from documentation

### Changed

- **Version**: Bumped from 0.1.0 to 0.2.0
- **Author**: Added author information to Cargo.toml
- **JSON Output**: Now uses detailed results structure with protocol-specific formatting
- **UDP Mode**: Tracks packets, jitter (exponential moving average), and loss statistics
- **Interval Statistics**: Include optional packet counts for UDP
- **Console Output**: UDP output displays packet counts
- **README**: Updated with UDP statistics documentation, CI badge, and usage examples
- **Documentation**: Improved organization and added CONTRIBUTING.md with project structure
- **TestEndInfo Enum**: Boxed large `StreamSummary` fields to reduce memory footprint

### Fixed

- Clippy warning: Derived `Default` trait for `TcpStats` instead of manual implementation
- Clippy warning: Removed empty line after doc comment in measurements module
- Clippy warning: Boxed large fields in `TestEndInfo` enum (reduced from 384 bytes)
- All code now passes `cargo clippy --all-targets --all-features -- -D warnings`

### Dependencies

- Added `rand` 0.8 for cookie generation
- Added `hostname` 0.3 for system information
- Added `libc` 0.2 (Linux only) for TCP statistics via socket options

## [0.1.0] - 2025-12-02

### Added

- Initial release of rperf3-rs
- TCP throughput testing (client and server modes)
- UDP throughput testing with bandwidth limiting
- Command-line interface using clap
- Library API for programmatic use
- Real-time interval reporting with bandwidth measurements
- Reverse mode testing (server sends, client receives)
- JSON output format for automation
- Configurable test duration and buffer sizes
- Async I/O with Tokio runtime
- Custom protocol for efficient client-server communication
- Thread-safe statistics collection
- Comprehensive error handling
- Example programs for common use cases
- Full documentation and guides

### Features Implemented

- TCP and UDP protocol support
- Bidirectional testing modes
- Periodic bandwidth reports at configurable intervals
- Parallel stream support
- Measurements collection and reporting
- Builder pattern configuration
- Clean separation between library and binary

### Technical Highlights

- Zero compiler warnings
- All tests passing
- ~1,500+ lines of well-documented code
- 3.6 MB optimized release binary
- 25-30 Gbps throughput on localhost
