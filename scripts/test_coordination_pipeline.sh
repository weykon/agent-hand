#!/usr/bin/env bash
# test_coordination_pipeline.sh — Shell smoke test for the coordination pipeline.
#
# Creates a temp test profile, writes a synthetic FeedbackPacket to
# feedback_packets.jsonl, and documents the expected output artifacts.
#
# Usage:
#   ./scripts/test_coordination_pipeline.sh
#
# NOTE: The actual pipeline runs inside Rust (ActionExecutor::run_coordination_pipeline).
# This script is a documentation / manual-verification aid that:
#   1. Creates a temp test profile directory
#   2. Writes a synthetic FeedbackPacket JSON to feedback_packets.jsonl
#   3. Lists expected output files and their format

set -euo pipefail

# ── 1. Create temp test profile ──────────────────────────────────────
TEST_DIR="$(mktemp -d)"
RUNTIME_DIR="${TEST_DIR}/agent-runtime"
mkdir -p "${RUNTIME_DIR}"

echo "==> Created temp test profile at: ${TEST_DIR}"
echo "    Runtime dir: ${RUNTIME_DIR}"

# ── 2. Write synthetic FeedbackPacket ────────────────────────────────
PACKET_ID="pkt-smoke-$(date +%s)"
TRACE_ID="trace-smoke-$(date +%s)"

cat > "${RUNTIME_DIR}/feedback_packets.jsonl" <<EOF
{"packet_id":"${PACKET_ID}","trace_id":"${TRACE_ID}","source_session_id":"session-smoke","created_at_ms":1700000005000,"goal":"finish auth integration","now":"coordinate follow-up","done_this_turn":["implemented auth adapter"],"blockers":["db connection timeout"],"decisions":["switched to connection pool"],"findings":["latency spike at 3pm"],"next_steps":["monitor pool metrics"],"affected_targets":["session-beta"],"source_refs":["commit:abc123"],"urgency_level":"Medium","recommended_response_level":"L2SelfInject"}
EOF

echo "==> Wrote synthetic FeedbackPacket:"
echo "    packet_id: ${PACKET_ID}"
echo "    trace_id:  ${TRACE_ID}"
echo ""

# ── 3. Expected output files ────────────────────────────────────────
echo "==> Expected output files from run_coordination_pipeline():"
echo ""
echo "  File                              Format         Description"
echo "  ────────────────────────────────  ─────────────  ───────────────────────────────────"
echo "  candidate_sets.jsonl              JSONL          Hot Brain candidate set output"
echo "  scheduler_outputs.jsonl           JSONL          Normalized scheduler hint records"
echo "  scheduler_state.json              JSON snapshot  Bounded scheduler state (pending/review/followup)"
echo "  followup_proposals.jsonl          JSONL          Follow-up proposal records"
echo "  followup_proposals_snapshot.json  JSON snapshot  Array of follow-up proposal records"
echo "  memory_ingest_entries.jsonl       JSONL          Validated memory candidates for promotion"
echo "  cold_memory.jsonl                 JSONL          Promoted cold memory records"
echo "  cold_memory_snapshot.json         JSON snapshot  Array of cold memory records"
echo ""

# ── 4. Verify the seed file ─────────────────────────────────────────
echo "==> Verifying seed file is valid JSON..."
if python3 -c "import json, sys; json.loads(open(sys.argv[1]).readline())" "${RUNTIME_DIR}/feedback_packets.jsonl" 2>/dev/null; then
    echo "    ✓ feedback_packets.jsonl is valid JSON"
else
    echo "    ✗ feedback_packets.jsonl is NOT valid JSON"
    exit 1
fi

echo ""
echo "==> Seed data is ready at: ${RUNTIME_DIR}/feedback_packets.jsonl"
echo "    To run the actual pipeline, use: cargo test --lib runner::tests"
echo ""
echo "==> Cleaning up temp directory..."
rm -rf "${TEST_DIR}"
echo "    Done."
