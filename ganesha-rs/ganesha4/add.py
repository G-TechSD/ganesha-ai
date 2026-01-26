#!/usr/bin/env python3
"""
Simple addition script.
Usage: python3 add.py <num1> <num2>
"""

import sys

def main():
    if len(sys.argv) != 3:
        print("Usage: python3 add.py <num1> <num2>")
        sys.exit(1)
    try:
        a = float(sys.argv[1])
        b = float(sys.argv[2])
    except ValueError:
        print("Both arguments must be numbers.")
        sys.exit(1)

    result = a + b
    print(f"{a} + {b} = {result}")

if __name__ == "__main__":
    main()
