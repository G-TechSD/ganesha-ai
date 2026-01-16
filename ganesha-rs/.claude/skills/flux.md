# /flux - Flux Capacitor Continuous Improvement Loop

When the user runs /flux, enter a continuous improvement loop that doesn't stop until a specified time.

## Usage
/flux

## Parameters
Ask the user:
1. What to improve (codebase, tests, docs, features, etc.)
2. How long to run (duration like "1h" or end time like "8:30 PM")

## Loop Behavior
- Analyze → Find Issues → Fix → Test → Commit → Repeat
- Check system time every iteration
- Never finish early (that's like leaving work at noon!)
- Use multiple agents if needed for large tasks
- Track improvements made
- Report progress periodically

## Tools Available
- Read, Edit, Write for code changes
- Bash for running tests
- Grep/Glob for searching
- Task for spawning sub-agents

## Example
User: /flux
Assistant: What should I continuously improve?
User: The Ganesha codebase - find and fix bugs
Assistant: How long should I run for?
User: Until 8:30 PM
Assistant: Starting Flux Capacitor loop... will run until 20:30
[continuous improvement begins]
