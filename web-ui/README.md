# rperf3 Web UI

A web-based interface for rperf3 network performance testing with real-time visualization using Plotly.js.

## Features

- **Real-time Testing**: Start TCP and UDP network performance tests from your browser
- **Live Visualization**: Watch throughput, jitter, packet loss, and retransmits update in real-time
- **Multiple Tests**: Run multiple tests simultaneously, each with its own chart
- **Protocol Support**:
  - **TCP**: Throughput and retransmits visualization
  - **UDP**: Throughput, jitter, and packet loss tracking
- **Server-Sent Events (SSE)**: Efficient real-time updates from backend to frontend

## Architecture

### Backend (Rust/Axum)
- `src/api/` - REST API endpoints and SSE streaming
- `src/test_manager/` - Test execution and state management
- `src/main.rs` - Axum web server

### Frontend (HTML/JS/Plotly)
- `frontend/dist/index.html` - Main UI
- `frontend/dist/app.js` - Client logic and SSE handling
- `frontend/dist/styles.css` - Styling

## Quick Start

### Prerequisites
1. You need a running rperf3 server to test against:
```bash
# In one terminal, start an rperf3 server
cargo run --bin rperf3 -- server
```

### Running the Web UI

```bash
# Navigate to web-ui directory
cd web-ui

# Run the web server (development)
cargo run

# Or build and run release version
cargo build --release
./target/release/rperf3-web
```

The web UI will be available at: **http://localhost:3000**

## Usage

1. Open your browser to `http://localhost:3000`

2. Fill in the test configuration:
   - **Protocol**: TCP or UDP
   - **Server IP**: IP address of your rperf3 server (e.g., `127.0.0.1` for localhost)
   - **Port**: Server port (default: 5201)
   - **Duration**: Test duration in seconds
   - **Bandwidth** (optional): Target bandwidth in bits per second (mainly for UDP)
   - **Reverse Mode**: Check to have server send data to client

3. Click **Start Test**

4. Watch real-time metrics:
   - **TCP**: Throughput and retransmit graph
   - **UDP**: Throughput, jitter, and packet loss graphs

5. View final results when test completes

## API Endpoints

- `POST /api/tests` - Start a new test
- `GET /api/tests` - List all tests
- `GET /api/tests/:test_id` - Get test status
- `GET /api/tests/:test_id/sse` - SSE stream for test progress
- `DELETE /api/tests/:test_id` - Cancel a test

## Example Test Configuration

### TCP Test (Local)
```json
{
  "protocol": "Tcp",
  "server": "127.0.0.1",
  "port": 5201,
  "duration": 10,
  "reverse": false
}
```

### UDP Test with Bandwidth Limit
```json
{
  "protocol": "Udp",
  "server": "192.168.1.100",
  "port": 5201,
  "duration": 30,
  "bandwidth": 100000000,
  "reverse": false
}
```

## Development

### Testing the Backend
```bash
# Run tests
cargo test

# Check compilation
cargo check

# Run with debug logging
RUST_LOG=debug cargo run
```

### Frontend Development
The current frontend is vanilla HTML/JS. To modify:
1. Edit files in `frontend/dist/`
2. Refresh browser to see changes

## Future Enhancements

The current implementation uses vanilla HTML/JavaScript for simplicity. Future versions could include:

- **Yew/WASM Frontend**: Full Rust frontend for type safety
- **Test History**: Persist and compare test results
- **Export Results**: Download results as JSON/CSV
- **Advanced Charting**: Zoom, pan, custom time ranges
- **Multi-server Testing**: Test multiple servers simultaneously
- **Authentication**: Multi-user support

## Troubleshooting

### Cannot connect to server
- Ensure rperf3 server is running: `cargo run --bin rperf3 -- server`
- Check firewall settings
- Verify server IP address is correct

### SSE connection fails
- Check browser console for errors
- Ensure CORS is enabled (handled by default)
- Try refreshing the page

### No data in charts
- Verify test actually started (check server logs)
- Open browser developer tools Network tab to see SSE events
- Check for JavaScript errors in console

## License

Same as rperf3-rs: MIT OR Apache-2.0
