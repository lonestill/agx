#!/usr/bin/env bash
set -e

REPO="lonestill/agx"
INSTALL_DIR="/usr/local/bin"

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${CYAN}=== Installing agx (Antigravity Multitool) ===${NC}"

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64) TARGET="macos-arm64" ;;
      x86_64) TARGET="macos-x86_64" ;;
      *) echo -e "${RED}Unsupported macOS architecture: $ARCH${NC}"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64) TARGET="linux-x86_64" ;;
      *) echo -e "${RED}Unsupported Linux architecture: $ARCH${NC}"; exit 1 ;;
    esac
    ;;
  *)
    echo -e "${RED}Unsupported operating system: $OS. On Windows, download the zip release directly.${NC}"
    exit 1
    ;;
esac

# Find latest release tag
LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$LATEST_TAG" ]; then
  LATEST_TAG="v1.3.3"
fi

TARBALL="agx-${LATEST_TAG}-${TARGET}.tar.gz"
URL="https://github.com/$REPO/releases/download/${LATEST_TAG}/${TARBALL}"

echo -e "Downloading ${CYAN}${TARBALL}${NC}..."
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fsSL "$URL" -o "$TMP_DIR/$TARBALL"
tar -xzf "$TMP_DIR/$TARBALL" -C "$TMP_DIR"

# Install binary
if [ ! -w "$INSTALL_DIR" ]; then
  if command -v sudo >/dev/null 2>&1; then
    echo -e "Installing to $INSTALL_DIR (requires sudo)..."
    sudo cp "$TMP_DIR/agx" "$INSTALL_DIR/agx"
    sudo chmod +x "$INSTALL_DIR/agx"
  else
    ALT_DIR="$HOME/.local/bin"
    mkdir -p "$ALT_DIR"
    cp "$TMP_DIR/agx" "$ALT_DIR/agx"
    chmod +x "$ALT_DIR/agx"
    INSTALL_DIR="$ALT_DIR"
  fi
else
  cp "$TMP_DIR/agx" "$INSTALL_DIR/agx"
  chmod +x "$INSTALL_DIR/agx"
fi

echo -e "${GREEN}[OK] agx ${LATEST_TAG} installed successfully to ${INSTALL_DIR}/agx${NC}"
echo -e "Run ${CYAN}agx${NC} to launch the interactive TUI or ${CYAN}agx --help${NC} for CLI options."
