#!/usr/bin/env python3
"""
FizzBuzz implementation.

Print numbers from 1 to 100 with the following rules:
- If a number is divisible by 3, print "Fizz".
- If it is divisible by 5, print "Buzz".
- If it is divisible by both 3 and 5, print "FizzBuzz".
- Otherwise, print the number itself.
"""

def fizzbuzz(limit=100):
    for i in range(1, limit + 1):
        output = ""
        if i % 3 == 0:
            output += "Fizz"
        if i % 5 == 0:
            output += "Buzz"
        print(output or i)

if __name__ == "__main__":
    fizzbuzz()
