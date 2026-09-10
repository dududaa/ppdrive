#!/usr/bin/env bash
set -euo pipefail

# === CONFIGURATION ===
REPO="dududaa/ppdrive"
INSTALL_DIR="$HOME/.local/share/ppdrive"
BIN_DIR="$HOME/.local/bin"

# === DETECT PLATFORM ===
detect_asset() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) ASSET_PATTERN="ppdrive-linux.tar.gz" ;;
        arm64|aarch64) ASSET_PATTERN="ppdrive-linux-arm64.tar.gz" ;;
        *)
          echo "❌ Unsupported architecture: $arch"
          echo "   Supported: x86_64, arm64"
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64|amd64) ASSET_PATTERN="ppdrive-macos.tar.gz" ;;
        arm64|aarch64) ASSET_PATTERN="ppdrive-macos-arm64.tar.gz" ;;
        *)
          echo "❌ Unsupported architecture: $arch"
          echo "   Supported: x86_64, arm64"
          exit 1
          ;;
      esac
      ;;
    *)
      echo "❌ Unsupported OS: $os"
      echo "   Supported: Linux, macOS"
      exit 1
      ;;
  esac
}

# === FUNCTIONS ===

get_installed_version() {
  if [[ -f "$INSTALL_DIR/ppdrive" ]]; then
    "$INSTALL_DIR/ppdrive" --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo ""
  else
    echo ""
  fi
}

get_latest_version() {
  local api_url="https://api.github.com/repos/${REPO}/releases/latest"
  curl -sL "$api_url" | grep '"tag_name"' | sed -E 's/.*"v?([0-9]+\.[0-9]+\.[0-9]+).*/\1/'
}

download_latest_release() {
  echo "📦 Fetching latest release info..."
  local api_url="https://api.github.com/repos/${REPO}/releases/latest"
  local download_url
  download_url=$(curl -sL "$api_url" | grep "browser_download_url" | grep "$ASSET_PATTERN" | cut -d '"' -f 4)

  if [[ -z "$download_url" ]]; then
    echo "❌ Could not find a release asset matching pattern '$ASSET_PATTERN'."
    echo "   This may be a network issue or the release may not include this platform."
    exit 1
  fi

  echo "⬇️  Downloading: $download_url"
  mkdir -p /tmp/ppdrive-install
  curl -L -o "/tmp/ppdrive-install/$ASSET_PATTERN" "$download_url"
}

extract_and_install() {
  echo "📂 Installing to: $INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"
  tar -xzf "/tmp/ppdrive-install/$ASSET_PATTERN" -C "$INSTALL_DIR"

  echo "🔧 Making executables runnable..."
  chmod +x "$INSTALL_DIR/ppdrive" "$INSTALL_DIR/server"

  echo "🔗 Linking to $BIN_DIR..."
  mkdir -p "$BIN_DIR"
  ln -sf "$INSTALL_DIR/ppdrive" "$BIN_DIR/ppdrive"
  ln -sf "$INSTALL_DIR/server" "$BIN_DIR/server"
}

ensure_bin_in_path() {
  if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "⚠️  $BIN_DIR is not in your PATH. Attempting to fix..."
    local shell_name
    shell_name=$(basename "${SHELL:-/bin/bash}")

    case "$shell_name" in
      bash)
        local config="$HOME/.bashrc"
        if ! grep -q '$HOME/.local/bin' "$config" 2>/dev/null; then
          echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$config"
          echo "✅ Added ~/.local/bin to PATH in $config"
        else
          echo "✅ ~/.local/bin already in $config"
        fi
        ;;
      zsh)
        local config="$HOME/.zshrc"
        if ! grep -q '$HOME/.local/bin' "$config" 2>/dev/null; then
          echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$config"
          echo "✅ Added ~/.local/bin to PATH in $config"
        else
          echo "✅ ~/.local/bin already in $config"
        fi
        ;;
      fish)
        fish -c 'set -U fish_user_paths ~/.local/bin $fish_user_paths'
        echo "✅ Added ~/.local/bin to PATH for fish shell"
        ;;
      *)
        echo "⚠️ Unknown shell ($shell_name). Please manually add this line to your shell config:"
        echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
        ;;
    esac

    echo "👉 Restart your terminal or run 'source ~/.bashrc' (or equivalent) to apply changes."
  fi
}

cleanup() {
  rm -rf /tmp/ppdrive-install
}

# === MAIN ===
trap cleanup EXIT

detect_asset

INSTALLED_VERSION=$(get_installed_version)
LATEST_VERSION=$(get_latest_version)

if [[ -n "$INSTALLED_VERSION" && -n "$LATEST_VERSION" ]]; then
  if [[ "$INSTALLED_VERSION" == "$LATEST_VERSION" ]]; then
    echo "✅ ppdrive is already up to date ($INSTALLED_VERSION)."
    exit 0
  fi
  echo "⬆️  Upgrading ppdrive: $INSTALLED_VERSION → $LATEST_VERSION"
fi

download_latest_release
extract_and_install
ensure_bin_in_path

echo "✅ PPDRIVE installation complete!"
echo "You can now run:"
echo "   ppdrive --help"
