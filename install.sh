#!/usr/bin/env bash
set -e

# === CONFIGURATION ===
REPO="dududaa/ppdrive"
INSTALL_DIR="$HOME/.local/share/ppdrive"
BIN_DIR="$HOME/.local/bin"
ASSET_PATTERN="ppdrive-linux.tar.gz"

# === FUNCTIONS ===

download_latest_release() {
  echo "📦 Fetching latest release info..."
  API_URL="https://api.github.com/repos/${REPO}/releases/264720386"
  DOWNLOAD_URL=$(curl -sL "$API_URL" | grep "browser_download_url" | grep "$ASSET_PATTERN" | cut -d '"' -f 4)

  if [[ -z "$DOWNLOAD_URL" ]]; then
    echo "❌ Could not find a release asset matching pattern '$ASSET_PATTERN'."
    exit 1
  fi

  echo "⬇️  Downloading: $DOWNLOAD_URL"
  mkdir -p /tmp/ppdrive-install
  cd /tmp/ppdrive-install
  curl -L -o "$ASSET_PATTERN" "$DOWNLOAD_URL"
}

extract_and_install() {
  echo "📂 Installing to: $INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"
  tar -xzf "$ASSET_PATTERN" -C "$INSTALL_DIR"

  echo "🔧 Making executables runnable..."
  chmod +x "$INSTALL_DIR"/ppdrive "$INSTALL_DIR"/manager || true

  echo "🔗 Linking to $BIN_DIR..."
  mkdir -p "$BIN_DIR"
  ln -sf "$INSTALL_DIR/ppdrive" "$BIN_DIR/ppdrive"
  ln -sf "$INSTALL_DIR/manager" "$BIN_DIR/manager"
}

ensure_bin_in_path() {
  # If ~/.local/bin isn't in PATH, try to fix it
  if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "⚠️  $BIN_DIR is not in your PATH. Attempting to fix..."
    SHELL_NAME=$(basename "$SHELL")

    case "$SHELL_NAME" in
      bash)
        CONFIG_FILE="$HOME/.bashrc"
        echo "export PATH=\"\$HOME/.local/bin:\$PATH\"" >> "$CONFIG_FILE"
        echo "✅ Added ~/.local/bin to PATH in $CONFIG_FILE"
        ;;
      zsh)
        CONFIG_FILE="$HOME/.zshrc"
        echo "export PATH=\"\$HOME/.local/bin:\$PATH\"" >> "$CONFIG_FILE"
        echo "✅ Added ~/.local/bin to PATH in $CONFIG_FILE"
        ;;
      fish)
        fish -c 'set -U fish_user_paths ~/.local/bin $fish_user_paths'
        echo "✅ Added ~/.local/bin to PATH for fish shell"
        ;;
      *)
        echo "⚠️ Unknown shell ($SHELL_NAME). Please manually add this line to your shell config:"
        echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
        ;;
    esac

    echo "👉 Restart your terminal or run 'source ~/.bashrc' (or equivalent) to apply changes."
  fi
}

cleanup() {
  echo "🧹 Cleaning up temporary files..."
  rm -rf /tmp/ppdrive-install
}

# === MAIN ===
download_latest_release
extract_and_install
ensure_bin_in_path
cleanup

echo "✅ PPDRIVE installation complete!"
echo "You can now run:"
echo "   ppdrive --help"
