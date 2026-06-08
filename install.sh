#!/bin/sh
set -e

# 1. Configuration
REPO="dududaa/ppdrive"
TAG="v1.0.0-alpha"

# 2. Detect OS and Architecture
OS_TYPE=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH_TYPE=$(uname -m)

if [ "$OS_TYPE" = "linux" ]; then
    ARTIFACT="linux-x86_64"
elif [ "$OS_TYPE" = "darwin" ]; then
    if [ "$ARCH_TYPE" = "arm64" ]; then
        ARTIFACT="macos-arm64"
    else
        ARTIFACT="macos-x86_64"
    fi
else
    echo "Unsupported OS for this shell script."
    exit 1
fi

FILE_NAME="release-${ARTIFACT}.tar.gz"
URL="https://github.com{REPO}/releases/download/${TAG}/${FILE_NAME}"

# 3. Establish System Install Path
INSTALL_DIR="/usr/local/bin/ppdrive"
echo "Installing to ${INSTALL_DIR}..."
sudo mkdir -p "$INSTALL_DIR"

# 4. Download and Extract
TEMP_DIR=$(mktemp -d)
echo "Downloading ${URL}..."
curl -sL "$URL" -o "${TEMP_DIR}/${FILE_NAME}"

echo "Extracting artifacts..."
tar -xzf "${TEMP_DIR}/${FILE_NAME}" -C "$TEMP_DIR"

# Move individual files up cleanly if nested
sudo cp -r "${TEMP_DIR}/release-${ARTIFACT}/"* "$INSTALL_DIR/"
sudo chmod +x "$INSTALL_DIR/ppdrive" "$INSTALL_DIR/server"

# 5. Add to Permanent Environment Path (With Fish Support)
# Detect the current user's default shell or active shell session
CURRENT_SHELL=$(basename "$SHELL")

if [ "$CURRENT_SHELL" = "fish" ] || [ -n "$FISH_VERSION" ]; then
    FISH_CONFIG_DIR="$HOME/.config/fish"
    PROFILE="$FISH_CONFIG_DIR/config.fish"

    # Ensure fish configuration directory exists
    mkdir -p "$FISH_CONFIG_DIR"

    # Check if the path is already added to fish config
    if ! grep -q "$INSTALL_DIR" "$PROFILE" 2>/dev/null; then
        echo "Adding $INSTALL_DIR to PATH in $PROFILE"
        echo "set -gx PATH \$PATH $INSTALL_DIR" >> "$PROFILE"
    fi
    echo "Success! Restart your terminal or run: source $PROFILE"

else
    # Fallback to Bash or Zsh configurations
    if [ -n "$ZSH_VERSION" ] || [ "$CURRENT_SHELL" = "zsh" ]; then
        PROFILE="$HOME/.zshrc"
    else
        PROFILE="$HOME/.bashrc"
    fi

    if ! grep -q "$INSTALL_DIR" "$PROFILE"; then
        echo "Adding $INSTALL_DIR to PATH in $PROFILE"
        echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$PROFILE"
    fi
    echo "Success! Restart your terminal or run: source $PROFILE"
fi

# Clean up temporary files
rm -rf "$TEMP_DIR"
