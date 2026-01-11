#!/bin/bash
# Simple Ganesha computer-use test: Open Firefox and go to ebay.com

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║           GANESHA COMPUTER USE TEST                           ║"
echo "║           Opening Firefox -> ebay.com                         ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo

# Ensure we have a display
export DISPLAY="${DISPLAY:-:0}"
echo "[*] Using DISPLAY=$DISPLAY"

# Step 1: Open Firefox
echo "[*] Opening Firefox..."
firefox &
FIREFOX_PID=$!
sleep 3

# Step 2: Wait for window to appear
echo "[*] Waiting for Firefox window..."
for i in {1..10}; do
    if xdotool search --name "Mozilla Firefox" 2>/dev/null | head -1; then
        break
    fi
    sleep 0.5
done

# Step 3: Focus the Firefox window
echo "[*] Focusing Firefox window..."
WINDOW_ID=$(xdotool search --name "Mozilla Firefox" 2>/dev/null | head -1)
if [ -n "$WINDOW_ID" ]; then
    xdotool windowactivate "$WINDOW_ID"
    sleep 0.5
else
    echo "[!] Could not find Firefox window, trying anyway..."
fi

# Step 4: Navigate to ebay.com
echo "[*] Navigating to ebay.com..."
# Ctrl+L to focus address bar
xdotool key ctrl+l
sleep 0.3

# Type the URL
xdotool type --delay 50 "ebay.com"
sleep 0.2

# Press Enter
xdotool key Return

echo
echo "[✓] Done! Firefox should now be loading ebay.com"
echo
echo "Press Enter to close this terminal..."
read
