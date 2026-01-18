# Installation

## Quick Install (Recommended)

```bash
curl -fsSL https://ganesha.dev/install.sh | bash
```

This installs:
- `ganesha` - The CLI tool
- `ganesha-desktop` - The desktop companion app (optional)

---

## Manual Installation

### From Source (Rust)

```bash
# Clone the repository
git clone https://github.com/gtechsd/ganesha.git
cd ganesha

# Build all components
cargo build --release

# Install to ~/.local/bin
cargo install --path crates/ganesha-cli

# Optional: Desktop app
cargo tauri build
```

### Package Managers

**macOS (Homebrew):**
```bash
brew install gtechsd/tap/ganesha
```

**Linux (apt):**
```bash
# Add repository
curl -fsSL https://ganesha.dev/apt-key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/ganesha.gpg
echo "deb [signed-by=/usr/share/keyrings/ganesha.gpg] https://ganesha.dev/apt stable main" | sudo tee /etc/apt/sources.list.d/ganesha.list

sudo apt update
sudo apt install ganesha
```

**Arch Linux (AUR):**
```bash
yay -S ganesha-bin
# or from source
yay -S ganesha-git
```

**Windows (Scoop):**
```powershell
scoop bucket add gtechsd https://github.com/gtechsd/scoop-bucket
scoop install ganesha
```

### Cargo Install

```bash
cargo install ganesha-cli
```

---

## System Requirements

### Minimum
- 4GB RAM
- 500MB disk space
- macOS 12+, Windows 10+, or Linux (glibc 2.17+)

### Recommended
- 8GB+ RAM (for local LLMs)
- SSD storage
- GPU (for local vision models)

### For Voice Features
- Microphone
- Speakers/headphones
- Optional: Dedicated audio interface

### For Vision Features
- Display (obviously)
- Permissions for screen capture
- Permissions for accessibility/input control

---

## Post-Installation

### 1. Verify Installation

```bash
ganesha --version
# Ganesha 4.0.0 - The Obstacle Remover
```

### 2. Run Configuration

```bash
ganesha --configure
```

This will:
- Detect available LLM providers
- Set up default risk level
- Configure voice (if desired)
- Set up MCP servers

### 3. Test It Out

```bash
# Simple test
ganesha "what time is it"

# Check system
ganesha "show me my system info"
```

---

## Troubleshooting

### Command Not Found

Add to your PATH:
```bash
# bash/zsh
export PATH="$HOME/.local/bin:$PATH"

# fish
fish_add_path ~/.local/bin
```

### Permission Denied (macOS)

Grant accessibility permissions:
1. System Preferences → Privacy & Security
2. Accessibility → Add Terminal/Ganesha
3. Screen Recording → Add Terminal/Ganesha (for vision)

### Permission Denied (Linux)

```bash
# For input control
sudo usermod -aG input $USER

# For screen capture (Wayland)
# Use X11 or grant portal permissions
```

---

## Uninstalling

```bash
# Remove binary
rm ~/.local/bin/ganesha

# Remove configuration (optional)
rm -rf ~/.ganesha

# Remove desktop app (macOS)
rm -rf /Applications/Ganesha.app

# Remove desktop app (Linux)
rm ~/.local/share/applications/ganesha.desktop
```

---

## Next Steps

- [Configuration →](configuration.md)
- [First Commands →](first-commands.md)
