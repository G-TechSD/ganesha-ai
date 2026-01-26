#!/usr/bin/env bash

# Show disk usage of the current directory (and its subdirectories)
# sorted by size in human‑readable format.
du -h --max-depth=1 . | sort -hr
