#!/usr/bin/env python3
"""
read_json_keys.py

Reads a JSON file specified as the first command-line argument and prints all top‑level keys.
"""

import json
import sys
from pathlib import Path


def main():
    if len(sys.argv) != 2:
        print(f"Usage: {Path(sys.argv[0]).name} <json_file>")
        sys.exit(1)

    json_path = Path(sys.argv[1])
    if not json_path.is_file():
        print(f"Error: File '{json_path}' does not exist.")
        sys.exit(1)

    try:
        with json_path.open('r', encoding='utf-8') as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON: {e}")
        sys.exit(1)

    if isinstance(data, dict):
        for key in data.keys():
            print(key)
    else:
        print("JSON root is not an object; no keys to display.")


if __name__ == "__main__":
    main()
