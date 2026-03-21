#!/bin/bash
# browsync installer for macOS and Linux
# Usage: curl -fsSL https://ishaileshpant.github.io/browsync/install.sh | bash
set -euo pipefail

REPO="ishaileshpant/browsync"
INSTALL_DIR="/usr/local/bin"
BOLD="\033[1m"
CYAN="\033[36m"
GREEN="\033[32m"
RED="\033[31m"
RESET="\033[0m"

info() { echo -e "${CYAN}[browsync]${RESET} $1"; }
success() { echo -e "${GREEN}[browsync]${RESET} $1"; }
error() { echo -e "${RED}[browsync]${RESET} $1" >&2; }

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64) TARGET="aarch64-apple-darwin" ;;
      x86_64) TARGET="x86_64-apple-darwin" ;;
      *) error "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64|amd64) TARGET="x86_64-unknown-linux-gnu" ;;
      aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
      *) error "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  *) error "Unsupported OS: $OS"; exit 1 ;;
esac

info "Detected: ${BOLD}$OS $ARCH${RESET} ($TARGET)"

# Get latest release
info "Fetching latest release..."
RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"
DOWNLOAD_URL=$(curl -fsSL "$RELEASE_URL" | grep "browser_download_url.*$TARGET" | head -1 | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
  info "No prebuilt binary found. Building from source..."

  # Check for Rust
  if ! command -v cargo &>/dev/null; then
    info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
  fi

  info "Building browsync from source..."
  cargo install --git "https://github.com/$REPO" browsync-cli
  cargo install --git "https://github.com/$REPO" browsync-daemon

  success "Installed browsync and browsyncd via cargo"
  echo ""
  info "Run ${BOLD}browsync detect${RESET} to get started"
  exit 0
fi

# Download binary
TMPDIR=$(mktemp -d)
TARBALL="$TMPDIR/browsync.tar.gz"
info "Downloading $DOWNLOAD_URL..."
curl -fsSL -o "$TARBALL" "$DOWNLOAD_URL"

# Extract
info "Extracting..."
tar -xzf "$TARBALL" -C "$TMPDIR"

# Install binaries
info "Installing to $INSTALL_DIR (may require sudo)..."
if [ -w "$INSTALL_DIR" ]; then
  cp "$TMPDIR/browsync" "$INSTALL_DIR/browsync"
  cp "$TMPDIR/browsyncd" "$INSTALL_DIR/browsyncd" 2>/dev/null || true
  chmod +x "$INSTALL_DIR/browsync" "$INSTALL_DIR/browsyncd" 2>/dev/null || true
else
  sudo cp "$TMPDIR/browsync" "$INSTALL_DIR/browsync"
  sudo cp "$TMPDIR/browsyncd" "$INSTALL_DIR/browsyncd" 2>/dev/null || true
  sudo chmod +x "$INSTALL_DIR/browsync" "$INSTALL_DIR/browsyncd" 2>/dev/null || true
fi

# Cleanup
rm -rf "$TMPDIR"

# Create data directory
mkdir -p "$HOME/.browsync"

success "browsync installed successfully!"
echo ""
info "Version: $(browsync --version 2>/dev/null || echo 'unknown')"
info "Location: $(which browsync)"
info "Data dir: ~/.browsync/"
echo ""
info "Get started:"
echo "  browsync detect     # Find installed browsers"
echo "  browsync import     # Import bookmarks & history"
echo "  browsync search     # Search across everything"
echo "  browsync tui        # Launch terminal UI"
