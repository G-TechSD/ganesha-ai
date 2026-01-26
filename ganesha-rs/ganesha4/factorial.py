#!/usr/bin/env python3

def factorial(n: int) -> int:
    """Return the factorial of n."""
    if n < 0:
        raise ValueError("Negative input not allowed")
    result = 1
    for i in range(2, n + 1):
        result *= i
    return result

if __name__ == "__main__":
    import sys
    try:
        num = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    except ValueError:
        print("Please provide a valid integer.")
        sys.exit(1)
    try:
        print(f"Factorial of {num} is {factorial(num)}")
    except Exception as e:
        print(e)