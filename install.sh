#!/bin/sh
set -e

# 1. Configuration
REPO="dududaa/ppdrive"
INSTALL_DIR="/usr/local/bin/ppdrive"

# 2. Fetch Latest Version from GitHub API (Including Alphas/Pre-releases)
echo "Checking GitHub for the latest release..."

AUTH_HEADER=""
if [ -n "$GITHUB_TOKEN" ]; then
    AUTH_HEADER="Authorization: Bearer $GITHUB_TOKEN"
fi

# Verify that the URL matches this path exactly:
API_RESPONSE=$(curl -sS -H "User-Agent: ppdrive-installer" -H "$AUTH_HEADER" "https://://github.com${REPO}/releases" || true)

# Extract the very first "tag_name" listed in the JSON array (the newest release)
LATEST_TAG=$(echo "$API_RESPONSE" | grep '"tag_name":' | head -n 1 | sed -E 's/.*"tag_name": "([^"]+)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
    echo "❌ Error: Could not retrieve the latest release tag from GitHub."
    echo "--------------------------------------------------------"
    echo "Diagnostic Data from GitHub API response:"
    if echo "$API_RESPONSE" | grep -q "rate limit"; then
        echo "Reason: Your IP address has reached GitHub's unauthenticated API rate limit."
        echo "Fix: Pass a token to bypass this constraint: export GITHUB_TOKEN=your_pat_token"
    else
        echo "$API_RESPONSE" | head -n 15
    fi
    echo "--------------------------------------------------------"
    exit 1
fi

echo "Latest remote version found (including alpha/beta): $LATEST_TAG"

# Normalize tag string for local system matching (e.g., "1.0.0-alpha")
LATEST_VERSION=$(echo "$LATEST_TAG" | sed 's/^v//')

# 3. Check Local Installation Version
if [ -f "$INSTALL_DIR/ppdrive" ]; then
    # Runs your binary to capture its version string
    LOCAL_VERSION_RAW=$("$INSTALL_DIR/ppdrive" --version 2>/dev/null || "$INSTALL_DIR/ppdrive" -V 2>/dev/null || echo "0.0.0")
    # Extracts the version number (e.g., "ppdrive 1.0.0" -> "1.0.0")
    LOCAL_VERSION=$(echo "$LOCAL_VERSION_RAW" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n1)

    echo "Current local version is: $LOCAL_VERSION"

    if [ "$LOCAL_VERSION" = "$LATEST_VERSION" ]; then
        echo "Success: ppdrive is already up to date ($LOCAL_VERSION)."
        exit 0
    fi
    echo "New version detected ($LATEST_VERSION). Proceeding with upgrade..."
fi

# 4. Detect OS and Architecture
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
URL="https://github.com{REPO}/releases/download/${LATEST_TAG}/${FILE_NAME}"

# 5. Establish/Clean System Install Path
echo "Installing to ${INSTALL_DIR}..."
sudo mkdir -p "$INSTALL_DIR"

# 6. Download and Extract
TEMP_DIR=$(mktemp -d)
echo "Downloading ${URL}..."
curl -sL "$URL" -o "${TEMP_DIR}/${FILE_NAME}"

echo "Extracting artifacts..."
tar -xzf "${TEMP_DIR}/${FILE_NAME}" -C "$TEMP_DIR"

# Move individual files up cleanly if nested
sudo cp -r "${TEMP_DIR}/release-${ARTIFACT}/"* "$INSTALL_DIR/"
sudo chmod +x "$INSTALL_DIR/ppdrive" "$INSTALL_DIR/server"

# 7. Add to Permanent Environment Path (With Fish Support)
CURRENT_SHELL=$(basename "$SHELL")

if [ "$CURRENT_SHELL" = "fish" ] || [ -n "$FISH_VERSION" ]; then
    FISH_CONFIG_DIR="$HOME/.config/fish"
    PROFILE="$FISH_CONFIG_DIR/config.fish"
    mkdir -p "$FISH_CONFIG_DIR"
    if ! grep -q "$INSTALL_DIR" "$PROFILE" 2>/dev/null; then
        echo "Adding $INSTALL_DIR to PATH in $PROFILE"
        echo "set -gx PATH \$PATH $INSTALL_DIR" >> "$PROFILE"
    fi
else
    if [ -n "$ZSH_VERSION" ] || [ "$CURRENT_SHELL" = "zsh" ]; then
        PROFILE="$HOME/.zshrc"
    else
        PROFILE="$HOME/.bashrc"
    fi
    if ! grep -q "$INSTALL_DIR" "$PROFILE"; then
        echo "Adding $INSTALL_DIR to PATH in $PROFILE"
        echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$PROFILE"
    fi
fi

rm -rf "$TEMP_DIR"
echo "Update/Installation complete!"
