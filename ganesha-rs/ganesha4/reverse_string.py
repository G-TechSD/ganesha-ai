#!/usr/bin/env python3
"""
reverse_string.py - Reverse a string provided as a command line argument or via stdin.
Usage:
    python reverse_string.py "hello"
    echo "world" | python reverse_string.py
"""
import sys

def reverse(s: str) -> str:
    return s[::-1]

if __name__ == "__main__":
    if len(sys.argv) > 1:
        # First argument is the string to reverse
        input_str = sys.argv[1]
    else:
        # Read from stdin
        input_str = sys.stdin.read().rstrip("\n")
    print(reverse(input_str))
