#!/bin/bash

# collect configurations
read -p "port [default: 5000]:" port
port=${port:-5000}

while true; do
    read -p "database url: " db_url
    if [[ -n "$db_url" ]]; then
        break
    else
        echo "Please provide a valid database url."
    fi
done

read -p "allowed origins [default: *]: " allowed_origins
allowed_origins=${allowed_origins:-*}

read -p "max upload size (in mb) [default: 10]: " max_upload_size
max_upload_size=${max_upload_size=-10}

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

# export variables
export PPDRIVE_PORT=$port
export PPDRIVE_DATABASE_URL=$db_url
export PPDRIVE_ALLOWED_ORIGINS=$allowed_origins
export PPDRIVE_MAX_UPLOAD_SIZE=$max_upload_size

export PATH=$PATH:$LINK_DIR

echo "✅ Installed to $INSTALL_DIR"
echo "🔗 Symlinked to $LINK_DIR/$APP_NAME"
echo "check with ppdrive -v"
