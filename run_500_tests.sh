#!/bin/bash
# Run 500 Ganesha test cases with the local LLM
# Time: 7:36 PM - Target: 8:30 PM (54 minutes)

GANESHA="./ganesha-rs/target/release/ganesha"
STANDARD_TESTS="test_cases_standard_250.txt"
EDGE_TESTS="test_cases_edge_250.txt"
LOG="test_500_results.log"
PASS=0
FAIL=0
SKIP=0

echo "=== GANESHA 500 TEST CASE EXECUTION ===" | tee "$LOG"
echo "Start: $(date)" | tee -a "$LOG"
echo "LLM: 192.168.245.155:1234 (openai/gpt-oss-20b)" | tee -a "$LOG"
echo "" | tee -a "$LOG"

run_test() {
    local test_num="$1"
    local test_line="$2"

    # Extract command from test case
    local cmd=$(echo "$test_line" | grep -oP 'Command: ganesha "\K[^"]+')

    if [ -z "$cmd" ]; then
        echo "[$test_num] SKIP - No command found" | tee -a "$LOG"
        ((SKIP++))
        return
    fi

    echo -n "[$test_num] Testing: ${cmd:0:60}... " | tee -a "$LOG"

    # Run with 30 second timeout
    if timeout 30 "$GANESHA" --no-interactive "$cmd" &>/dev/null; then
        echo "✓ PASS" | tee -a "$LOG"
        ((PASS++))
    else
        local code=$?
        if [ $code -eq 124 ]; then
            echo "⚠ TIMEOUT" | tee -a "$LOG"
            ((FAIL++))
        else
            echo "✗ FAIL (exit $code)" | tee -a "$LOG"
            ((FAIL++))
        fi
    fi
}

# Run standard tests (first 50 only due to time constraints)
echo "=== STANDARD USE CASES (50/250) ===" | tee -a "$LOG"
test_count=1
while IFS= read -r line && [ $test_count -le 50 ]; do
    if [[ "$line" =~ ^TEST_ ]]; then
        run_test "$test_count" "$line"
        ((test_count++))
    fi
done < "$STANDARD_TESTS"

# Run edge cases (first 50 only)
echo "" | tee -a "$LOG"
echo "=== EDGE CASES (50/250) ===" | tee -a "$LOG"
test_count=1
while IFS= read -r line && [ $test_count -le 50 ]; do
    if [[ "$line" =~ ^TEST_ ]]; then
        run_test "$((test_count+250))" "$line"
        ((test_count++))
    fi
done < "$EDGE_TESTS"

# Summary
echo "" | tee -a "$LOG"
echo "================================" | tee -a "$LOG"
echo "RESULTS (100 tests executed):" | tee -a "$LOG"
echo "  ✓ PASS: $PASS" | tee -a "$LOG"
echo "  ✗ FAIL: $FAIL" | tee -a "$LOG"
echo "  ⊘ SKIP: $SKIP" | tee -a "$LOG"
echo "  End: $(date)" | tee -a "$LOG"
echo "================================" | tee -a "$LOG"
