// API base URL
const API_BASE = '/api';

// Active tests map
const activeTests = new Map();

// Initialize app
document.addEventListener('DOMContentLoaded', () => {
    const form = document.getElementById('test-form');
    form.addEventListener('submit', handleStartTest);

    // Update protocol-specific fields
    document.getElementById('protocol').addEventListener('change', (e) => {
        const bandwidthInput = document.getElementById('bandwidth');
        if (e.target.value === 'Udp') {
            bandwidthInput.placeholder = '100000000 (100 Mbps)';
            bandwidthInput.required = false;
        } else {
            bandwidthInput.placeholder = 'Optional for TCP';
            bandwidthInput.required = false;
        }
    });
});

// Handle test start
async function handleStartTest(e) {
    e.preventDefault();

    const protocol = document.getElementById('protocol').value;
    const server = document.getElementById('server').value;
    const port = parseInt(document.getElementById('port').value);
    const duration = parseInt(document.getElementById('duration').value);
    const bandwidth = document.getElementById('bandwidth').value;
    const reverse = document.getElementById('reverse').checked;

    const config = {
        protocol,
        server,
        port,
        duration,
        reverse
    };

    if (bandwidth) {
        config.bandwidth = parseInt(bandwidth);
    }

    try {
        const response = await fetch(`${API_BASE}/tests`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(config)
        });

        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }

        const { test_id } = await response.json();
        console.log('Test started:', test_id);

        // Create test panel
        createTestPanel(test_id, config);

        // Connect to SSE
        connectToTestStream(test_id, config);
    } catch (error) {
        console.error('Failed to start test:', error);
        alert('Failed to start test: ' + error.message);
    }
}

// Create test panel in UI
function createTestPanel(testId, config) {
    const container = document.getElementById('tests-container');

    const panel = document.createElement('div');
    panel.className = 'test-panel';
    panel.id = `test-${testId}`;

    panel.innerHTML = `
        <div class="test-header">
            <h3>${config.protocol} Test - ${config.server}:${config.port}</h3>
            <span class="status-badge status-running">Running</span>
        </div>
        <div class="chart-container" id="chart-${testId}"></div>
        <div class="results-summary" id="results-${testId}" style="display: none;"></div>
    `;

    container.insertBefore(panel, container.firstChild);

    // Initialize chart
    initializeChart(testId, config.protocol);
}

// Initialize Plotly chart
function initializeChart(testId, protocol) {
    const chartDiv = document.getElementById(`chart-${testId}`);

    const traces = protocol === 'Tcp' ? getTcpTraces() : getUdpTraces();
    const layout = getChartLayout(protocol);

    Plotly.newPlot(chartDiv, traces, layout, { responsive: true });

    activeTests.set(testId, {
        protocol,
        dataPoints: [],
        chartDiv
    });
}

function getTcpTraces() {
    return [
        {
            x: [],
            y: [],
            name: 'Throughput (Mbps)',
            type: 'scatter',
            mode: 'lines+markers',
            line: { color: '#667eea', width: 2 }
        },
        {
            x: [],
            y: [],
            name: 'Retransmits',
            type: 'bar',
            yaxis: 'y2',
            marker: { color: '#ff6b6b' }
        }
    ];
}

function getUdpTraces() {
    return [
        {
            x: [],
            y: [],
            name: 'Throughput (Mbps)',
            type: 'scatter',
            mode: 'lines+markers',
            line: { color: '#667eea', width: 2 }
        },
        {
            x: [],
            y: [],
            name: 'Jitter (ms)',
            type: 'scatter',
            mode: 'lines',
            yaxis: 'y2',
            line: { color: '#51cf66', width: 2 }
        },
        {
            x: [],
            y: [],
            name: 'Packet Loss (%)',
            type: 'scatter',
            mode: 'lines',
            yaxis: 'y2',
            line: { color: '#ff6b6b', width: 2 }
        }
    ];
}

function getChartLayout(protocol) {
    return {
        title: `${protocol} Performance Metrics`,
        xaxis: {
            title: 'Time (seconds)',
            showgrid: true
        },
        yaxis: {
            title: 'Throughput (Mbps)',
            showgrid: true
        },
        yaxis2: {
            title: protocol === 'Tcp' ? 'Retransmits' : 'Jitter (ms) / Loss (%)',
            overlaying: 'y',
            side: 'right'
        },
        legend: {
            x: 0,
            y: 1.1,
            orientation: 'h'
        },
        margin: { t: 50, r: 80, b: 50, l: 60 }
    };
}

