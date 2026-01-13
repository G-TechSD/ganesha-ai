#!/bin/bash
# Build Ganesha releases for all platforms
#
# Prerequisites:
#   - Rust toolchain
#   - cross (cargo install cross)
#   - Docker (for cross-compilation)
#
# Usage:
#   ./scripts/build-releases.sh
#   ./scripts/build-releases.sh --version 3.0.1

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Get version from Cargo.toml or argument
VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')}"
VERSION="${VERSION#--version=}"
VERSION="${VERSION#--version }"

echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}         Ganesha Release Builder v${VERSION}${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo ""

# Output directory
DIST_DIR="dist"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Targets to build
declare -A TARGETS=(
    ["linux-x86_64"]="x86_64-unknown-linux-gnu"
    ["linux-aarch64"]="aarch64-unknown-linux-gnu"
    ["macos-x86_64"]="x86_64-apple-darwin"
    ["macos-aarch64"]="aarch64-apple-darwin"
    ["windows-x86_64"]="x86_64-pc-windows-gnu"
)

# Check for cross
if ! command -v cross &> /dev/null; then
    echo -e "${YELLOW}Warning: 'cross' not found. Installing...${NC}"
    cargo install cross
fi

# Build function
build_target() {
    local name=$1
    local target=$2

    echo ""
    echo -e "${CYAN}Building ${name}...${NC}"
    echo -e "  Target: ${target}"

    # Check if we can build natively or need cross
    local current_target=$(rustc -vV | grep host | sed 's/host: //')

    if [[ "$target" == "$current_target" ]]; then
        echo -e "  Method: Native build"
        cargo build --release --target "$target"
    elif [[ "$target" == *"apple"* ]] && [[ "$(uname)" != "Darwin" ]]; then
        echo -e "${YELLOW}  Skipping macOS target (requires macOS host)${NC}"
        return 1
    else
        echo -e "  Method: Cross compilation"
        cross build --release --target "$target"
    fi

    return 0
}

# Package function
package_target() {
    local name=$1
    local target=$2

    local binary_name="ganesha"
    local ext=""

    if [[ "$name" == *"windows"* ]]; then
        binary_name="ganesha.exe"
        ext=".zip"
    else
        ext=".tar.gz"
    fi

    local binary_path="target/${target}/release/${binary_name}"
    local package_name="ganesha-${VERSION}-${name}${ext}"
    local package_dir="$DIST_DIR/ganesha-${name}"

    if [[ ! -f "$binary_path" ]]; then
        echo -e "${YELLOW}  Binary not found: ${binary_path}${NC}"
        return 1
    fi

    echo -e "  Packaging: ${package_name}"

    # Create package directory
    mkdir -p "$package_dir"
    cp "$binary_path" "$package_dir/"
    cp "scripts/install.sh" "$package_dir/" 2>/dev/null || true
    cp "README.md" "$package_dir/" 2>/dev/null || true
    cp "LICENSE" "$package_dir/" 2>/dev/null || true

    # Create archive
    cd "$DIST_DIR"
    if [[ "$ext" == ".zip" ]]; then
        zip -rq "../${DIST_DIR}/${package_name}" "ganesha-${name}"
    else
        tar -czf "${package_name}" "ganesha-${name}"
    fi
    cd ..

    # Cleanup
    rm -rf "$package_dir"

    # Generate checksum
    cd "$DIST_DIR"
    sha256sum "${package_name}" > "${package_name}.sha256"
    cd ..

    echo -e "${GREEN}  ✓ Created: ${DIST_DIR}/${package_name}${NC}"
}

# Build all targets
BUILT=()
FAILED=()

for name in "${!TARGETS[@]}"; do
    target="${TARGETS[$name]}"

    if build_target "$name" "$target"; then
        BUILT+=("$name")
    else
        FAILED+=("$name")
    fi
done

echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}                    Packaging${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"

# Package successful builds
for name in "${BUILT[@]}"; do
    target="${TARGETS[$name]}"
    package_target "$name" "$target"
done

# Also copy install scripts to dist
cp scripts/install.sh "$DIST_DIR/"
cp scripts/install.ps1 "$DIST_DIR/"

echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${CYAN}                    Summary${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
echo ""

echo -e "${GREEN}Built:${NC}"
for name in "${BUILT[@]}"; do
    echo -e "  ✓ ${name}"
done

if [[ ${#FAILED[@]} -gt 0 ]]; then
    echo ""
    echo -e "${YELLOW}Skipped (requires different host):${NC}"
    for name in "${FAILED[@]}"; do
        echo -e "  ○ ${name}"
    done
fi

echo ""
echo -e "${CYAN}Output directory: ${DIST_DIR}/${NC}"
ls -la "$DIST_DIR/"

echo ""
echo -e "${GREEN}Release packages ready for upload!${NC}"
echo ""

# Print upload instructions
echo -e "${CYAN}To create a GitLab release:${NC}"
echo ""
echo "  1. Tag the release:"
echo "     git tag -a v${VERSION} -m 'Release ${VERSION}'"
echo "     git push origin v${VERSION}"
echo ""
echo "  2. Upload assets via GitLab UI or API"
echo ""
echo "  3. Or use gitlab-release-cli:"
echo "     release-cli create --name \"Ganesha v${VERSION}\" --tag-name \"v${VERSION}\" \\"
for f in "$DIST_DIR"/*.tar.gz "$DIST_DIR"/*.zip; do
    [[ -f "$f" ]] && echo "       --assets-link \"{\\\"name\\\":\\\"$(basename $f)\\\",\\\"url\\\":\\\"file://$f\\\"}\" \\"
done
echo ""
