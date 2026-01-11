#!/bin/bash
# Test script for 1000 cat facts challenge

cd /home/bill/projects/ganesha-ai/ganesha-rs

# Clean up any previous test file
rm -f /home/bill/projects/cats.html

echo "=== Testing Ganesha Cat Facts Challenge ==="
echo "Model: gpt-oss-20b"
echo "Max tokens: 65536"
echo ""

# Run ganesha with a piped prompt
echo "make a one page fancy modern website with 1000 cat facts in little mouse shaped bubbles which scurry across the screen and save it as cats.html in /home/bill/projects" | timeout 600 ./target/release/ganesha 2>&1

echo ""
echo "=== Checking results ==="

if [ -f /home/bill/projects/cats.html ]; then
    echo "SUCCESS: cats.html was created!"
    echo "File size: $(wc -c < /home/bill/projects/cats.html) bytes"
    echo "Line count: $(wc -l < /home/bill/projects/cats.html) lines"

    # Count how many facts are in the file
    fact_count=$(grep -oE '"[0-9]+\.' /home/bill/projects/cats.html | wc -l)
    echo "Approximate fact count: $fact_count"

    # Show first few lines
    echo ""
    echo "=== First 20 lines ==="
    head -20 /home/bill/projects/cats.html
else
    echo "FAILED: cats.html was NOT created"
fi
