# Release Notes - Version 0.3.4

**Release Date:** December 2, 2025

## Overview

Version 0.3.4 introduces SHA256 checksum generation for all release artifacts, enhancing security and enabling users to verify the integrity and authenticity of downloaded binaries. This security-focused release ensures that users can confidently deploy rperf3 in production environments with cryptographic verification.

## Major Enhancement: SHA256 Checksums for All Artifacts

### What's New

Every release artifact now includes an accompanying `.sha256` checksum file:

- **22 files per release**: 11 binaries + 11 SHA256 checksum files
- **Platform-specific generation**: Uses native tools (certutil on Windows, shasum on Unix)
- **Automatic verification**: Checksums generated during build and uploaded to GitHub releases

### Security Benefits

#### Download Integrity Verification
Users can now verify that downloaded binaries haven't been corrupted during transfer:
```bash
# Linux/macOS
shasum -a 256 -c rperf3-linux-x86_64.sha256

# Windows PowerShell
$hash = (Get-FileHash rperf3-windows-x86_64.exe -Algorithm SHA256).Hash
$expected = Get-Content rperf3-windows-x86_64.exe.sha256
if ($hash -eq $expected.Split()[0]) { "Valid" } else { "Invalid" }
```

#### Tamper Detection
Checksums protect against:
- Network transmission errors
- Storage corruption
- Man-in-the-middle attacks
- Unauthorized binary modifications

#### Industry Standard Practice
- Follows security best practices for binary distribution
- Common requirement for enterprise deployment
- Essential for compliance and audit requirements
- Aligns with supply chain security standards

## Artifact Coverage

All 11 platform variants include checksums:

### Linux (6 variants with checksums)
- `rperf3-linux-x86_64` + `.sha256`
- `rperf3-linux-x86_64-musl` + `.sha256`
- `rperf3-linux-aarch64` + `.sha256`
- `rperf3-linux-aarch64-musl` + `.sha256`
- `rperf3-linux-armv7` + `.sha256`
- `rperf3-linux-i686` + `.sha256`

### macOS (2 variants with checksums)
- `rperf3-macos-x86_64` + `.sha256`
- `rperf3-macos-aarch64` + `.sha256`

### Windows (3 variants with checksums)
- `rperf3-windows-x86_64.exe` + `.sha256`
- `rperf3-windows-i686.exe` + `.sha256`
- `rperf3-windows-aarch64.exe` + `.sha256`

## Verification Examples

### Linux/macOS Quick Verification
```bash
# Download binary and checksum
curl -LO https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.4/rperf3-linux-x86_64
curl -LO https://github.com/arunkumar-mourougappane/rperf3-rs/releases/download/v0.3.4/rperf3-linux-x86_64.sha256

# Verify
shasum -a 256 -c rperf3-linux-x86_64.sha256
# Output: rperf3-linux-x86_64: OK
```

### Windows Verification (CMD)
```cmd
:: Download files first
certutil -hashfile rperf3-windows-x86_64.exe SHA256
type rperf3-windows-x86_64.exe.sha256
:: Compare the hashes manually
```

### Windows Verification (PowerShell)
```powershell
# Automated verification
$binary = "rperf3-windows-x86_64.exe"
$checksumFile = "$binary.sha256"

$actualHash = (Get-FileHash $binary -Algorithm SHA256).Hash
$expectedHash = (Get-Content $checksumFile).Split()[0]

if ($actualHash -eq $expectedHash) {
    Write-Host "✓ Checksum verified successfully" -ForegroundColor Green
} else {
    Write-Host "✗ Checksum verification failed!" -ForegroundColor Red
}
```

### macOS Homebrew-style Verification
```bash
# Single command verification
echo "$(cat rperf3-macos-aarch64.sha256)" | shasum -a 256 -c -
```

## Use Cases

### Enterprise Deployment
- Compliance requirements for software verification
- Audit trail for binary provenance
- Security policy enforcement

### CI/CD Pipelines
```yaml
# GitHub Actions example
- name: Download and verify rperf3
  run: |
    curl -LO https://github.com/.../rperf3-linux-x86_64
    curl -LO https://github.com/.../rperf3-linux-x86_64.sha256
    shasum -a 256 -c rperf3-linux-x86_64.sha256
    chmod +x rperf3-linux-x86_64
```

### Containerized Deployments
```dockerfile
FROM alpine:latest
WORKDIR /app

# Download with verification
RUN apk add --no-cache curl \
    && curl -LO https://github.com/.../rperf3-linux-x86_64-musl \
    && curl -LO https://github.com/.../rperf3-linux-x86_64-musl.sha256 \
    && sha256sum -c rperf3-linux-x86_64-musl.sha256 \
    && chmod +x rperf3-linux-x86_64-musl \
    && mv rperf3-linux-x86_64-musl /usr/local/bin/rperf3

ENTRYPOINT ["/usr/local/bin/rperf3"]
```

### Air-Gapped Environments
- Verify binaries before transferring to isolated networks
- Ensure no corruption during offline transfer
- Document integrity for security reviews

## Technical Implementation

### Checksum Generation Process
1. Binary compiled with release optimizations
2. Binary stripped (Linux/macOS) to reduce size
3. SHA256 hash computed using platform tools:
   - **Unix**: `shasum -a 256`
   - **Windows**: `certutil -hashfile`
4. Checksum saved as `{binary_name}.sha256`
5. Both binary and checksum uploaded to GitHub release

### Checksum File Format
Unix format (Linux/macOS):
```
abc123def456...  rperf3-linux-x86_64
```

Windows format:
```
SHA256 hash of file rperf3-windows-x86_64.exe:
abc123def456...
CertUtil: -hashfile command completed successfully.
```

## Breaking Changes

None. This release is fully backward compatible with 0.3.3.

## Upgrade Notes

Simply download the new version and verify it using the checksum file. No configuration changes required.

## Migration from Previous Versions

If you're upgrading from v0.3.3 or earlier:
1. Download both the binary and `.sha256` file
2. Verify the checksum (optional but recommended)
3. Replace your existing binary
4. No other changes needed

## Known Limitations

- Checksum verification is optional (not enforced by the download process)
- Users must manually verify checksums before use
- Requires basic command-line knowledge for verification

## Performance Impact

- No runtime performance impact
- Minimal build time increase (~1-2 seconds per platform)
- Negligible storage overhead (checksums are ~65 bytes each)

## What's Next

The v0.3.5 roadmap may include:
- GPG signature support for even stronger verification
- Automated verification scripts
- Integration with package managers
- SBOM (Software Bill of Materials) generation

## Getting Help

- **Documentation**: [README.md](https://github.com/arunkumar-mourougappane/rperf3-rs/blob/main/README.md)
- **Security**: Report security concerns to the maintainers
- **Issues**: [GitHub Issues](https://github.com/arunkumar-mourougappane/rperf3-rs/issues)
- **Discussions**: [GitHub Discussions](https://github.com/arunkumar-mourougappane/rperf3-rs/discussions)

## Contributors

- Arunkumar Mourougappane (@arunkumar-mourougappane)

## Full Changelog

See [CHANGELOG.md](https://github.com/arunkumar-mourougappane/rperf3-rs/blob/main/CHANGELOG.md) for detailed changes.
