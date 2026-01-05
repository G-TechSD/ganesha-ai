#!/bin/bash
#
# Build Ganesha for all platforms
#
# Prerequisites:
#   rustup target add x86_64-unknown-linux-gnu
#   rustup target add x86_64-apple-darwin
#   rustup target add aarch64-apple-darwin
#   rustup target add x86_64-pc-windows-gnu
#
# For cross-compilation:
#   cargo install cross
#

set -e

VERSION="3.0.0"
OUTPUT_DIR="dist"

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║           Building Ganesha v${VERSION}                             ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo

mkdir -p "$OUTPUT_DIR"

# Linux x86_64
echo "Building for Linux x86_64..."
if command -v cross &> /dev/null; then
    cross build --release --target x86_64-unknown-linux-gnu
else
    cargo build --release --target x86_64-unknown-linux-gnu 2>/dev/null || cargo build --release
fi
cp target/x86_64-unknown-linux-gnu/release/ganesha "$OUTPUT_DIR/ganesha-linux-x86_64" 2>/dev/null || \
cp target/release/ganesha "$OUTPUT_DIR/ganesha-linux-x86_64"
cp target/x86_64-unknown-linux-gnu/release/ganesha-daemon "$OUTPUT_DIR/ganesha-daemon-linux-x86_64" 2>/dev/null || \
cp target/release/ganesha-daemon "$OUTPUT_DIR/ganesha-daemon-linux-x86_64" 2>/dev/null || true
echo "  ✓ Linux x86_64"

# Linux ARM64
echo "Building for Linux ARM64..."
if command -v cross &> /dev/null; then
    cross build --release --target aarch64-unknown-linux-gnu
    cp target/aarch64-unknown-linux-gnu/release/ganesha "$OUTPUT_DIR/ganesha-linux-arm64"
    echo "  ✓ Linux ARM64"
else
    echo "  ⚠ Skipped (install 'cross' for ARM builds)"
fi

# macOS x86_64
echo "Building for macOS x86_64..."
if [[ "$OSTYPE" == "darwin"* ]] || command -v cross &> /dev/null; then
    if command -v cross &> /dev/null; then
        cross build --release --target x86_64-apple-darwin
    else
        cargo build --release --target x86_64-apple-darwin 2>/dev/null || true
    fi
    if [[ -f target/x86_64-apple-darwin/release/ganesha ]]; then
        cp target/x86_64-apple-darwin/release/ganesha "$OUTPUT_DIR/ganesha-macos-x86_64"
        echo "  ✓ macOS x86_64"
    else
        echo "  ⚠ Skipped"
    fi
else
    echo "  ⚠ Skipped (not on macOS)"
fi

# macOS ARM64 (Apple Silicon)
echo "Building for macOS ARM64..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    cargo build --release --target aarch64-apple-darwin 2>/dev/null || true
    if [[ -f target/aarch64-apple-darwin/release/ganesha ]]; then
        cp target/aarch64-apple-darwin/release/ganesha "$OUTPUT_DIR/ganesha-macos-arm64"
        echo "  ✓ macOS ARM64"
    else
        echo "  ⚠ Skipped"
    fi
else
    echo "  ⚠ Skipped (not on macOS)"
fi

# Windows x86_64
echo "Building for Windows x86_64..."
if command -v cross &> /dev/null; then
    cross build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/ganesha.exe "$OUTPUT_DIR/ganesha-windows-x86_64.exe"
    echo "  ✓ Windows x86_64"
elif command -v x86_64-w64-mingw32-gcc &> /dev/null; then
    cargo build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/ganesha.exe "$OUTPUT_DIR/ganesha-windows-x86_64.exe"
    echo "  ✓ Windows x86_64"
else
    echo "  ⚠ Skipped (install mingw-w64 or 'cross')"
fi

echo
echo "Build complete!"
echo
echo "Output files:"
ls -lh "$OUTPUT_DIR"
echo
echo "Binary sizes (stripped):"
for f in "$OUTPUT_DIR"/*; do
    if [[ -f "$f" ]]; then
        size=$(du -h "$f" | cut -f1)
        echo "  $f: $size"
    fi
done
