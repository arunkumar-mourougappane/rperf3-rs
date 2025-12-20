# Quick Start Guide: rperf3 Web UI

## What Was Built

A complete web application for rperf3 network performance testing with:

### Backend (Rust/Axum)
✅ REST API for starting/managing tests
✅ Server-Sent Events (SSE) for real-time progress streaming
✅ Integration with rperf3 client library
✅ Support for TCP and UDP protocols
✅ Concurrent test execution

### Frontend (HTML/JavaScript/Plotly.js)
✅ User-friendly form for test configuration
✅ Real-time charts for throughput, jitter, packet loss, retransmits
✅ SSE client for live updates
✅ Multiple simultaneous test tracking
✅ Professional UI with gradient styling

## Running Your First Test

### Step 1: Start an rperf3 Server

In one terminal:
```bash
cd /home/amouroug/rperf3-rs
cargo run --bin rperf3 -- server
```

You should see:
```
Server listening on 0.0.0.0:5201
```

### Step 2: Start the Web UI

In another terminal:
```bash
cd /home/amouroug/rperf3-rs/web-ui
cargo run --release
```

You should see:
```
Starting rperf3 Web UI server on http://0.0.0.0:3000
API endpoints available at http://0.0.0.0:3000/api
```

### Step 3: Open Your Browser

Navigate to: **http://localhost:3000**

You'll see the rperf3 Web UI with:
- Test configuration form at the top
- Active tests display area below

### Step 4: Run a Test

1. **Fill in the form:**
   - Protocol: TCP (or UDP)
   - Server IP: `127.0.0.1` (for localhost)
   - Port: `5201`
   - Duration: `10` seconds
   - Leave bandwidth empty for TCP
   - Reverse Mode: unchecked

2. **Click "Start Test"**

3. **Watch the magic happen!**
   - A new test panel appears
   - Real-time chart updates every second
   - Throughput line graph shows performance
   - For TCP: Retransmits bar chart
   - For UDP: Jitter and packet loss graphs

4. **View results**
   - After 10 seconds, test completes
   - Final statistics appear below the chart
   - Status badge changes to "Completed" (green)

## Testing Different Scenarios

### UDP Test with Bandwidth Limit
```
Protocol: UDP
Server: 127.0.0.1
Port: 5201
Duration: 15
Bandwidth: 100000000  (100 Mbps)
Reverse: unchecked
```

### Reverse Mode (Server Sends to Client)
```
Protocol: TCP
Server: 127.0.0.1
Port: 5201
Duration: 10
Reverse: ✓ (checked)
```

### Network Test to Remote Server
```
Protocol: TCP
Server: 192.168.1.100  (your server IP)
Port: 5201
Duration: 30
```

## Understanding the Metrics

### TCP Metrics
- **Throughput (Mbps)**: Data transfer rate over time (blue line)
- **Retransmits**: Number of TCP retransmissions (red bars)
- **Final Results**: Average throughput, total data transferred, duration

### UDP Metrics
- **Throughput (Mbps)**: Data transfer rate (blue line)
- **Jitter (ms)**: Network delay variation (green line)
- **Packet Loss (%)**: Percentage of lost packets (red line)
- **Final Results**: Throughput, packet stats, jitter, loss percentage

## Real-Time Features

- **Multiple Tests**: Start multiple tests to see them all updating simultaneously
- **Live Updates**: Charts update every second with new data points
- **SSE Connection**: Efficient server push updates (no polling!)
- **Auto-scroll**: New tests appear at the top

## Architecture Overview

```
Browser (localhost:3000)
    ↓ (HTTP POST /api/tests)
Axum Web Server
    ↓
Test Manager (spawns tokio task)
    ↓
rperf3 Client (with callback)
    ↓ (ProgressEvent)
Broadcast Channel
    ↓ (SSE stream /api/tests/:id/sse)
Browser (EventSource)
    ↓
JavaScript (updates Plotly chart)
```

## API Examples

### Start a Test (curl)
```bash
curl -X POST http://localhost:3000/api/tests \
  -H "Content-Type: application/json" \
  -d '{
    "protocol": "Tcp",
    "server": "127.0.0.1",
    "port": 5201,
    "duration": 10,
    "reverse": false
  }'
```

Response:
```json
{"test_id":"550e8400-e29b-41d4-a716-446655440000"}
```

### Subscribe to SSE (browser)
```javascript
const eventSource = new EventSource('/api/tests/550e8400-e29b-41d4-a716-446655440000/sse');

eventSource.addEventListener('interval_update', (e) => {
  const data = JSON.parse(e.data);
  console.log('Throughput:', data.bits_per_second / 1_000_000, 'Mbps');
});
```

### List All Tests
```bash
curl http://localhost:3000/api/tests
```

## Troubleshooting

### Problem: "Connection refused" in browser
**Solution**: Make sure the web server is running (`cargo run` in web-ui/)

### Problem: Test fails immediately
**Solution**:
1. Check that rperf3 server is running
2. Verify server IP address is correct
3. Check firewall settings
4. Look at server logs for errors

### Problem: No chart updates
**Solution**:
1. Open browser Developer Tools (F12)
2. Check Console tab for JavaScript errors
3. Check Network tab for SSE connection
4. Refresh the page and try again

### Problem: Build errors
**Solution**:
```bash
# Clean and rebuild
cargo clean
cargo build --release
```

## Next Steps

### For Development
- Modify `frontend/dist/app.js` to customize behavior
- Edit `frontend/dist/styles.css` for different styling
- Add new API endpoints in `src/api/routes.rs`

### For Production
1. Build release version: `cargo build --release`
2. Binary located at: `web-ui/target/release/rperf3-web`
3. Run with: `./web-ui/target/release/rperf3-web`
4. Consider reverse proxy (nginx) for HTTPS

### Future Enhancements
- Convert to Yew/WASM for full Rust stack
- Add test history and database storage
- Export results to CSV/JSON
- Compare multiple test results
- Add authentication for multi-user support

## Performance Testing Tips

1. **Baseline Test**: Run a local test first (127.0.0.1) to establish maximum performance
2. **Multiple Streams**: The CLI supports parallel streams, but web UI doesn't yet - this can be added
3. **Long Tests**: For stable measurements, run 30-60 second tests
4. **UDP Bandwidth**: Start with conservative values (50-100 Mbps) and increase

## Support

- Check `web-ui/README.md` for detailed documentation
- View server logs for debugging: `RUST_LOG=debug cargo run`
- Open browser console for client-side issues
- Report issues on GitHub

Enjoy testing your network performance! 🚀
