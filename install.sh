#!/usr/bin/env bash
set -euo pipefail

REPO="weykon/agent-hand"
BIN_NAME="agent-hand"

usage() {
  cat <<EOF
Usage: install.sh [--prefix DIR] [--version vX.Y.Z]

Installs ${BIN_NAME} from GitHub Releases.

Options:
  --prefix DIR   Install directory (default: /usr/local/bin if writable, else ~/.local/bin)
  --version TAG  Install a specific tag (default: latest)
EOF
}

PREFIX=""
VERSION="latest"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$2"; shift 2 ;;
    --version)
      VERSION="$2"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "Unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os="apple-darwin" ;;
  Linux)  os="unknown-linux-gnu" ;;
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
asset="${BIN_NAME}-${target}.tar.gz"

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

if [[ ! -f "${tmpdir}/${BIN_NAME}" ]]; then
  echo "Malformed archive: ${asset} (missing ${BIN_NAME})" >&2
  exit 1
fi

install -m 0755 "${tmpdir}/${BIN_NAME}" "${PREFIX}/${BIN_NAME}"

echo "Installed ${BIN_NAME} to ${PREFIX}/${BIN_NAME}" >&2
echo "Run: ${BIN_NAME} --help" >&2
