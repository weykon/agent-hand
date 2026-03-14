#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────
# agent-hand preflight check
#
# Run this BEFORE pushing to catch CI failures locally.
# Mirrors what GitHub Actions does for the release build.
#
# Usage:
#   ./scripts/preflight.sh          # full check
#   ./scripts/preflight.sh --quick  # skip cross-platform + tests
# ─────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
DIM='\033[2m'
NC='\033[0m'

PASS=0
FAIL=0
WARN=0
QUICK=false
ERRORS=""

[[ "${1:-}" == "--quick" ]] && QUICK=true

fail()  { echo -e "${RED}  ✗ $1${NC}"; FAIL=$((FAIL + 1)); ERRORS="${ERRORS}\n  ✗ $1"; }
ok()    { echo -e "${GREEN}  ✓ $1${NC}"; PASS=$((PASS + 1)); }
warn()  { echo -e "${YELLOW}  ! $1${NC}"; WARN=$((WARN + 1)); }
info()  { echo -e "${DIM}    $1${NC}"; }
header(){ echo -e "\n${BLUE}─── $1 ───${NC}"; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo ""
echo -e "${BLUE}  agent-hand preflight check${NC}"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo -e "  ${DIM}version: ${VERSION}${NC}"
echo ""

# ── 1. Git state ──────────────────────────────────

header "Git State"

BRANCH=$(git branch --show-current 2>/dev/null || echo "detached")
echo -e "  branch: ${BRANCH}"

if [ -n "$(git status --porcelain -- src/ Cargo.toml Cargo.lock pro/src/ 2>/dev/null)" ]; then
    warn "Uncommitted changes in src/ or Cargo files"
    git status --short -- src/ Cargo.toml Cargo.lock pro/src/ 2>/dev/null | head -10 | while read -r line; do
        info "$line"
    done
else
    ok "Working tree clean (src/, Cargo)"
fi

# Check tool-adapters submodule
if [ -d "$ROOT/tool-adapters/.git" ] || [ -f "$ROOT/tool-adapters/.git" ]; then
    ok "tool-adapters submodule present"
else
    fail "tool-adapters/ missing — CI clones this from github.com/weykon/agent-hooks"
fi

# Check pro/ submodule
if [ -d "$ROOT/pro/.git" ]; then
    cd "$ROOT/pro"
    if [ -n "$(git status --porcelain -- src/ 2>/dev/null)" ]; then
        fail "pro/src/ has uncommitted changes"
    else
        ok "pro/ is clean"
    fi
    # Check if pushed
    LOCAL_HEAD=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
    REMOTE_HEAD=$(git rev-parse origin/main 2>/dev/null || echo "unknown")
    if [ "$LOCAL_HEAD" != "$REMOTE_HEAD" ] && [ "$REMOTE_HEAD" != "unknown" ]; then
        UNPUSHED=$(git log origin/main..HEAD --oneline 2>/dev/null | wc -l | tr -d ' ')
        warn "pro/ has ${UNPUSHED} unpushed commit(s) — CI pulls from remote"
    else
        ok "pro/ in sync with remote"
    fi
    cd "$ROOT"
else
    warn "pro/ directory not a git repo (Pro build will fail in CI)"
fi

# ── 2. Compile checks ────────────────────────────

header "Compilation (native target)"

# Free tier
info "cargo check (free)..."
if cargo check 2>&1 | grep -q "^error"; then
    fail "Free build: compilation errors"
    cargo check 2>&1 | grep "^error" | head -5 | while read -r line; do
        info "$line"
    done
else
    ok "Free build compiles"
fi

# Pro tier
info "cargo check --features pro..."
if cargo check --features pro 2>&1 | grep -q "^error"; then
    fail "Pro build: compilation errors"
    cargo check --features pro 2>&1 | grep "^error" | head -5 | while read -r line; do
        info "$line"
    done
else
    ok "Pro build compiles"
fi

# Max tier (optional — only if ai_api_provider is available)
if [ -d "$ROOT/../ai_api_provider" ] || cargo check --features max 2>&1 | grep -q "^error\[E0433\]"; then
    info "cargo check --features max..."
    if cargo check --features max 2>&1 | grep -q "^error"; then
        MAX_ERR=$(cargo check --features max 2>&1 | grep "^error" | head -1)
        if echo "$MAX_ERR" | grep -q "ai_api_provider"; then
            warn "Max build: ai_api_provider not available locally (OK — CI stubs it)"
        else
            fail "Max build: compilation errors"
            cargo check --features max 2>&1 | grep "^error" | head -5 | while read -r line; do
                info "$line"
            done
        fi
    else
        ok "Max build compiles"
    fi
fi

# ── 3. Cross-platform checks ─────────────────────

if ! $QUICK; then
    header "Cross-Platform (cargo check --target)"

    # Windows (most likely to break due to unix socket guards)
    info "cargo check --target x86_64-pc-windows-msvc (free)..."
    # Ensure target is installed
    rustup target add x86_64-pc-windows-msvc 2>/dev/null || true
    if cargo check --target x86_64-pc-windows-msvc 2>&1 | grep -q "^error\["; then
        fail "Windows cross-check: compilation errors"
        cargo check --target x86_64-pc-windows-msvc 2>&1 | grep "^error" | head -5 | while read -r line; do
            info "$line"
        done
    else
        ok "Windows (free) cross-check passes"
    fi

    # Windows Pro
    info "cargo check --target x86_64-pc-windows-msvc --features pro..."
    if cargo check --target x86_64-pc-windows-msvc --features pro 2>&1 | grep -q "^error\["; then
        fail "Windows Pro cross-check: compilation errors"
        cargo check --target x86_64-pc-windows-msvc --features pro 2>&1 | grep "^error" | head -5 | while read -r line; do
            info "$line"
        done
    else
        ok "Windows (pro) cross-check passes"
    fi

    # macOS x86_64 (if on ARM mac)
    ARCH=$(uname -m)
    if [ "$ARCH" = "arm64" ]; then
        rustup target add x86_64-apple-darwin 2>/dev/null || true
        info "cargo check --target x86_64-apple-darwin..."
        if cargo check --target x86_64-apple-darwin 2>&1 | grep -q "^error\["; then
            fail "macOS x86_64 cross-check: compilation errors"
        else
            ok "macOS x86_64 cross-check passes"
        fi
    fi

    # Linux cross-check skipped — requires system headers (openssl-sys etc.)
    info "Linux cross-check: skipped (requires native headers, verified in CI)"
fi

# ── 4. Tests ──────────────────────────────────────

if ! $QUICK; then
    header "Tests"

    info "cargo test (free)..."
    TEST_OUTPUT=$(cargo test 2>&1)
    if echo "$TEST_OUTPUT" | grep -q "test result:.*FAILED"; then
        FAILED_COUNT=$(echo "$TEST_OUTPUT" | grep "test result:" | sed 's/.*\([0-9]\+\) failed.*/\1/')
        fail "Tests: ${FAILED_COUNT} test(s) failed"
        echo "$TEST_OUTPUT" | grep "^test .* FAILED" | head -5 | while read -r line; do
            info "$line"
        done
    elif echo "$TEST_OUTPUT" | grep -q "test result:"; then
        PASSED=$(echo "$TEST_OUTPUT" | grep "test result:" | sed 's/.*ok\. \([0-9]\+\) passed.*/\1/' | head -1)
        ok "Tests: ${PASSED} passed"
    else
        warn "No tests found"
    fi

    info "cargo test --features pro..."
    TEST_OUTPUT=$(cargo test --features pro 2>&1)
    if echo "$TEST_OUTPUT" | grep -q "test result:.*FAILED"; then
        FAILED_COUNT=$(echo "$TEST_OUTPUT" | grep "test result:" | sed 's/.*\([0-9]\+\) failed.*/\1/')
        fail "Pro tests: ${FAILED_COUNT} test(s) failed"
    elif echo "$TEST_OUTPUT" | grep -q "test result:"; then
        PASSED=$(echo "$TEST_OUTPUT" | grep "test result:" | sed 's/.*ok\. \([0-9]\+\) passed.*/\1/' | head -1)
        ok "Pro tests: ${PASSED} passed"
    fi
fi

# ── 5. Warnings audit ────────────────────────────

if ! $QUICK; then
    header "Warnings Audit"

    WARN_COUNT=$(cargo check 2>&1 | grep -c "^warning\[" || true)
    if [ "$WARN_COUNT" -gt 0 ]; then
        warn "${WARN_COUNT} compiler warning(s) (cargo check)"
        cargo check 2>&1 | grep "^warning\[" | sort | uniq -c | sort -rn | head -5 | while read -r line; do
            info "$line"
        done
    else
        ok "No compiler warnings"
    fi
fi

# ── 6. Binary size check ─────────────────────────

if ! $QUICK; then
    header "Binary Size (debug)"

    if [ -f "$ROOT/target/debug/agent-hand" ]; then
        SIZE=$(du -sh "$ROOT/target/debug/agent-hand" | cut -f1)
        info "agent-hand (debug): ${SIZE}"
    fi
    if [ -f "$ROOT/target/debug/agent-hand-bridge" ]; then
        SIZE=$(du -sh "$ROOT/target/debug/agent-hand-bridge" | cut -f1)
        info "agent-hand-bridge (debug): ${SIZE}"
    fi
fi

# ── Summary ───────────────────────────────────────

echo ""
echo -e "${BLUE}─── Summary ───${NC}"
echo ""

if [ $FAIL -gt 0 ]; then
    echo -e "${RED}  PREFLIGHT FAILED${NC}"
    echo -e "${RED}  ${FAIL} error(s), ${WARN} warning(s), ${PASS} passed${NC}"
    echo -e "\n${RED}  Errors:${ERRORS}${NC}"
    echo ""
    echo -e "  ${DIM}Fix errors before pushing to avoid CI failures.${NC}"
    echo ""
    exit 1
else
    echo -e "${GREEN}  PREFLIGHT PASSED${NC}"
    echo -e "  ${PASS} passed, ${WARN} warning(s)"
    echo ""
    if [ $WARN -gt 0 ]; then
        echo -e "  ${DIM}Warnings are non-blocking but worth reviewing.${NC}"
    fi
    echo -e "  ${DIM}Safe to push / release.${NC}"
    echo ""
    exit 0
fi
