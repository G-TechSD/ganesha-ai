#!/bin/bash

# Check if a file is provided as an argument
if [ -z "$1" ]; then
  echo "Usage: $0 <filename>"
  exit 1
fi

# Check if the file exists
if [ ! -f "$1" ]; then
  echo "Error: File '$1' not found."
  exit 1
fi

# Count words, lines, and characters
word_count=$(wc -w "$1" | awk '{print $1}')
line_count=$(wc -l "$1" | awk '{print $1}')
char_count=$(wc -c "$1" | awk '{print $1}')

# Print the results
echo "Word count: $word_count"
echo "Line count: $line_count"
echo "Character count: $char_count"
