#!/bin/bash

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

# Create Install DIR
echo "Creating install dir..."
sudo mkdir -p "$INSTALL_DIR"

# Fetch the latest release tag from GitHub API
TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | cut -d '"' -f4)

# Download the binary
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME"

echo "📥 Downloading $ASSET_NAME (version $TAG)..."
curl -L --fail "$DOWNLOAD_URL" -o "$INSTALL_DIR/$BINARY_NAME" || {
  echo "❌ Failed to download the binary."
  exit 1
}

# Download default config
CONFIG_FILENAME=ppd_config.toml
CONFIG_SRC=https://raw.githubusercontent.com/prodbyola/ppdrive/refs/heads/main/$CONFIG_FILENAME

echo "📥 Downloading default config..."
curl -L --fail "$CONFIG_SRC" -o "$INSTALL_DIR/$CONFIG_FILENAME" || {
  echo "❌ Failed to download default config."
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
ppdrive configure
