#!/bin/bash
# Ganesha Installer for Linux and macOS
# https://bill-dev-linux-1/gtechsd/ganesha-ai
#
# Usage:
#   curl -sSL https://bill-dev-linux-1/gtechsd/ganesha-ai/-/releases/permalink/latest/downloads/install.sh | bash
#
# Or download and run:
#   chmod +x install.sh && ./install.sh

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
DIM='\033[2m'
NC='\033[0m' # No Color

# Detect OS and architecture
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$os" in
        linux)
            case "$arch" in
                x86_64|amd64) echo "linux-x86_64" ;;
                aarch64|arm64) echo "linux-aarch64" ;;
                *) echo "unsupported" ;;
            esac
            ;;
        darwin)
            case "$arch" in
                x86_64|amd64) echo "macos-x86_64" ;;
                arm64|aarch64) echo "macos-aarch64" ;;
                *) echo "unsupported" ;;
            esac
            ;;
        *)
            echo "unsupported"
            ;;
    esac
}

# Print banner
echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}              Ganesha Installer${NC}"
echo -e "${CYAN}         The Remover of Obstacles${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo ""

# Detect platform
PLATFORM=$(detect_platform)

if [ "$PLATFORM" = "unsupported" ]; then
    echo -e "${RED}Error: Unsupported platform${NC}"
    echo "Supported platforms: Linux (x86_64, aarch64), macOS (x86_64, arm64)"
    exit 1
fi

echo -e "${DIM}Detected platform: ${PLATFORM}${NC}"

# Configuration
GITLAB_URL="https://bill-dev-linux-1/gtechsd/ganesha-ai"
VERSION="${GANESHA_VERSION:-latest}"
INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="ganesha"

# Determine download URL
if [ "$VERSION" = "latest" ]; then
    DOWNLOAD_URL="${GITLAB_URL}/-/releases/permalink/latest/downloads/ganesha-${PLATFORM}.tar.gz"
else
    DOWNLOAD_URL="${GITLAB_URL}/-/releases/${VERSION}/downloads/ganesha-${PLATFORM}.tar.gz"
fi

echo -e "${DIM}Download URL: ${DOWNLOAD_URL}${NC}"
echo ""

# Create temp directory
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

# Download
echo -e "${CYAN}Downloading Ganesha...${NC}"
if command -v curl &> /dev/null; then
    curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/ganesha.tar.gz" || {
        echo -e "${RED}Download failed${NC}"
        echo "URL: $DOWNLOAD_URL"
        exit 1
    }
elif command -v wget &> /dev/null; then
    wget -q "$DOWNLOAD_URL" -O "$TMP_DIR/ganesha.tar.gz" || {
        echo -e "${RED}Download failed${NC}"
        echo "URL: $DOWNLOAD_URL"
        exit 1
    }
else
    echo -e "${RED}Error: curl or wget required${NC}"
    exit 1
fi

# Extract
echo -e "${CYAN}Extracting...${NC}"
tar -xzf "$TMP_DIR/ganesha.tar.gz" -C "$TMP_DIR"

# Find the binary
BINARY_PATH=$(find "$TMP_DIR" -name "ganesha" -type f | head -1)

if [ -z "$BINARY_PATH" ]; then
    echo -e "${RED}Error: Binary not found in archive${NC}"
    exit 1
fi

# Create install directory
mkdir -p "$INSTALL_DIR"

# Install
echo -e "${CYAN}Installing to ${INSTALL_DIR}...${NC}"
cp "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo ""
echo -e "${GREEN}✓ Ganesha installed successfully!${NC}"
echo ""

# Check if in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${YELLOW}⚠ ${INSTALL_DIR} is not in your PATH${NC}"
    echo ""

    # Detect shell
    SHELL_NAME=$(basename "$SHELL")
    case "$SHELL_NAME" in
        bash)
            PROFILE="$HOME/.bashrc"
            ;;
        zsh)
            PROFILE="$HOME/.zshrc"
            ;;
        *)
            PROFILE="$HOME/.profile"
            ;;
    esac

    echo -e "${DIM}Add to your PATH by running:${NC}"
    echo ""
    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> $PROFILE"
    echo "  source $PROFILE"
    echo ""
else
    echo -e "${GREEN}✓ You can now run 'ganesha' from anywhere!${NC}"
fi

# Version check
echo ""
echo -e "${DIM}Installed version:${NC}"
"$INSTALL_DIR/$BINARY_NAME" --version 2>/dev/null || echo "  (run 'ganesha --version' to verify)"

# Optional: Browser automation
echo ""
if command -v node &> /dev/null; then
    echo -e "${DIM}Node.js detected. For browser automation:${NC}"
    echo "  npx playwright install chromium"
else
    echo -e "${DIM}Optional: Install Node.js for browser automation features${NC}"
fi

echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${DIM}Documentation: ${GITLAB_URL}${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo ""
