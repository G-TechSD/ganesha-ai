# CLI Commands Reference

## Command Structure

```
ganesha [OPTIONS] [TASK]
```

All commands follow a consistent pattern. Options modify behavior, and the task is what you want Ganesha to do.

---

## Basic Usage

### Direct Commands

```bash
# Ask questions
ganesha "what's in my downloads folder"
ganesha "explain this error" < error.log

# System tasks
ganesha "install nginx"
ganesha "update all packages"

# Code tasks
ganesha "create a python script that converts CSV to JSON"
ganesha "fix the bug in main.rs"

# Research
ganesha "search for the latest rust async patterns"
ganesha "what's new in python 3.12"
```

### Interactive Modes

```bash
# TUI - Full terminal interface
ganesha --tui
ganesha -t

# Voice - Conversational mode
ganesha --voice
ganesha -v

# REPL - Simple interactive
ganesha --interactive
ganesha -i
```

---

## Risk Level Options

| Flag | Level | Description |
|------|-------|-------------|
| `--safe` | Safe | Read-only, no system changes |
| (default) | Normal | Asks before risky operations |
| `--trusted` | Trusted | Auto-approves routine tasks |
| `-A`, `--yolo` | YOLO | Auto-approves everything |

```bash
# Read-only mode (safest)
ganesha --safe "analyze this codebase"

# Normal (default) - asks for confirmation
ganesha "install docker"

# Trusted - auto-approves routine operations
ganesha --trusted "update all packages and restart services"

# YOLO - no confirmations (use carefully!)
ganesha -A "refactor everything"
```

---

## Execution Context

### Live vs Sandbox

```bash
# Live (default) - Direct system access
ganesha "modify /etc/hosts"

# Sandbox - Isolated container
ganesha --sandbox "test this dangerous script"
```

### Working Directory

```bash
# Use specific directory
ganesha --dir ~/projects/myapp "run tests"

# Show current directory context
ganesha "where am I working"
```

---

## Session Management

### Resume & Persistence

```bash
# Continue last session
ganesha --resume

# Named sessions
ganesha --session myproject "continue working"

# List sessions
ganesha --sessions

# Clear current session
ganesha --new-session
```

### History & Rollback

```bash
# View history
ganesha --history

# Rollback changes
ganesha --rollback           # List available rollbacks
ganesha --rollback abc123    # Rollback specific session
```

---

## Time-Boxed Execution (Flux Capacitor)

```bash
# Work for specified duration
ganesha --flux 1h "optimize this codebase"
ganesha --flux 30m "write tests for all functions"

# Work until specific time
ganesha --until 5pm "finish this feature"
ganesha --until 17:30 "complete the documentation"

# Auto-extend if making progress
ganesha --flux auto "build this application"
```

---

## Verification Mode (Wiggum)

```bash
# Enable verification loop
ganesha --wiggum "set up kubernetes cluster"

# Wiggum verifies each step before proceeding
# Automatically retries on failure
```

---

## Provider Options

```bash
# Use specific provider
ganesha --provider openrouter "complex task"
ganesha --provider local "quick question"

# Use specific model
ganesha --model gpt-4o "analyze this"
ganesha --model claude-3.5-sonnet "write code"

# List available
ganesha --providers
ganesha --models
```

---

## MCP Server Commands

```bash
# List servers
ganesha mcp list

# Install server
ganesha mcp install playwright
ganesha mcp install github

# Enable/disable (hot load)
ganesha mcp enable playwright
ganesha mcp disable github

# Configure
ganesha mcp config github    # Set API key etc

# Remove
ganesha mcp remove playwright
```

---

## Output Options

```bash
# Quiet - minimal output
ganesha -q "task"

# Verbose - detailed output
ganesha --verbose "task"

# Bare - raw output only (for scripting)
ganesha --bare "get system info"

# JSON output
ganesha --json "list files"
```

---

## Configuration Commands

```bash
# Interactive setup
ganesha --configure

# Show current config
ganesha --config show

# Set specific values
ganesha --config set provider.default openrouter
ganesha --config set risk.default trusted

# Reset
ganesha --config reset
```

---

## Piping & Scripting

```bash
# Pipe input
cat error.log | ganesha "explain this error"
git diff | ganesha "review these changes"

# Pipe output
ganesha --bare "generate dockerfile" > Dockerfile

# Chain commands
ganesha "analyze code" && ganesha "generate tests"

# Use in scripts
#!/bin/bash
SUMMARY=$(ganesha --bare "summarize $(cat report.txt)")
echo "Summary: $SUMMARY"
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GANESHA_PROVIDER` | Default provider |
| `GANESHA_MODEL` | Default model |
| `GANESHA_RISK` | Default risk level |
| `GANESHA_DEBUG` | Enable debug output |
| `GANESHA_CONFIG` | Config file path |

---

## Keyboard Shortcuts (TUI/Interactive)

| Key | Action |
|-----|--------|
| `Enter` | Submit message |
| `Ctrl+C` | Cancel current operation |
| `Ctrl+D` | Exit |
| `Ctrl+L` | Clear screen |
| `Ctrl+R` | Search history |
| `Tab` | Auto-complete |
| `↑/↓` | Navigate history |
| `Esc` | Cancel input |

---

## Examples by Category

### System Administration

```bash
ganesha "check disk usage and clean if above 80%"
ganesha "set up SSH key authentication"
ganesha "configure firewall to allow ports 80, 443"
ganesha "troubleshoot why nginx isn't starting"
```

### Development

```bash
ganesha "create a new rust project with tokio"
ganesha "add error handling to all functions in src/"
ganesha "write unit tests for the user module"
ganesha "optimize database queries in this file"
```

### Git Operations

```bash
ganesha "create a branch for this feature"
ganesha "squash the last 3 commits"
ganesha "write a commit message for my changes"
ganesha "help me resolve these merge conflicts"
```

### Research

```bash
ganesha "find the best practices for rust error handling"
ganesha "compare react vs vue vs svelte"
ganesha "what's the latest on AI agents"
```

---

## See Also

- [Risk Levels](risk-levels.md)
- [Voice Interface](voice.md)
- [MCP Servers](mcp.md)
- [Flux Capacitor](flux.md)