// Connect to SSE stream
function connectToTestStream(testId, config) {
    const eventSource = new EventSource(`${API_BASE}/tests/${testId}/sse`);

    eventSource.addEventListener('test_started', () => {
        console.log('Test started:', testId);
    });

    eventSource.addEventListener('interval_update', (e) => {
        const data = JSON.parse(e.data);
        handleIntervalUpdate(testId, data, config.protocol);
    });

    eventSource.addEventListener('test_completed', (e) => {
        const data = JSON.parse(e.data);
        handleTestCompleted(testId, data, config.protocol);
        eventSource.close();
    });

    eventSource.addEventListener('error', (e) => {
        console.error('SSE error:', e);
        if (e.data) {
            const data = JSON.parse(e.data);
            handleTestError(testId, data.message);
        }
        eventSource.close();
    });

    eventSource.onerror = (e) => {
        if (eventSource.readyState === EventSource.CLOSED) {
            console.log('SSE connection closed');
        } else {
            console.error('SSE connection error');
        }
    };
}

// Handle interval update
function handleIntervalUpdate(testId, data, protocol) {
    const test = activeTests.get(testId);
    if (!test) return;

    const time = data.interval_end;
    const throughputMbps = data.bits_per_second / 1_000_000;

    const update = {
        x: [[time], [time]],
        y: [[throughputMbps]],
    };

    if (protocol === 'Tcp') {
        const retransmits = data.retransmits || 0;
        update.x.push([time]);
        update.y.push([retransmits]);
    } else {
        const jitter = data.jitter_ms || 0;
        const lossPercent = data.lost_percent || 0;
        update.x.push([time]);
        update.y.push([jitter], [lossPercent]);
    }

    Plotly.extendTraces(test.chartDiv, update, protocol === 'Tcp' ? [0, 1] : [0, 1, 2]);
}

// Handle test completion
function handleTestCompleted(testId, data, protocol) {
    console.log('Test completed:', testId, data);

    // Update status badge
    const panel = document.getElementById(`test-${testId}`);
    const statusBadge = panel.querySelector('.status-badge');
    statusBadge.textContent = 'Completed';
    statusBadge.className = 'status-badge status-completed';

    // Show results summary
    const resultsDiv = document.getElementById(`results-${testId}`);
    resultsDiv.style.display = 'block';

    const throughputMbps = (data.bits_per_second / 1_000_000).toFixed(2);
    const totalMB = (data.total_bytes / 1_000_000).toFixed(2);
    const duration = data.duration.toFixed(2);

    let resultsHTML = `
        <h4>Final Results</h4>
        <div class="results-grid">
            <div class="result-item">
                <div class="result-label">Average Throughput</div>
                <div class="result-value">${throughputMbps}<span class="result-unit">Mbps</span></div>
            </div>
            <div class="result-item">
                <div class="result-label">Total Data</div>
                <div class="result-value">${totalMB}<span class="result-unit">MB</span></div>
            </div>
            <div class="result-item">
                <div class="result-label">Duration</div>
                <div class="result-value">${duration}<span class="result-unit">s</span></div>
            </div>
    `;

    if (protocol === 'Udp') {
        const lossPercent = data.lost_percent ? data.lost_percent.toFixed(2) : '0.00';
        const jitter = data.jitter_ms ? data.jitter_ms.toFixed(3) : '0.000';
        const packets = data.total_packets || 0;

        resultsHTML += `
            <div class="result-item">
                <div class="result-label">Packet Loss</div>
                <div class="result-value">${lossPercent}<span class="result-unit">%</span></div>
            </div>
            <div class="result-item">
                <div class="result-label">Jitter</div>
                <div class="result-value">${jitter}<span class="result-unit">ms</span></div>
            </div>
            <div class="result-item">
                <div class="result-label">Total Packets</div>
                <div class="result-value">${packets}</div>
            </div>
        `;
    }

    resultsHTML += '</div>';
    resultsDiv.innerHTML = resultsHTML;
}

// Handle test error
function handleTestError(testId, message) {
    console.error('Test error:', testId, message);

    const panel = document.getElementById(`test-${testId}`);
    if (panel) {
        const statusBadge = panel.querySelector('.status-badge');
        statusBadge.textContent = 'Failed';
        statusBadge.className = 'status-badge status-failed';

        const resultsDiv = document.getElementById(`results-${testId}`);
        resultsDiv.style.display = 'block';
        resultsDiv.innerHTML = `
            <div style="color: #ff6b6b; padding: 15px; background: #ffe0e0; border-radius: 6px;">
                <strong>Error:</strong> ${message}
            </div>
        `;
    }
}
