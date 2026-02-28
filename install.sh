#!/usr/bin/env bash
set -euo pipefail

REPO="weykon/agent-hand"
BIN_NAME="agent-hand"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $*" >&2; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

usage() {
  cat <<EOF
Usage: install.sh [--prefix DIR] [--version vX.Y.Z] [--skip-tmux] [--pro]

Installs ${BIN_NAME} from GitHub Releases.

Options:
  --prefix DIR   Install directory (default: /usr/local/bin if writable, else ~/.local/bin)
  --version TAG  Install a specific tag (default: latest)
  --skip-tmux    Skip tmux installation check
  --pro          Install the Pro build (requires a valid Pro license)
EOF
}

# Check and install tmux if needed
check_tmux() {
  if command -v tmux &>/dev/null; then
    local tmux_version
    tmux_version=$(tmux -V 2>/dev/null || echo "unknown")
    info "tmux is installed: $tmux_version"
    return 0
  fi

  warn "tmux is not installed. ${BIN_NAME} requires tmux to function."

  # Detect OS and package manager
  local os
  os="$(uname -s)"

  case "$os" in
    Darwin)
      if command -v brew &>/dev/null; then
        info "Installing tmux via Homebrew..."
        brew install tmux
      else
        error "Homebrew not found. Please install tmux manually:"
        echo "  brew install tmux" >&2
        echo "  or visit: https://github.com/tmux/tmux/wiki/Installing" >&2
        return 1
      fi
      ;;
    Linux)
      if command -v apt-get &>/dev/null; then
        info "Installing tmux via apt..."
        sudo apt-get update && sudo apt-get install -y tmux
      elif command -v dnf &>/dev/null; then
        info "Installing tmux via dnf..."
        sudo dnf install -y tmux
      elif command -v yum &>/dev/null; then
        info "Installing tmux via yum..."
        sudo yum install -y tmux
      elif command -v pacman &>/dev/null; then
        info "Installing tmux via pacman..."
        sudo pacman -S --noconfirm tmux
      elif command -v apk &>/dev/null; then
        info "Installing tmux via apk..."
        sudo apk add tmux
      else
        error "Could not detect package manager. Please install tmux manually:"
        echo "  Ubuntu/Debian: sudo apt install tmux" >&2
        echo "  Fedora: sudo dnf install tmux" >&2
        echo "  Arch: sudo pacman -S tmux" >&2
        return 1
      fi
      ;;
    MINGW*|MSYS*|CYGWIN*)
      warn "Detected Windows shell environment ($os). Auto-installing tmux is not supported."
      warn "Tip: Use WSL (recommended) or MSYS2, then install tmux there."
      return 0
      ;;
    *)
      error "Unsupported OS for automatic tmux installation: $os"
      echo "Please install tmux manually: https://github.com/tmux/tmux/wiki/Installing" >&2
      return 1
      ;;
  esac

  # Verify installation
  if command -v tmux &>/dev/null; then
    info "tmux installed successfully: $(tmux -V)"
    return 0
  else
    error "tmux installation failed"
    return 1
  fi
}

PREFIX=""
VERSION="latest"
SKIP_TMUX=false
PRO_BUILD=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$2"; shift 2 ;;
    --version)
      VERSION="$2"; shift 2 ;;
    --skip-tmux)
      SKIP_TMUX=true; shift ;;
    --pro)
      PRO_BUILD=true; shift ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "Unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

# Check tmux first (unless skipped)
if [[ "$SKIP_TMUX" == "false" ]]; then
  check_tmux || exit 1
fi

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os="apple-darwin" ;;
  Linux)  os="unknown-linux-gnu" ;;
  MINGW*|MSYS*|CYGWIN*) os="pc-windows-msvc" ;;
  *)
    echo "Unsupported OS: $os" >&2
    exit 1
    ;;
esac

case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *)
    echo "Unsupported arch: $arch" >&2
    exit 1
    ;;
esac

target="${arch}-${os}"
if [[ "$PRO_BUILD" == "true" ]]; then
  asset="${BIN_NAME}-pro-${target}.tar.gz"
