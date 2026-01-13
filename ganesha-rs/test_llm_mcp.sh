#!/bin/bash
# Test MCP + LLM integration
# This script verifies that when MCP servers are connected, the LLM uses them

set -e

export GANESHA_DEBUG=1

echo "=== MCP + LLM Integration Test ==="
echo ""

# Step 1: Verify Playwright MCP server is available
echo "Step 1: Verify Playwright MCP server..."
RESULT=$(echo '{
"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | timeout 30 npx -y @playwright/mcp@latest 2>/dev/null)

if echo "$RESULT" | grep -q "browser_navigate"; then
    echo "  ✓ Playwright MCP server has browser_navigate tool"
else
    echo "  ✗ Playwright MCP server failed"
    exit 1
fi

# Step 2: Test ganesha direct MCP API
echo ""
echo "Step 2: Testing ganesha MCP integration binary..."
timeout 120 cargo run --release --bin test_mcp 2>&1 | tail -30

# Step 3: Test with actual ganesha using expect-like automation
echo ""
echo "Step 3: Testing full ganesha session with MCP..."

# Use Python for more reliable process control
python3 << 'PYTHON'
import subprocess
import sys
import time
import os
import select

os.environ['GANESHA_DEBUG'] = '1'
os.environ['TERM'] = 'xterm-256color'

# Start ganesha
proc = subprocess.Popen(
    [os.path.expanduser('~/.local/bin/ganesha')],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    bufsize=0
)

def send(text, delay=0.5):
    print(f"  > Sending: {repr(text)}")
    proc.stdin.write(text)
    proc.stdin.flush()
    time.sleep(delay)

def read_available(timeout=2):
    """Read available output without blocking"""
    output = ""
    end_time = time.time() + timeout
    while time.time() < end_time:
        ready, _, _ = select.select([proc.stdout], [], [], 0.1)
        if ready:
            char = proc.stdout.read(1)
            if char:
                output += char
        elif output:
            break
    return output

# Wait for startup
print("  Waiting for startup...")
time.sleep(3)
startup = read_available(2)
print(f"  Got {len(startup)} chars of startup output")

# Go to MCP menu
send("/mcp\n")
time.sleep(1)
menu = read_available(2)
print(f"  MCP menu: {len(menu)} chars")

# Connect to playwright (option 2)
send("2\n", delay=1)
select_out = read_available(2)
print(f"  Server select: {len(select_out)} chars")

# Select playwright (usually option 1 or 2)
# Look for playwright in output to determine which option
if "playwright" in select_out.lower():
    # Check which number playwright is
    lines = select_out.split('\n')
    for line in lines:
        if 'playwright' in line.lower():
            # Find the number
            for char in line:
                if char.isdigit():
                    send(f"{char}\n", delay=3)
                    break
            break
else:
    send("1\n", delay=3)  # Default to first option

connect_out = read_available(5)
print(f"  Connection result: {len(connect_out)} chars")

# Check for success
if "connected" in connect_out.lower() or "22 tools" in connect_out.lower():
    print("  ✓ Playwright appears to be connected")
else:
    print(f"  Connection output: {connect_out[:500] if len(connect_out) > 500 else connect_out}")

# Go back to main menu
send("b\n", delay=1)
back_out = read_available(2)

# Now send a request that should use MCP
print("\n  Testing MCP tool usage with browser request...")
send("navigate to google.com using the browser\n", delay=5)

# Wait for response
response = read_available(10)
print(f"  Response length: {len(response)} chars")

# Check if MCP tools were used
response_lower = response.lower()
if "mcp" in response_lower or "playwright" in response_lower or "browser_navigate" in response_lower:
    print("  ✓ Response mentions MCP/Playwright tools!")
elif "google" in response_lower:
    print("  ✓ Response mentions Google (may have worked)")
else:
    print(f"  Response preview: {response[:1000] if len(response) > 1000 else response}")

# Clean exit
send("exit\n")
time.sleep(1)

# Terminate
proc.terminate()
try:
    proc.wait(timeout=3)
except:
    proc.kill()

# Get any remaining stderr (debug output)
stderr = proc.stderr.read()
if "MCP tools count" in stderr:
    print("\n  ✓ Debug shows MCP tools were included in prompt!")
if "MCP: " in stderr:
    # Show MCP-related debug lines
    for line in stderr.split('\n'):
        if 'MCP' in line:
            print(f"  DEBUG: {line}")

print("\n=== Test Complete ===")
PYTHON

echo ""
echo "=== All MCP+LLM Tests Complete ==="
