#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────
# agent-hand release script
#
# Usage:
#   ./scripts/release.sh 0.3.6
#   ./scripts/release.sh patch    (auto-bump patch)
#   ./scripts/release.sh minor    (auto-bump minor)
# ─────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

fail()  { echo -e "${RED}✗ $1${NC}" >&2; exit 1; }
ok()    { echo -e "${GREEN}✓ $1${NC}"; }
warn()  { echo -e "${YELLOW}! $1${NC}"; }
info()  { echo -e "  $1"; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# ── Parse version argument ──────────────────────

CURRENT=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "${1:-}" in
    patch)  NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))" ;;
    minor)  NEW_VERSION="$MAJOR.$((MINOR + 1)).0" ;;
    major)  NEW_VERSION="$((MAJOR + 1)).0.0" ;;
    "")     fail "Usage: $0 <version|patch|minor|major>" ;;
    *)      NEW_VERSION="$1" ;;
esac

echo ""
echo "  Release: v${CURRENT} → v${NEW_VERSION}"
echo ""

# ── Step 1: Check pro/ repo ─────────────────────

echo "─── Checking pro/ repository ───"

if [ ! -d "$ROOT/pro/.git" ]; then
    fail "pro/ directory is not a git repo"
fi

cd "$ROOT/pro"

# Check for uncommitted changes
if [ -n "$(git status --porcelain -- src/)" ]; then
    echo ""
    git status --short -- src/
    echo ""
    fail "pro/src/ has uncommitted changes. Commit and push first."
fi

# Check for unpushed commits
LOCAL_HEAD=$(git rev-parse HEAD)
REMOTE_HEAD=$(git rev-parse origin/main 2>/dev/null || echo "unknown")

if [ "$LOCAL_HEAD" != "$REMOTE_HEAD" ]; then
    UNPUSHED=$(git log origin/main..HEAD --oneline 2>/dev/null | wc -l | tr -d ' ')
    warn "pro/ has ${UNPUSHED} unpushed commit(s):"
    git log origin/main..HEAD --oneline 2>/dev/null
    echo ""
    read -p "  Push pro/ to origin/main now? [Y/n] " -n 1 -r REPLY
    echo ""
    if [[ "$REPLY" =~ ^[Nn]$ ]]; then
        fail "Aborted. Push pro/ manually first."
    fi
    git push origin main
    ok "pro/ pushed to origin/main"
else
    ok "pro/ is clean and in sync with remote"
fi

cd "$ROOT"

# ── Step 2: Check main repo ─────────────────────

echo ""
echo "─── Checking main repository ───"

# Check for uncommitted changes in src/
if [ -n "$(git status --porcelain -- src/ Cargo.toml)" ]; then
    echo ""
    git status --short -- src/ Cargo.toml
    echo ""
    read -p "  Commit these changes before release? [Y/n] " -n 1 -r REPLY
    echo ""
    if [[ "$REPLY" =~ ^[Nn]$ ]]; then
        fail "Aborted. Commit changes first."
    fi
    git add src/ Cargo.toml
    read -p "  Commit message: " MSG
    git commit -m "$MSG"
    ok "Changes committed"
else
    ok "Working tree is clean"
fi

# ── Step 3: Compile check ───────────────────────

echo ""
echo "─── Build verification ───"

info "Checking free build..."
cargo check 2>&1 | grep -E "^error" && fail "Free build failed" || ok "Free build OK"

info "Checking pro build..."
cargo check --features pro 2>&1 | grep -E "^error" && fail "Pro build failed" || ok "Pro build OK"

# ── Step 4: Version bump + tag ──────────────────

echo ""
echo "─── Creating release v${NEW_VERSION} ───"

sed -i '' "s/^version = \"${CURRENT}\"/version = \"${NEW_VERSION}\"/" Cargo.toml
cargo check 2>/dev/null  # regenerate Cargo.lock

git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to ${NEW_VERSION}"
ok "Version bumped to ${NEW_VERSION}"

git tag "v${NEW_VERSION}"
ok "Tag v${NEW_VERSION} created"

# ── Step 5: Push ────────────────────────────────

echo ""
read -p "  Push to origin and trigger CI release? [Y/n] " -n 1 -r REPLY
echo ""
if [[ "$REPLY" =~ ^[Nn]$ ]]; then
    warn "Tag created locally. Push manually with:"
    info "  git push origin master && git push origin v${NEW_VERSION}"
    exit 0
fi

git push origin master
git push origin "v${NEW_VERSION}"
ok "Pushed to origin — CI release triggered"

echo ""
echo -e "${GREEN}  Release v${NEW_VERSION} is on its way!${NC}"
echo "  Track CI: gh run watch --repo weykon/agent-hand"
echo ""
