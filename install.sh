#!/bin/sh
set -e

REPO="pholgy/fiq"
INSTALL_DIR="${FIQ_INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) os="macos" ;;
  Linux)  os="linux" ;;
  *)
    echo "Error: unsupported OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  arm64|aarch64) arch="aarch64" ;;
  x86_64|amd64)  arch="x86_64" ;;
  *)
    echo "Error: unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

# Linux only has x86_64 builds
if [ "$os" = "linux" ] && [ "$arch" != "x86_64" ]; then
  echo "Error: Linux builds are only available for x86_64" >&2
  exit 1
fi

ASSET="fiq-${os}-${arch}.tar.gz"

# Get latest release tag
if command -v curl >/dev/null 2>&1; then
  fetch="curl -fsSL"
  fetch_redirect="curl -fsSL -o"
elif command -v wget >/dev/null 2>&1; then
  fetch="wget -qO-"
  fetch_redirect="wget -qO"
else
  echo "Error: curl or wget is required" >&2
  exit 1
fi

echo "Fetching latest release..."
TAG=$($fetch "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')

if [ -z "$TAG" ]; then
  echo "Error: could not determine latest release" >&2
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"

echo "Downloading fiq ${TAG} (${os}/${arch})..."
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

if [ "$fetch" = "curl -fsSL" ]; then
  curl -fsSL -o "$TMPDIR/$ASSET" "$URL"
else
  wget -qO "$TMPDIR/$ASSET" "$URL"
fi

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
tar xzf "$TMPDIR/$ASSET" -C "$TMPDIR"
mv "$TMPDIR/fiq" "$INSTALL_DIR/fiq"
chmod +x "$INSTALL_DIR/fiq"

# Check if install dir is in PATH
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "Add to your PATH by adding this to your shell profile:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo ""
echo "fiq ${TAG} installed to ${INSTALL_DIR}/fiq"
