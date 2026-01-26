#!/bin/bash

# Configuration
LOG_FILES="/var/log/syslog /var/log/auth.log"  # Space-separated list of log files to monitor
KEYWORD="error"                             # Keyword to search for
SLEEP_INTERVAL=60                             # How often to check logs (in seconds)

# Function to check for the keyword in a log file
check_log_file() {
  local log_file="$1"
  local keyword="$2"

  # Get the number of lines containing the keyword since the last check
  lines=$(tail -n 100 "$log_file" | grep -c "$keyword")

  if [ "$lines" -gt 0 ]; then
    echo "Found $lines occurrences of '$keyword' in $log_file"
    tail -n 100 "$log_file" | grep "$keyword"
  fi
}

# Main loop
while true; do
  for log_file in $LOG_FILES; do
    if [ -f "$log_file" ]; then
      check_log_file "$log_file" "$KEYWORD"
    else
      echo "Log file not found: $log_file"
    fi
  done

  sleep $SLEEP_INTERVAL
done
