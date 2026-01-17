#!/bin/bash
#
# Ganesha Test VM Setup Script
# For Ubuntu 24.04 Desktop (vanilla install)
#
# Run as: sudo bash setup_test_vm.sh
#

set -e

echo "========================================"
echo "  GANESHA TEST VM SETUP"
echo "  Ubuntu 24.04"
echo "========================================"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root: sudo bash setup_test_vm.sh"
    exit 1
fi

# Get the actual user (not root)
ACTUAL_USER="${SUDO_USER:-$USER}"
ACTUAL_HOME=$(getent passwd "$ACTUAL_USER" | cut -d: -f6)

echo "[1/8] Updating system..."
apt update && apt upgrade -y

echo "[2/8] Installing build essentials..."
apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake \
    git \
    curl \
    wget \
    jq

echo "[3/8] Installing X11 and GUI automation tools..."
apt install -y \
    libxcb1-dev \
    libxcb-shm0-dev \
    libxcb-randr0-dev \
    libxcb-xfixes0-dev \
    libxcb-shape0-dev \
    libx11-dev \
    libxext-dev \
    libxrandr-dev \
    xdotool \
    xclip \
    scrot \
    imagemagick

echo "[4/8] Installing test applications..."
apt install -y \
    blender \
    gimp \
    firefox \
    gedit \
    gnome-calculator \
    nautilus

echo "[5/8] Setting up SSH server..."
apt install -y openssh-server
systemctl enable ssh
systemctl start ssh

# Configure SSH for easier access
sed -i 's/#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config
sed -i 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' /etc/ssh/sshd_config
systemctl restart ssh

echo "[6/8] Installing Rust for user $ACTUAL_USER..."
sudo -u "$ACTUAL_USER" bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
sudo -u "$ACTUAL_USER" bash -c 'source $HOME/.cargo/env && rustup default stable'

echo "[7/8] Setting up display access..."
# Allow X11 forwarding
sed -i 's/#X11Forwarding yes/X11Forwarding yes/' /etc/ssh/sshd_config
sed -i 's/#X11DisplayOffset 10/X11DisplayOffset 10/' /etc/ssh/sshd_config
systemctl restart ssh

# Create a script to export DISPLAY for SSH sessions
cat > /etc/profile.d/ganesha-display.sh << 'EOF'
# Auto-detect and export DISPLAY for Ganesha
if [ -z "$DISPLAY" ]; then
    # Try to find an active display
    if [ -e /tmp/.X11-unix/X0 ]; then
        export DISPLAY=:0
    elif [ -e /tmp/.X11-unix/X1 ]; then
        export DISPLAY=:1
    fi
fi

# Allow local connections to X
if [ -n "$DISPLAY" ] && command -v xhost &> /dev/null; then
    xhost +local: 2>/dev/null || true
fi
EOF
chmod +x /etc/profile.d/ganesha-display.sh

# Create ganesha workspace
sudo -u "$ACTUAL_USER" mkdir -p "$ACTUAL_HOME/ganesha-testing"

echo "[8/8] Gathering system information..."

# Get network info
IP_ADDR=$(hostname -I | awk '{print $1}')
HOSTNAME=$(hostname)

# Get display info
DISPLAY_INFO=$(who | grep -E '\(:' | head -1 || echo "No active display session")

# Check if desktop is running
DESKTOP_SESSION="${XDG_CURRENT_DESKTOP:-unknown}"

# Generate connection info file
cat > "$ACTUAL_HOME/ganesha-testing/CONNECTION_INFO.txt" << EOF
========================================
  GANESHA TEST VM - CONNECTION INFO
========================================

SSH Connection:
  Host: $IP_ADDR
  User: $ACTUAL_USER
  Command: ssh $ACTUAL_USER@$IP_ADDR

VM Details:
  Hostname: $HOSTNAME
  Ubuntu: $(lsb_release -ds)
  Kernel: $(uname -r)
  Desktop: $DESKTOP_SESSION

Display Info:
  Active Session: $DISPLAY_INFO
  X11 Sockets: $(ls /tmp/.X11-unix/ 2>/dev/null | tr '\n' ' ' || echo "none")

Network:
  IP Address: $IP_ADDR
  LLM API Reachable: $(curl -s --connect-timeout 2 http://localhost:1234/v1/models > /dev/null && echo "YES" || echo "NO - check network")

Installed Tools:
  Rust: $(sudo -u "$ACTUAL_USER" bash -c 'source $HOME/.cargo/env && rustc --version 2>/dev/null' || echo "not in path yet - relogin")
  xdotool: $(xdotool --version 2>/dev/null | head -1 || echo "missing")
  Blender: $(blender --version 2>/dev/null | head -1 || echo "missing")
  Git: $(git --version 2>/dev/null || echo "missing")

To Clone Ganesha (after SSH):
  git clone https://bill-dev-linux-1/gtechsd/ganesha-ai.git ~/ganesha-testing/ganesha-ai
  cd ~/ganesha-testing/ganesha-ai/ganesha-rs
  cargo build --features computer-use

Quick Test Command:
  DISPLAY=:0 cargo run --example ganesha_autonomous --features computer-use -- --task "open calculator"

========================================
COPY EVERYTHING BELOW THIS LINE TO CLAUDE:
========================================

VM_IP=$IP_ADDR
VM_USER=$ACTUAL_USER
VM_DISPLAY=:0
RUST_INSTALLED=true
XDOTOOL_INSTALLED=true
BLENDER_INSTALLED=true
LLM_API_REACHABLE=$(curl -s --connect-timeout 2 http://localhost:1234/v1/models > /dev/null && echo "true" || echo "false")

EOF

chown "$ACTUAL_USER:$ACTUAL_USER" "$ACTUAL_HOME/ganesha-testing/CONNECTION_INFO.txt"

echo ""
echo "========================================"
echo "  SETUP COMPLETE!"
echo "========================================"
echo ""
echo "Connection info saved to:"
echo "  $ACTUAL_HOME/ganesha-testing/CONNECTION_INFO.txt"
echo ""
echo "Quick summary:"
echo "  IP Address: $IP_ADDR"
echo "  SSH User: $ACTUAL_USER"
echo "  SSH Command: ssh $ACTUAL_USER@$IP_ADDR"
echo ""
echo "IMPORTANT: Log out and back in for Rust to be in PATH"
echo ""
echo "Then cat the connection info file and paste to Claude:"
echo "  cat ~/ganesha-testing/CONNECTION_INFO.txt"
echo ""
