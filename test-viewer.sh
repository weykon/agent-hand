#!/bin/bash
# End-to-end test for viewer functionality

set -e

echo "=== Agent Hand Viewer E2E Test ==="
echo

# Clean up any existing processes
cleanup() {
    echo "Cleaning up..."
    kill $HOST_PID $VIEWER_PID 2>/dev/null || true
    rm -f /tmp/host-test.log /tmp/viewer-test.log
}
trap cleanup EXIT

# Build release binary
echo "1. Building release binary..."
cargo build --release --features pro --quiet
echo "   ✓ Build complete"
echo

# Start host
echo "2. Starting host (sharing session d798c305-858)..."
./target/release/agent-hand share d798c305-858 > /tmp/host-test.log 2>&1 &
HOST_PID=$!
sleep 3

# Extract share URL
SHARE_URL=$(grep "Share URL:" /tmp/host-test.log | awk '{print $NF}')
if [ -z "$SHARE_URL" ]; then
    echo "   ✗ Failed to get share URL"
    cat /tmp/host-test.log
    exit 1
fi
echo "   ✓ Host started (PID: $HOST_PID)"
echo "   Share URL: $SHARE_URL"
echo

# Start viewer
echo "3. Starting viewer..."
./target/release/agent-hand join "$SHARE_URL" > /tmp/viewer-test.log 2>&1 &
VIEWER_PID=$!
sleep 5

# Check if viewer is still running
if ! ps -p $VIEWER_PID > /dev/null; then
    echo "   ✗ Viewer exited unexpectedly"
    cat /tmp/viewer-test.log
    exit 1
fi
echo "   ✓ Viewer started (PID: $VIEWER_PID)"
echo

# Capture viewer output
echo "4. Capturing viewer output..."
sleep 2
kill $VIEWER_PID 2>/dev/null || true
wait $VIEWER_PID 2>/dev/null || true

# Analyze output
echo "5. Analyzing output..."
LINES=$(wc -l < /tmp/viewer-test.log)
echo "   Total lines captured: $LINES"

# Check for key indicators
if grep -q "Connected!" /tmp/viewer-test.log; then
    echo "   ✓ Viewer connected successfully"
else
    echo "   ✗ Viewer did not connect"
fi

if grep -q "Claude Code" /tmp/viewer-test.log; then
    echo "   ✓ Content received from host"
else
    echo "   ✗ No content received"
fi

# Check logs for dimension info
echo
echo "6. Checking dimension logs..."
LOG_FILE="$HOME/Library/Application Support/agent-hand/agent-hand.log"
if [ -f "$LOG_FILE" ]; then
    echo "   Recent VIEWER RENDER logs:"
    grep "VIEWER RENDER" "$LOG_FILE" | tail -3 | sed 's/^/     /'
    echo
    echo "   Recent Resize logs:"
    grep "Received Resize" "$LOG_FILE" | tail -3 | sed 's/^/     /'
else
    echo "   ✗ Log file not found"
fi

echo
echo "=== Test Complete ==="
echo
echo "To view full output:"
echo "  Host:   cat /tmp/host-test.log"
echo "  Viewer: cat /tmp/viewer-test.log"
