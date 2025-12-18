# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Token bucket algorithm for bandwidth limiting (issue #6)
  - Replaces rate-based limiting with more efficient token bucket approach
  - Uses integer arithmetic instead of floating-point calculations
  - Pre-calculated sleep durations based on token consumption
  - Fewer timing checks reduce per-packet overhead
  - New `token_bucket` module with `TokenBucket` type
  - Example demonstrating token bucket efficiency (`token_bucket_demo.rs`)
- Batch socket operations for UDP on Linux (issue #5)
  - sendmmsg/recvmmsg support for batching up to 64 packets per system call
  - 30-50% UDP throughput improvement at high packet rates
  - Adaptive batch sizing based on bandwidth target
  - Automatic fallback to standard operations on non-Linux platforms
- New `batch_socket` module with `UdpSendBatch` and `UdpRecvBatch` types
- Example demonstrating batch socket operations (`batch_send_test.rs`)

### Performance

- Token bucket provides 5-10% improvement when bandwidth limiting is active
- Integer arithmetic is faster than float calculations for rate control
- Reduced overhead from timing checks on every iteration
- Simpler algorithm with better cache locality
- Batch operations reduce system call overhead for UDP by ~98% (64 packets vs 1)
- Client-side batched sending with adaptive batch sizing (16/32/64 packets)
- Server-side batched receiving for improved UDP reception performance
- Maintains measurement accuracy and bandwidth limiting with batched operations

## [0.5.0] - 2025-12-06

### Added

- Atomic counters for high-frequency measurement operations (issue #3)
  - Lock-free byte and packet counting using AtomicU64
  - 15-30% performance improvement at >10 Gbps throughput
  - Reduces per-operation latency from ~50ns to ~5ns
- UDP packet timestamp caching optimization (issue #4)
  - Thread-local timestamp cache with 1ms update interval
  - Avoids expensive SystemTime::now() calls in hot path
  - 20-30% UDP throughput improvement
  - New `create_packet_fast()` function for optimized packet creation
- Comprehensive doc-tests for UDP packet module
  - 6 new executable documentation examples (3 → 9 total)
  - Examples for all UdpPacketHeader methods
  - Performance comparison examples
  - Error handling demonstrations

### Performance

- Atomic counter optimization: 27.98 Gbps TCP throughput achieved
- Timestamp caching: 94.70 Mbps UDP throughput achieved
- Reduced SystemTime::now() calls by ~99% in UDP send loops
- Lock-free measurement recording eliminates mutex contention

### Documentation

- Added doc-tests for `UdpPacketHeader::new`, `with_current_time`, `to_bytes`, `from_bytes`
- Enhanced `create_packet_fast` documentation with usage examples
- Added `parse_packet` error handling examples
- All 73 doc-tests passing

## [0.4.0] - 2025-12-04

### Added
- UDP reverse mode implementation with TCP control channel handshake
- UDP normal mode server reception (server now properly receives UDP packets)
- TCP bandwidth limiting with rate-based algorithm (checks every 1ms, sleeps if >0.1ms ahead)
- Bidirectional bandwidth calculations (sum of sent and received bytes)
- Bandwidth notation documentation (K/M/G suffixes: 100M = 100,000,000 bps)
- Comprehensive README enhancements:
  - "What is rperf3-rs?" section
  - "Why rperf3-rs?" section
  - crates.io installation instructions
- Enhanced public API documentation in lib.rs with UDP examples and performance benchmarks

### Fixed
- UDP reverse mode (-R flag) now correctly implemented with bidirectional data transfer
- UDP normal mode server bug where server was sleeping instead of receiving packets
- Packet counting bug - corrected parameter order in `record_udp_packet_received()` to (sequence, send_timestamp_us, recv_timestamp_us)
- Bandwidth calculations now account for both sent AND received bytes for accurate bidirectional measurements
- All clippy warnings resolved:
  - Used `abs_diff()` for unsigned integer differences
  - Reduced function parameters by extracting from config
  - Changed assertions from `len() > 0` to `!is_empty()`

### Changed
- `handle_udp_test()` reduced from 8 to 4 parameters (extracted duration, bandwidth, buffer_size, reverse from config)
- UDP reverse mode architecture: TCP control channel (port 5201) + UDP data channel
- Initialization packet uses sequence u64::MAX to discover client UDP port
- `bits_per_second()` and `total_bits_per_second()` now use (bytes_sent + bytes_received) * 8 / duration
- Transit time calculation uses `abs_diff()` instead of subtraction

### Planned

- Enhanced parallel stream support
- IPv6 improvements
- SCTP protocol support
- CPU utilization monitoring
- Additional output formats (CSV, XML)
- Bidirectional simultaneous testing
- Historical data storage
- Web UI for results visualization

## [0.3.9] - 2025-12-03

### Added

- **Comprehensive API Documentation**: Added extensive Rust documentation for all public-facing APIs
  - Enhanced module-level documentation in `lib.rs` with detailed examples and architecture overview
  - Documented all types in `config.rs`: `Protocol`, `Mode`, and `Config` with 11 builder methods
  - Documented `Client` struct with all public methods (`new()`, `with_callback()`, `run()`, `get_measurements()`)
  - Documented `ProgressEvent` enum (4 variants) and `ProgressCallback` trait with usage examples
  - Documented `Server` struct with constructor, `run()`, and `get_measurements()` methods
  - Documented `Error` enum (6 variants) and `Result` type alias with error handling examples
  - Documented `Measurements` struct and helper functions (`get_system_info()`, `get_connection_info()`, `get_tcp_stats()`)
  - Added platform-specific documentation notes for Linux vs non-Linux features
  - Included 30+ code examples demonstrating API usage patterns
  - All documentation compiles without warnings

- **Documentation Testing in CI**: Added dedicated doctest job to GitHub Actions workflow
  - Runs `cargo test --doc` to validate all documentation examples
  - Runs `cargo doc` with `-D warnings` flag to catch documentation issues
  - Ensures documentation stays up-to-date and accurate
  - Uses caching for faster CI builds

### Changed

- **Documentation Quality**: Enhanced all public API documentation with:
  - Clear descriptions of purpose and behavior
  - Complete parameter and return value documentation
  - Error conditions and edge cases
  - Platform-specific feature notes (Linux TCP stats, file descriptors)
  - Real-world usage examples for common scenarios

### Technical Details

- Documentation can be generated locally with `cargo doc --open`
- All examples are tested in CI to ensure they remain valid
- Documentation follows Rust best practices with proper linking and formatting
- Platform-specific features (Linux TCP_INFO) are clearly marked
- Total additions: 967 lines of documentation across 6 files

## [0.3.8] - 2025-12-02

### Added

- **Crates.io Publishing Metadata**: Added required package metadata for crates.io publishing
  - Added `description` field with project summary
  - Added `license` field with dual MIT/Apache-2.0 licensing
  - Added `repository` URL pointing to GitHub
  - Added `readme` reference to README.md
  - Added `keywords` for discoverability (network, benchmarking, performance, iperf, bandwidth)
  - Added `categories` for crates.io classification (network-programming, command-line-utilities)

### Changed

- **Documentation**: Enhanced README.md with more comprehensive project description
  - Added detailed "What is rperf3-rs?" section explaining use cases
  - Added "Key Capabilities" highlighting main features
  - Added "Why rperf3-rs?" section covering performance, safety, and developer benefits
  - Adjusted tone to be more approachable and modest

### Fixed

- **Publishing Error**: Resolved crates.io publishing failure (status 400)
  - Fixed "missing or empty metadata fields: description, license" error
  - Package now meets all crates.io requirements for publishing

## [0.3.7] - 2025-12-02

### Fixed

- **Windows Build Failures**: Corrected cross-rs usage for Linux-only targets
  - Windows i686 and ARM64 now use native MSVC toolchain
  - cross-rs restricted to Linux targets only (armv7, musl, i686)
  - Fixes GitHub Actions job 56985371251 failure
  - cross-rs doesn't support Windows targets - only Linux

### Changed

- **Build System Simplification**: Streamlined cross-compilation logic
  - Install cross-rs only for Linux complex targets
  - Windows builds exclusively use native toolchain
  - Simplified build conditionals for better maintainability
  - Removed disabled crates.io publishing job entirely

### Technical Details

- cross-rs provides Docker images for Linux cross-compilation only
- Windows MSVC toolchain handles all Windows targets natively
- Cleaner workflow reduces build complexity and potential errors

## [0.3.6] - 2025-12-02

### Fixed

- **aarch64 GitHub Actions Build**: Configured linker for ARM64 cross-compilation in CI/CD
  - Set `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER` environment variable
  - Points to `gcc-aarch64-linux-gnu` cross-compiler
  - Fixes build failure in GitHub Actions release workflow
  - Ensures successful artifact generation for ARM64 GNU target

### Technical Details

- cargo requires explicit linker configuration for cross-compilation targets
- Environment variable automatically set during cross-compiler installation
- Matches local development build configuration
- Completes the cross-compilation infrastructure from v0.3.5

## [0.3.5] - 2025-12-02

### Fixed

- **Cross-Compilation Build Failures**: Resolved ARMv7 and complex target build issues
  - Integrated cross-rs for reliable cross-compilation of complex targets
  - Fixed ARMv7 build failure (exit code 101) by using Docker-based builds
  - Improved build reliability for musl and i686 targets
  - Docker-based environments ensure consistent cross-compilation results

### Changed

- **Build System Improvements**: Enhanced cross-compilation infrastructure
  - Use cross-rs for: ARMv7, all musl variants (x86_64, ARM64), i686
  - Use native cargo for: x86_64 GNU, ARM64 GNU, macOS, Windows
  - Simplified toolchain setup by leveraging cross-rs Docker images
  - More reliable builds across all 11 platform variants

### Technical Details

- cross-rs provides pre-configured build environments with proper linkers
- Eliminates manual gcc cross-compiler configuration complexity
- Ensures consistent builds regardless of CI runner environment
- Docker-based isolation prevents dependency conflicts

## [0.3.4] - 2025-12-02

### Added

- **SHA256 Checksums for Release Artifacts**: Enhanced security and integrity verification
  - Automatic generation of SHA256 checksums for all release binaries
  - Checksum files uploaded alongside each artifact (`.sha256` extension)
  - Enables users to verify download integrity and authenticity
  - Platform-specific checksum generation (certutil on Windows, shasum on Unix)
  - 22 total files per release: 11 binaries + 11 checksum files

### Security

- Release artifacts can now be cryptographically verified
- Protection against corrupted or tampered downloads
- Follows industry standard security practices for binary distribution

## [0.3.3] - 2025-12-02

### Added

- **Comprehensive Cross-Platform Build Support**: Extended release workflow to build 11 OS/architecture combinations
  - **Linux (6 variants)**:
    - x86_64 GNU (standard glibc)
    - x86_64 musl (static binary, no runtime dependencies)
    - ARM64/aarch64 GNU
    - ARM64/aarch64 musl (static binary)
    - ARMv7 (32-bit ARM for Raspberry Pi and embedded devices)
    - i686 (32-bit x86)
  - **macOS (2 variants)**:
    - x86_64 (Intel processors)
    - ARM64 (Apple Silicon M1/M2/M3)
  - **Windows (3 variants)**:
    - x86_64 (64-bit)
    - i686 (32-bit)
    - ARM64 (ARM-based Windows devices)
- Enhanced cross-compilation toolchain setup for Linux targets
- Improved artifact naming and organization in releases
- Static musl binaries for containerized and minimal Linux environments

### Changed

- Release workflow now builds for all major platforms and architectures
- Added proper cross-compilation dependencies installation
- Improved build matrix organization and documentation

## [0.3.2] - 2025-12-02

### Fixed

- **GitHub Actions Release Workflow**: Fixed "Resource not accessible by integration" error
  - Added `contents: write` permission at workflow and job levels
  - Replaced deprecated `actions/create-release@v1` with `softprops/action-gh-release@v1`
  - Replaced deprecated `actions/upload-release-asset@v1` with modern alternative
  - Enabled automatic release notes generation
  - Temporarily disabled crates.io publishing until CARGO_TOKEN is configured
  - Release workflow now properly creates GitHub releases and uploads artifacts

## [0.3.1] - 2025-12-02

### Fixed

- **Connection Error Handling**: Fixed "Connection reset by peer" and "early eof" errors
  - Client now handles connection errors gracefully when reading final messages
  - Result message reading wrapped in error handling
  - Done message reading wrapped in error handling
  - Test completes successfully even if server closes connection early
  - Debug logging instead of fatal errors for expected connection closures

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
