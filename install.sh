#!/bin/bash

# Download binary
REPO="prodbyola/ppdrive"
BINARY_NAME="ppdrive"
INSTALL_DIR="/opt/ppdrive"
LINK_DIR="$HOME/.local/bin"

# Detect OS and set the correct asset name
OS=$(uname -s)
case "$OS" in
  Linux*)   ASSET_NAME="ppdrive-linux" ;;
  Darwin*)  ASSET_NAME="ppdrive-macos" ;;
  *)        echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Fetch the latest release tag from GitHub API
TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | cut -d '"' -f4)

# Compose download URL
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME"

# Download the binary
echo "📥 Downloading $ASSET_NAME (version $TAG)..."
mkdir -p "$INSTALL_DIR"
curl -L --fail "$DOWNLOAD_URL" -o "$INSTALL_DIR/$BINARY_NAME" || {
  echo "❌ Failed to download the binary."
  exit 1
}

chmod +x "$INSTALL_DIR/$BINARY_NAME"

# Optionally create symlink to a folder in PATH
mkdir -p "$LINK_DIR"
ln -sf "$INSTALL_DIR/$BINARY_NAME" "$LINK_DIR/$BINARY_NAME"

# export variable(s)
export PATH=$PATH:$LINK_DIR

echo "✅ Installed to $INSTALL_DIR"
echo "🔗 Symlinked to $LINK_DIR/$APP_NAME"
echo "check with ppdrive -v"