else
  asset="${BIN_NAME}-${target}.tar.gz"
fi

# Windows release contains an .exe inside the tarball.
out_bin="${BIN_NAME}"
if [[ "$os" == "pc-windows-msvc" ]]; then
  out_bin="${BIN_NAME}.exe"
fi

if [[ -z "$PREFIX" ]]; then
  if [[ -w "/usr/local/bin" ]]; then
    PREFIX="/usr/local/bin"
  else
    PREFIX="${HOME}/.local/bin"
  fi
fi

mkdir -p "$PREFIX"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

url_base="https://github.com/${REPO}/releases"
if [[ "$VERSION" == "latest" ]]; then
  url="${url_base}/latest/download/${asset}"
else
  url="${url_base}/download/${VERSION}/${asset}"
fi

echo "Downloading ${url}" >&2
curl -fsSL "$url" -o "${tmpdir}/${asset}"

tar -xzf "${tmpdir}/${asset}" -C "$tmpdir"

if [[ ! -f "${tmpdir}/${out_bin}" ]]; then
  echo "Malformed archive: ${asset} (missing ${out_bin})" >&2
  exit 1
fi

# Prefer install if available; fallback to cp.
if command -v install &>/dev/null; then
  if [[ "$os" == "pc-windows-msvc" ]]; then
    install "${tmpdir}/${out_bin}" "${PREFIX}/${out_bin}" || cp "${tmpdir}/${out_bin}" "${PREFIX}/${out_bin}"
  else
    install -m 0755 "${tmpdir}/${out_bin}" "${PREFIX}/${out_bin}"
  fi
else
  cp "${tmpdir}/${out_bin}" "${PREFIX}/${out_bin}"
fi

info "Installed ${out_bin} to ${PREFIX}/${out_bin}"

# Ensure ~/.local/bin is in PATH if that's where we installed
if [[ "$PREFIX" == "${HOME}/.local/bin" ]]; then
  if ! echo "$PATH" | tr ':' '\n' | grep -qx "${HOME}/.local/bin"; then
    # Detect shell config file
    shell_rc=""
    case "$(basename "${SHELL:-/bin/bash}")" in
      zsh)  shell_rc="$HOME/.zshrc" ;;
      bash)
        if [[ -f "$HOME/.bash_profile" ]]; then
          shell_rc="$HOME/.bash_profile"
        else
          shell_rc="$HOME/.bashrc"
        fi
        ;;
      fish) shell_rc="$HOME/.config/fish/config.fish" ;;
      *)    shell_rc="$HOME/.profile" ;;
    esac

    path_line='export PATH="$HOME/.local/bin:$PATH"'
    fish_line='set -gx PATH $HOME/.local/bin $PATH'

    if [[ -n "$shell_rc" ]]; then
      if [[ "$(basename "$SHELL")" == "fish" ]]; then
        if ! grep -qF '.local/bin' "$shell_rc" 2>/dev/null; then
          echo "" >> "$shell_rc"
          echo "# Added by agent-hand installer" >> "$shell_rc"
          echo "$fish_line" >> "$shell_rc"
          info "Added ~/.local/bin to PATH in $shell_rc"
        fi
      else
        if ! grep -qF '.local/bin' "$shell_rc" 2>/dev/null; then
          echo "" >> "$shell_rc"
          echo "# Added by agent-hand installer" >> "$shell_rc"
          echo "$path_line" >> "$shell_rc"
          info "Added ~/.local/bin to PATH in $shell_rc"
        fi
      fi
    fi

    warn "~/.local/bin is not in your current PATH."
    echo ""
    echo "To use agent-hand right now, either:"
    echo "  1. Open a new terminal, or"
    echo "  2. Run:  source ${shell_rc}"
    echo ""
  fi
fi

echo ""
echo "Next steps:"
echo "  1. Run: ${BIN_NAME}"
echo "  2. Press 'n' to create a new session"
echo "  3. Press '?' for help"
echo ""
echo "Tip: For session persistence across reboots, consider installing tmux-resurrect:"
echo "  https://github.com/tmux-plugins/tmux-resurrect"
