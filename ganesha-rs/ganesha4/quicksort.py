#!/usr/bin/env python3
"""
quicksort.py

A simple implementation of the QuickSort algorithm.
"""

def quicksort(arr):
    """Return a new list containing the elements from arr sorted in ascending order."""
    if len(arr) <= 1:
        return arr[:]
    pivot = arr[len(arr) // 2]
    left   = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right  = [x for x in arr if x > pivot]
    return quicksort(left) + middle + quicksort(right)

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Sort a list of numbers using QuickSort.")
    parser.add_argument('numbers', metavar='N', type=float, nargs='+',
                        help='Numbers to sort')
    args = parser.parse_args()
    sorted_numbers = quicksort(args.numbers)
    print("Sorted:", sorted_numbers)

if __name__ == "__main__":
    main()
