#!/bin/bash
#
# Ganesha Installer - The Remover of Obstacles
# Cross-platform installer for Linux and macOS
#
set -e

VERSION="4.0.0-beta"
REPO="G-TechSD/ganesha-ai"
INSTALL_DIR="${GANESHA_INSTALL_DIR:-$HOME/.local/bin}"

echo ""
echo "  ██████   █████  ███    ██ ███████ ███████ ██   ██  █████"
echo " ██       ██   ██ ████   ██ ██      ██      ██   ██ ██   ██"
echo " ██   ███ ███████ ██ ██  ██ █████   ███████ ███████ ███████"
echo " ██    ██ ██   ██ ██  ██ ██ ██           ██ ██   ██ ██   ██"
echo "  ██████  ██   ██ ██   ████ ███████ ███████ ██   ██ ██   ██"
echo ""
echo "        ✦ The Remover of Obstacles ✦  v${VERSION}"
echo ""

# Detect OS and Architecture
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case "$OS" in
        linux*)  OS="linux" ;;
        darwin*) OS="macos" ;;
        *)       echo "❌ Unsupported OS: $OS"; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *)             echo "❌ Unsupported architecture: $ARCH"; exit 1 ;;
    esac

    echo "📦 Detected: ${OS} ${ARCH}"
}

# Download binary from GitHub releases
download_binary() {
    local url="https://github.com/${REPO}/releases/download/v${VERSION}/ganesha-${OS}-${ARCH}.tar.gz"
    local tmp_dir=$(mktemp -d)
    local archive="${tmp_dir}/ganesha.tar.gz"

    echo "⬇️  Downloading from: ${url}"

    # Try curl first, then wget
    if command -v curl &> /dev/null; then
        if curl -fsSL "$url" -o "$archive" 2>/dev/null; then
            extract_and_install "$archive" "$tmp_dir"
            return 0
        fi
    elif command -v wget &> /dev/null; then
        if wget -q "$url" -O "$archive" 2>/dev/null; then
            extract_and_install "$archive" "$tmp_dir"
            return 0
        fi
    fi

    rm -rf "$tmp_dir"
    return 1
}

# Extract and install binary
extract_and_install() {
    local archive="$1"
    local tmp_dir="$2"

    echo "📂 Extracting..."
    tar -xzf "$archive" -C "$tmp_dir"

    # Find the binary
    local binary=$(find "$tmp_dir" -name "ganesha" -type f -perm -u+x 2>/dev/null | head -1)
    if [ -z "$binary" ]; then
        binary="${tmp_dir}/ganesha"
    fi

    if [ ! -f "$binary" ]; then
        echo "❌ Binary not found in archive"
        return 1
    fi

    # Install
    mkdir -p "$INSTALL_DIR"
    cp "$binary" "$INSTALL_DIR/ganesha"
    chmod +x "$INSTALL_DIR/ganesha"

    rm -rf "$tmp_dir"
    echo "✅ Installed to: ${INSTALL_DIR}/ganesha"
}

# Build from source (fallback)
build_from_source() {
    echo ""
    echo "🔨 Pre-built binary not available. Building from source..."
    echo ""

    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        echo "📥 Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    # Install system dependencies
    if [[ "$OS" == "linux" ]]; then
        echo "📦 Installing system dependencies..."
        if command -v apt-get &> /dev/null; then
            sudo apt-get update -qq
            sudo apt-get install -y -qq libasound2-dev pkg-config libssl-dev libudev-dev build-essential
        elif command -v dnf &> /dev/null; then
            sudo dnf install -y alsa-lib-devel pkg-config openssl-devel libudev-devel gcc
        elif command -v pacman &> /dev/null; then
            sudo pacman -S --noconfirm alsa-lib pkg-config openssl base-devel
        fi
    elif [[ "$OS" == "macos" ]]; then
        # Check for Xcode Command Line Tools (required for compilation)
        if ! xcode-select -p &> /dev/null; then
            echo "📥 Installing Xcode Command Line Tools (required for compilation)..."
            echo "   A dialog will appear - click 'Install' and wait for it to complete."
            xcode-select --install
            echo ""
            echo "⏳ After installation completes, run this installer again:"
            echo "   curl -sSL https://raw.githubusercontent.com/G-TechSD/ganesha-ai/main/install.sh | bash"
            exit 0
        fi

        # Install pkg-config via Homebrew if needed
        if ! command -v pkg-config &> /dev/null; then
            if command -v brew &> /dev/null; then
                brew install pkg-config openssl
            fi
        fi
    fi

    # Clone and build
    local tmp_dir=$(mktemp -d)
    echo "📥 Cloning repository..."
    git clone --depth 1 "https://github.com/${REPO}.git" "$tmp_dir/ganesha" 2>/dev/null || \
    git clone --depth 1 "https://github.com/G-TechSD/ganesha-ai.git" "$tmp_dir/ganesha"

    cd "$tmp_dir/ganesha/ganesha-rs/ganesha4"
    echo "🔨 Building (this may take a few minutes)..."
    cargo build --release

    mkdir -p "$INSTALL_DIR"
    cp target/release/ganesha "$INSTALL_DIR/ganesha"
    chmod +x "$INSTALL_DIR/ganesha"

    cd /
    rm -rf "$tmp_dir"
    echo "✅ Built and installed to: ${INSTALL_DIR}/ganesha"
}

# Add to PATH
setup_path() {
    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        echo ""
        echo "📍 Adding ${INSTALL_DIR} to PATH..."

        local shell_rc=""
        if [ -n "$ZSH_VERSION" ] || [ -f "$HOME/.zshrc" ]; then
            shell_rc="$HOME/.zshrc"
        elif [ -n "$BASH_VERSION" ] || [ -f "$HOME/.bashrc" ]; then
            shell_rc="$HOME/.bashrc"
        fi

        if [ -n "$shell_rc" ]; then
            echo "" >> "$shell_rc"
            echo "# Ganesha" >> "$shell_rc"
            echo "export PATH=\"${INSTALL_DIR}:\$PATH\"" >> "$shell_rc"
            echo "   Added to ${shell_rc}"
        fi

        export PATH="${INSTALL_DIR}:$PATH"
    fi
}

# Verify installation
verify_install() {
    if command -v ganesha &> /dev/null || [ -x "${INSTALL_DIR}/ganesha" ]; then
        echo ""
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        "${INSTALL_DIR}/ganesha" --version 2>/dev/null || echo "ganesha v${VERSION}"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo ""
        echo "🎉 Installation complete!"
        echo ""
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "⚠️  IMPORTANT: Close and reopen your terminal before using Ganesha"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo ""
        echo "   Then get started with:"
        echo "   ganesha \"hello world\"    # Quick task"
        echo "   ganesha -i               # Interactive mode"
        echo "   ganesha --help           # Show all options"
        echo ""
    else
        echo "❌ Installation failed"
        exit 1
    fi
}

# Main
main() {
    detect_platform

    # Try downloading pre-built binary first
    if download_binary; then
        setup_path
        verify_install
    else
        echo "⚠️  Pre-built binary not available for ${OS}-${ARCH}"
        build_from_source
        setup_path
        verify_install
    fi
}

main "$@"
