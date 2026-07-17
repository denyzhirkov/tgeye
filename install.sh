#!/bin/sh
# tgeye installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/denyzhirkov/tgeye/main/install.sh | sh
#   wget -qO- https://raw.githubusercontent.com/denyzhirkov/tgeye/main/install.sh | sh
set -eu

REPO="denyzhirkov/tgeye"
VERSION="${TGEYE_VERSION:-latest}"
INSTALL_DIR="${TGEYE_DIR:-$HOME/.local/bin}"

# HTTP client: prefer curl, fallback to wget
fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$1"
  else
    echo "Error: curl or wget is required. Install one and retry."
    exit 1
  fi
}

download() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$1" -O "$2"
  fi
}

# Detect platform
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
RAW_ARCH="$(uname -m)"
case "$RAW_ARCH" in
  x86_64|amd64)    ARCH="x86_64" ;;
  arm64|aarch64)   ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $RAW_ARCH"; exit 1 ;;
esac

case "$OS" in
  darwin|linux) ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

PLATFORM="${OS}-${ARCH}"

# Resolve version
if [ "$VERSION" = "latest" ]; then
  VERSION=$(fetch "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
  if [ -z "$VERSION" ]; then
    echo "Failed to detect latest version. Set TGEYE_VERSION=x.y.z manually."
    exit 1
  fi
fi

TAG="v${VERSION}"
BINARY="tgeye-${PLATFORM}"
URL="https://github.com/${REPO}/releases/download/${TAG}/${BINARY}"

echo ""
echo "  tgeye installer"
echo "  ---------------"
echo "  Version:  ${VERSION}"
echo "  Platform: ${PLATFORM}"
echo ""

# Download to temp file
TMPFILE="$(mktemp)"
trap 'rm -f "$TMPFILE"' EXIT

echo "  Downloading..."
if ! download "$URL" "$TMPFILE"; then
  echo "  Binary not found at ${URL}"
  echo "  Check available releases: https://github.com/${REPO}/releases"
  exit 1
fi

chmod +x "$TMPFILE"
mkdir -p "$INSTALL_DIR"
mv "$TMPFILE" "$INSTALL_DIR/tgeye"
trap - EXIT

echo "  -> $INSTALL_DIR/tgeye"

# Remove quarantine on macOS
if [ "$OS" = "darwin" ]; then
  xattr -cr "$INSTALL_DIR/tgeye" 2>/dev/null || true
fi

# Symlink to /usr/local/bin for MCP server compatibility
GLOBAL_BIN="/usr/local/bin"
if [ -d "$GLOBAL_BIN" ]; then
  if [ -w "$GLOBAL_BIN" ]; then
    ln -sf "$INSTALL_DIR/tgeye" "$GLOBAL_BIN/tgeye"
    echo "  -> $GLOBAL_BIN/tgeye (symlink)"
  elif command -v sudo >/dev/null 2>&1; then
    sudo ln -sf "$INSTALL_DIR/tgeye" "$GLOBAL_BIN/tgeye" 2>/dev/null && \
      echo "  -> $GLOBAL_BIN/tgeye (symlink)" || true
  fi
fi

# Check PATH
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
  echo ""
  echo "  Add to your shell profile:"
  echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo ""
echo "  Done! Next steps:"
echo "    cd <your-project>"
echo "    tgeye init                      # creates ./.tgeye, asks for the bot token"
echo "    tgeye run                       # start collecting (Ctrl-C to stop)"
echo "    tgeye chats list                # find your chat id"
echo "    tgeye chats allow <chat-id>     # allow storing its content"
echo "    claude mcp add tgeye -- tgeye run-mcp"
echo ""
