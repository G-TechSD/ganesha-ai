#!/bin/bash
set -e

echo "🐘 Ganesha Installer - The Remover of Obstacles"
echo "=============================================="

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "Rust not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Install dependencies for voice
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "🐧 Linux detected. Checking system dependencies..."
    if command -v apt-get &> /dev/null; then
        sudo apt-get update
        sudo apt-get install -y libasound2-dev pkg-config libssl-dev libudev-dev
    elif command -v dnf &> /dev/null; then
        sudo dnf install -y alsa-lib-devel pkg-config openssl-devel libudev-devel
    fi
elif [[ "$OSTYPE" == "darwin"* ]]; then
    echo "🍎 macOS detected. Using CoreAudio (built-in)."
    # Check for pkg-config which might be needed for some crates
    if ! command -v pkg-config &> /dev/null; then
        echo "⚠️  pkg-config not found. It might be needed for some builds."
        echo "   Recommendation: brew install pkg-config openssl"
    fi
fi

echo "Building Ganesha with Voice support..."
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd "$SCRIPT_DIR/ganesha-rs"
cargo build --release --features "voice"

echo "Installing binary..."
mkdir -p ~/.local/bin
cp target/release/ganesha ~/.local/bin/

# Add to PATH if needed
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
    echo "Added ~/.local/bin to PATH. Please restart your shell."
fi

echo "✅ Ganesha installed successfully!"
echo "Run 'ganesha --help' to get started."
