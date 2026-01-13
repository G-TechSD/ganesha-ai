#!/bin/bash
# Test MCP integration

export GANESHA_DEBUG=1

echo "=== Testing MCP client directly ==="

# Test 1: Verify playwright MCP server works standalone
echo "Test 1: Playwright MCP server standalone..."
RESULT=$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | timeout 10 npx -y @playwright/mcp@latest 2>/dev/null)

if echo "$RESULT" | grep -q "browser_navigate"; then
    echo "✓ Playwright MCP server works - has browser_navigate tool"
else
    echo "✗ Playwright MCP server failed"
    echo "Result: $RESULT"
    exit 1
fi

# Test 2: Test ganesha MCP connection
echo ""
echo "Test 2: Testing ganesha MCP connection..."

# Create a simple expect-like script using bash
cat > /tmp/test_ganesha_mcp.py << 'PYTHON'
import subprocess
import sys
import time
import os

os.environ['GANESHA_DEBUG'] = '1'

proc = subprocess.Popen(
    [os.path.expanduser('~/.local/bin/ganesha')],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True
)

# Wait for startup
time.sleep(2)

# Send commands
commands = [
    "/mcp\n",      # Open MCP menu
    "2\n",         # Connect server
    "2\n",         # Select playwright (usually option 2)
    "\n",          # Press enter to continue
    "b\n",         # Back to main
    "go to google.com\n",  # Test MCP tool usage
    "exit\n"       # Exit
]

for cmd in commands:
    proc.stdin.write(cmd)
    proc.stdin.flush()
    time.sleep(1)

# Get output
try:
    stdout, stderr = proc.communicate(timeout=30)
except subprocess.TimeoutExpired:
    proc.kill()
    stdout, stderr = proc.communicate()

print("=== STDERR (debug) ===")
print(stderr)
print("\n=== STDOUT ===")
print(stdout[-2000:] if len(stdout) > 2000 else stdout)
PYTHON

python3 /tmp/test_ganesha_mcp.py

echo ""
echo "=== Test complete ==="
