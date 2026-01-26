#!/usr/bin/env bash

# Display the current git status
echo "=== Git Status ==="
git status

# Show the last 3 commits
echo ""
echo "=== Last 3 Commits ==="
git log -n 3 --oneline
