#!/usr/bin/env sh
set -eu

REPO="prodbyola/ppdrive"
BINARY_NAME="ppdrive"
INSTALL_DIR=$HOME/.local/ppdrive
LINK_DIR="$HOME/.local/bin"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_FILE="$SERVICE_DIR/ppdrive.service"

rm $INSTALL_DIR/$BINARY_NAME
rm -rf $INSTALL_DIR/migrations

# Detect OS and set the correct asset name
OS=$(uname -s)
case "$OS" in
  Linux*)   ASSET_NAME="ppdrive-linux" ;;
  Darwin*)  ASSET_NAME="ppdrive-macos" ;;
  *)        echo "Unsupported OS: $OS"; exit 1 ;;
esac

# Create Install DIR
mkdir -p "$INSTALL_DIR"

# Fetch the latest release tag from GitHub API
TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | cut -d '"' -f4)

# Download the binary
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME.tar.gz"

echo "📥 Downloading $ASSET_NAME (version $TAG)..."
curl -L --fail "$DOWNLOAD_URL" | tar -xz -C "$INSTALL_DIR" || {
  echo "❌ Failed to download the binary."
  exit 1
}

chmod +x "$INSTALL_DIR/$BINARY_NAME"

# Optionally create symlink to a folder in PATH
mkdir -p "$LINK_DIR"
ln -sf "$INSTALL_DIR/$BINARY_NAME" "$LINK_DIR/$BINARY_NAME"

# Setup systemd user service
mkdir -p "$SERVICE_DIR"
cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=PPDrive Service
After=network.target

[Service]
ExecStart=$INSTALL_DIR/$BINARY_NAME
Restart=on-failure
WorkingDirectory=$INSTALL_DIR

[Install]
WantedBy=default.target
EOF

# Reload systemd user manager and enable service
systemctl --user daemon-reexec || true
systemctl --user daemon-reload
systemctl --user enable ppdrive.service
systemctl --user start ppdrive.service

# export variable(s)
export PATH=$PATH:$INSTALL_DIR
export PATH=$PATH:$LINK_DIR

echo "✅ Installed to $INSTALL_DIR"
echo "🔗 Symlinked to $LINK_DIR/$BINARY_NAME"
