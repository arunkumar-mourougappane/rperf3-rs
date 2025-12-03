# rperf3-rs v0.3.1 Release Notes

## Bug Fix Release

This is a maintenance release that fixes a critical connection handling issue that could cause test failures.

## What's Fixed

### Connection Error Handling

Fixed a bug where the client would fail with "Connection reset by peer" or "early eof" errors when the server closed the connection before sending final protocol messages.

**Issue**: After completing data transfer, the client expected to receive Result and Done messages from the server. If the server closed the connection early (due to timeout, error, or other reasons), the client would fail with:
```
Error: IO error: early eof
Caused by: early eof
```

**Solution**: The client now handles connection errors gracefully:
- Result message reading is wrapped in error handling
- Done message reading is wrapped in error handling  
- Connection closures are logged as debug messages instead of causing fatal errors
- Tests complete successfully and display results even if the server closes the connection early

## Impact

This fix improves reliability when:
- Testing against servers with different timeout configurations
- Network connections are unstable
- Servers implement slightly different protocol timing
- Testing with reverse mode where server-side issues might cause early disconnection

## Installation

### From Source

```bash
git clone https://github.com/arunkumar-mourougappane/rperf3-rs.git
cd rperf3-rs
git checkout v0.3.1
cargo build --release
```

The binary will be available at `target/release/rperf3`.

## Usage

No changes to usage - all existing commands work the same:

```bash
# Start server
rperf3 server

# Run client test
rperf3 client <server-address>

# TCP test
rperf3 client 192.168.1.100 --time 10

# UDP test  
rperf3 client 192.168.1.100 --udp --bandwidth 100

# JSON output
rperf3 client 192.168.1.100 --json
```

## Upgrade Notes

If you're currently using v0.3.0, this is a drop-in replacement with no breaking changes. Simply update to v0.3.1 for improved reliability.

## Technical Details

**Changed Files**:
- `src/client.rs`: Modified `run_tcp()` to wrap result and done message deserialization in match expressions that catch and log errors instead of propagating them.

**Behavior Changes**:
- Connection errors during final message reading are now logged at DEBUG level
- Tests complete successfully and show "Test completed" message even without receiving Done message
- No change to test measurements or results accuracy

## Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for complete version history.

## Previous Release

For information about v0.3.0 features, see [RELEASE_NOTES.md](RELEASE_NOTES.md).

## Support

- **Repository**: https://github.com/arunkumar-mourougappane/rperf3-rs
- **Issues**: https://github.com/arunkumar-mourougappane/rperf3-rs/issues
- **Documentation**: See [README.md](README.md)

## Author

**Arunkumar Mourougappane**
- Email: amouroug@buffalo.edu
- GitHub: [@arunkumar-mourougappane](https://github.com/arunkumar-mourougappane)

---

**Released**: December 2, 2025  
**Version**: 0.3.1
