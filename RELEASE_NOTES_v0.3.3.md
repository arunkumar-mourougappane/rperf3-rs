# Release Notes - Version 0.3.3

**Release Date:** December 2, 2025

## Overview

Version 0.3.3 significantly expands platform and architecture support, making rperf3 available for 11 different OS/architecture combinations. This release focuses on broadening deployment options across diverse hardware platforms, from embedded devices to modern cloud infrastructure.

## Major Enhancement: Comprehensive Cross-Platform Support

### Expanded Platform Coverage

This release introduces automated builds for **11 OS/architecture combinations**, up from the previous 5:

#### Linux (6 variants)
- **x86_64 GNU** - Standard glibc-based binary for most Linux distributions
- **x86_64 musl** - Static binary with no runtime dependencies (ideal for containers and minimal environments)
- **ARM64 GNU** - For modern ARM servers and devices (AWS Graviton, Raspberry Pi 4/5)
- **ARM64 musl** - Static binary for ARM-based containers and embedded systems
- **ARMv7** - 32-bit ARM support for Raspberry Pi 2/3, older embedded devices
- **i686** - 32-bit x86 support for legacy systems

#### macOS (2 variants)
- **x86_64** - Intel-based Macs
- **ARM64** - Apple Silicon (M1/M2/M3/M4)

#### Windows (3 variants)
- **x86_64** - Standard 64-bit Windows
- **i686** - 32-bit Windows support
- **ARM64** - ARM-based Windows devices (Surface Pro X, etc.)

### Key Benefits

#### Static Binaries (musl targets)
- No runtime dependencies required
- Perfect for containerized deployments (Alpine, scratch images)
- Works on older Linux distributions without glibc compatibility issues
- Simplified deployment in minimal environments

#### Embedded Systems Support
- ARMv7 support for Raspberry Pi and IoT devices
- Enables network performance testing on edge computing platforms
- Suitable for industrial automation and monitoring systems

#### Wide Hardware Compatibility
- Support for legacy 32-bit systems (i686)
- Modern ARM server support (AWS Graviton, Ampere)
- Apple Silicon native performance
- Windows ARM devices coverage

## Technical Improvements

### Enhanced Build Infrastructure
- Automated cross-compilation toolchain setup
- Proper dependency management for each target platform
- Improved artifact naming scheme for clarity
- Robust build matrix with comprehensive platform coverage

### Quality Assurance
- All binaries built with release optimizations
- Stripped binaries for reduced file size (Linux/macOS)
- Consistent naming convention across all platforms
- Automated upload to GitHub releases

## Deployment Scenarios

### Docker/Container Environments
Use the musl static binaries for minimal container images:
```dockerfile
FROM scratch
COPY rperf3-linux-x86_64-musl /rperf3
ENTRYPOINT ["/rperf3"]
```

### Raspberry Pi / IoT
ARM variants provide native performance on embedded devices:
- ARMv7 for Pi 2/3 and similar devices
- ARM64 for Pi 4/5 and newer hardware

### Cloud Infrastructure
- x86_64 GNU for traditional VMs
- ARM64 for cost-effective ARM-based instances (AWS Graviton, Azure Ampere)
- musl variants for containerized workloads

### Legacy Systems
- i686 Linux for older 32-bit servers
- i686 Windows for legacy Windows installations

## Installation

Download the appropriate binary for your platform from the [releases page](https://github.com/arunkumar-mourougappane/rperf3-rs/releases/tag/v0.3.3):

```bash
# Linux x86_64 (standard)
wget https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.3/rperf3-linux-x86_64
chmod +x rperf3-linux-x86_64

# Linux x86_64 (static, for containers)
wget https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.3/rperf3-linux-x86_64-musl
chmod +x rperf3-linux-x86_64-musl

# Linux ARM64 (Raspberry Pi 4/5, ARM servers)
wget https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.3/rperf3-linux-aarch64
chmod +x rperf3-linux-aarch64

# Linux ARMv7 (Raspberry Pi 2/3)
wget https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.3/rperf3-linux-armv7
chmod +x rperf3-linux-armv7

# macOS (Intel)
wget https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.3/rperf3-macos-x86_64
chmod +x rperf3-macos-x86_64

# macOS (Apple Silicon)
wget https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.3/rperf3-macos-aarch64
chmod +x rperf3-macos-aarch64
```

For Windows, download the `.exe` file for your architecture (x86_64, i686, or aarch64).

## Breaking Changes

None. This release is fully backward compatible with 0.3.2.

## Upgrade Notes

Simply replace your existing binary with the new version. No configuration changes required.

## Known Limitations

- Some cross-compilation targets may have slightly longer build times
- Windows ARM64 support is experimental (limited testing hardware availability)
- musl binaries may have minor performance differences compared to glibc versions

## What's Next

The v0.3.4 roadmap includes:
- Enhanced parallel stream support
- IPv6 improvements
- SCTP protocol support
- CPU utilization monitoring
- Additional output formats (CSV, XML)

## Getting Help

- **Documentation**: [README.md](https://github.com/arunkumar-mourougappane/rperf3-rs/blob/main/README.md)
- **Issues**: [GitHub Issues](https://github.com/arunkumar-mourougappane/rperf3-rs/issues)
- **Discussions**: [GitHub Discussions](https://github.com/arunkumar-mourougappane/rperf3-rs/discussions)

## Contributors

- Arunkumar Mourougappane (@arunkumar-mourougappane)

## Full Changelog

See [CHANGELOG.md](https://github.com/arunkumar-mourougappane/rperf3-rs/blob/main/CHANGELOG.md) for detailed changes.
