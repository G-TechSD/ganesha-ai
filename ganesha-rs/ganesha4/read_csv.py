#!/usr/bin/env python3
"""
Read a CSV file and print the first 5 rows.
Usage: python read_csv.py <path_to_csv>
"""

import csv
import sys

def main():
    if len(sys.argv) != 2:
        print("Usage: python read_csv.py <csv_file>")
        sys.exit(1)

    csv_path = sys.argv[1]

    try:
        with open(csv_path, newline='', encoding='utf-8') as f:
            reader = csv.reader(f)
            for i, row in enumerate(reader):
                if i >= 5:
                    break
                print(row)
    except FileNotFoundError:
        print(f"File not found: {csv_path}")
    except Exception as e:
        print(f"Error reading CSV: {e}")

if __name__ == "__main__":
    main()
