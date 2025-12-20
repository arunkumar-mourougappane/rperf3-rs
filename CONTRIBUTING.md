# Contributing to rperf3-rs

Thank you for your interest in contributing! This document provides guidelines for contributing to the project.

## Development Setup

### Prerequisites

- Rust 1.70 or later (install from [rustup.rs](https://rustup.rs/))
- Git

### Getting Started

```bash
# Clone the repository
git clone https://github.com/arunkumar-mourougappane/rperf3-rs.git
cd rperf3-rs

# Build the project
cargo build

# Run tests
cargo test

# Run clippy (linter)
cargo clippy

# Format code
cargo fmt
```

### Project Structure

```text
rperf3-rs/
├── src/
│   ├── lib.rs                      # Library root, exports public API
│   ├── bin/
│   │   └── main.rs                 # Primary CLI binary entry point
│   ├── batch_socket.rs             # Batch UDP operations (sendmmsg/recvmmsg)
│   ├── buffer_pool.rs              # Buffer pooling for memory efficiency
│   ├── client.rs                   # Client implementation with callback support
│   ├── config.rs                   # Configuration and builder patterns
│   ├── error.rs                    # Error types and handling
│   ├── interval_reporter.rs        # Async interval reporting system
│   ├── measurements.rs             # Statistics collection (TCP/UDP)
│   ├── protocol.rs                 # Protocol message definitions
│   ├── server.rs                   # Server implementation
│   ├── token_bucket.rs             # Token bucket bandwidth limiting
│   └── udp_packet.rs               # UDP packet structure and parsing
├── examples/                       # Usage examples
│   ├── server.rs                   # Basic server example
│   ├── client.rs                   # Basic client example
│   ├── udp_client.rs               # UDP client example
│   ├── client_with_callback.rs     # Callback example with custom struct
│   ├── client_closure_callback.rs  # Callback example with closures
│   ├── show_json_structure.rs      # TCP JSON output demonstration
│   ├── show_udp_json.rs            # UDP JSON output demonstration
│   ├── test_json_output.rs         # JSON serialization testing
│   ├── batch_send_test.rs          # Batch socket operations demo
│   ├── cancellable_test.rs         # Cancellation token usage
│   ├── tcp_nodelay_test.rs         # TCP_NODELAY optimization demo
│   ├── test_retransmits.rs         # TCP retransmit statistics
│   ├── token_bucket_demo.rs        # Token bucket algorithm demo
│   └── udp_buffer_test.rs          # UDP buffer size testing
├── tests/                          # Integration tests
│   ├── callback_tests.rs           # Callback functionality tests
│   └── integration_tests.rs        # End-to-end integration tests
├── docs/
│   └── TOKEN_BUCKET.md             # Token bucket algorithm documentation
├── CHANGELOG.md                    # Version history and features
├── CONTRIBUTING.md                 # This file
├── LICENSE                         # MIT/Apache-2.0 dual license
├── README.md                       # User documentation
└── RELEASE_NOTES.md                # Release notes and migration guides
```

## How to Contribute

### Reporting Bugs

Found a bug? Please open an issue with:

- Clear description of the problem
- Steps to reproduce
- Expected vs actual behavior
- System information (OS, Rust version)
- Relevant logs or error messages

### Suggesting Features

Have an idea? Open an issue with:

- Clear description of the feature
- Use cases and benefits
- Any implementation ideas

### Submitting Changes

1. **Fork** the repository on GitHub
2. **Create a branch** for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **Make your changes** with clear, focused commits
4. **Test your changes**:
   ```bash
   cargo test
   cargo clippy
   cargo fmt
   ```
5. **Push** to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```
6. **Open a Pull Request** with a clear description

## Code Standards

### Formatting

Use standard Rust formatting:

```bash
cargo fmt
```

### Linting

Ensure clippy passes:

```bash
cargo clippy -- -D warnings
```

### Testing

- Add tests for new functionality
- Ensure all existing tests pass
- Aim for meaningful test coverage

### Documentation

- Add doc comments for public APIs
- Update README.md for user-facing changes
- Keep inline comments clear and concise

### Commit Messages

Write clear commit messages:

```text
Add UDP packet loss measurement

- Implement packet loss tracking for UDP streams
- Add loss statistics to measurements
- Update JSON output format
```

## Pull Request Guidelines

Before submitting a PR:

- [ ] Code builds without warnings: `cargo build --release`
- [ ] All tests pass: `cargo test`
- [ ] Code is formatted: `cargo fmt`
- [ ] Clippy passes: `cargo clippy`
- [ ] Documentation is updated
- [ ] Commit messages are clear
- [ ] PR description explains the changes

## Development Tips

### Running the CLI during development

```bash
# Run server
cargo run --bin rperf3 -- server

# Run client
cargo run --bin rperf3 -- client 127.0.0.1
```

### Running with debug logging

```bash
RUST_LOG=debug cargo run --bin rperf3 -- server
```

### Running examples

```bash
# Server example
cargo run --example server

# Client example (in another terminal)
cargo run --example client

# UDP client
cargo run --example udp_client
```

### Testing network functionality

Since this is a network tool, manual testing is important:

1. Start a server in one terminal
2. Run clients from other terminals
3. Test different configurations (TCP/UDP, reverse mode, etc.)
4. Verify output is correct

## Priority Areas for Contribution

### High Priority

- UDP packet loss and jitter measurement
- Enhanced parallel stream support
- Comprehensive error handling improvements
- More integration tests

### Medium Priority

- IPv6 support enhancements
- SCTP protocol support
- TCP retransmission statistics
- CPU utilization monitoring

### Low Priority

- Additional output formats (CSV, XML)
- Web UI for results visualization
- Historical data storage
- Integration with monitoring systems

## Code Review Process

All submissions go through code review:

1. Automated checks (build, tests, clippy)
2. Manual review for:
   - Code quality and style
   - Test coverage
   - Documentation completeness
   - API design
   - Performance implications

Reviews typically happen within a few days. Be patient and responsive to feedback.

## Getting Help

- Open an issue for questions
- Check existing issues and PRs
- Read the documentation in README.md

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (MIT/Apache-2.0).
