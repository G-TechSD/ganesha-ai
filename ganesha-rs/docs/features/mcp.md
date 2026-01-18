# MCP Servers

## Overview

Model Context Protocol (MCP) servers extend Ganesha's capabilities with specialized tools. Ganesha supports hot-loading MCP servers - they start when needed and unload when idle.

---

## Quick Start

```bash
# List available servers
ganesha mcp list

# Install a server
ganesha mcp install playwright

# Use it immediately
ganesha "take a screenshot of google.com"
# → Playwright server auto-loads and captures
```

---

## Built-in Servers

These servers come with Ganesha:

| Server | Tools | Description |
|--------|-------|-------------|
| `ganesha:search` | `web_search` | DuckDuckGo/SearxNG search |
| `ganesha:fetch` | `fetch_url`, `extract_text` | HTTP content extraction |
| `ganesha:filesystem` | `read`, `write`, `list` | Sandboxed file access |
| `ganesha:shell` | `execute` | Command execution |

---

## Installable Servers

### Browser Automation

```bash
# Playwright - Full browser control
ganesha mcp install playwright

# Available tools:
# - browser_navigate, browser_click, browser_type
# - browser_screenshot, browser_snapshot
# - browser_execute_js
```

### Developer Tools

```bash
# GitHub
ganesha mcp install github
# Tools: create_issue, list_prs, merge_pr, etc.

# GitLab
ganesha mcp install gitlab

# Linear
ganesha mcp install linear

# Jira
ganesha mcp install jira
```

### Communication

```bash
# Slack
ganesha mcp install slack
# Tools: send_message, list_channels, search

# Discord
ganesha mcp install discord

# Email (IMAP/SMTP)
ganesha mcp install email
```

### Cloud & DevOps

```bash
# Kubernetes
ganesha mcp install kubernetes
# Tools: get_pods, describe, logs, apply

# AWS
ganesha mcp install aws

# Docker
ganesha mcp install docker
```

### Data & Files

```bash
# PostgreSQL
ganesha mcp install postgres
# Tools: query, insert, update, schema

# SQLite
ganesha mcp install sqlite

# Google Drive
ganesha mcp install gdrive

# S3
ganesha mcp install s3
```

---

## Server Management

### Install

```bash
# From registry
ganesha mcp install <name>

# From URL
ganesha mcp install https://github.com/user/mcp-server

# From npm
ganesha mcp install npm:@company/mcp-server

# From local path
ganesha mcp install ./my-mcp-server
```

### Enable/Disable (Hot Loading)

```bash
# Enable (loads server)
ganesha mcp enable github

# Disable (unloads server)
ganesha mcp disable github

# Servers auto-enable based on task context
```

### Configure

```bash
# Interactive configuration
ganesha mcp config github
# → Prompts for API token, org, etc.

# Direct setting
ganesha mcp config github token=ghp_xxx
ganesha mcp config github org=mycompany
```

### Remove

```bash
ganesha mcp remove github
```

### Update

```bash
# Update specific
ganesha mcp update playwright

# Update all
ganesha mcp update --all
```

---

## Auto-Detection

Ganesha automatically loads relevant servers based on your task:

| Task Pattern | Server Loaded |
|--------------|---------------|
| "search for..." | ganesha:search |
| "what's on [website]" | fetch or playwright |
| "create github issue" | github |
| "check kubernetes pods" | kubernetes |
| "send slack message" | slack |

```bash
# Disable auto-loading
ganesha --config set mcp.auto_load false

# Explicit server use
ganesha --mcp github "create an issue for this bug"
```

---

## Credential Management

### Interactive Setup

```bash
ganesha mcp config github
# ┌─────────────────────────────────────────┐
# │  GitHub MCP Server Configuration        │
# │                                         │
# │  API Token: ********                    │
# │  (Enter your GitHub personal access     │
# │   token with repo scope)                │
# │                                         │
# │  Organization (optional): mycompany     │
# │                                         │
# │  [Save] [Cancel]                        │
# └─────────────────────────────────────────┘
```

### Storage

Credentials are stored securely:

| Platform | Storage |
|----------|---------|
| macOS | Keychain |
| Linux | Secret Service / libsecret |
| Windows | Credential Manager |

```bash
# View stored credentials (names only)
ganesha mcp credentials

# Delete credential
ganesha mcp credentials delete github
```

### Environment Variables

Alternative to stored credentials:

```bash
export GITHUB_TOKEN=ghp_xxx
export SLACK_TOKEN=xoxb_xxx
ganesha "create github issue"  # Uses env var
```

---

## Global vs Project MCP

### Global Configuration

```
~/.ganesha/mcp/
├── servers.json       # Installed servers
├── config/            # Server configurations
│   ├── github.json
│   ├── slack.json
│   └── ...
└── cache/             # Downloaded binaries
```

### Project Configuration

```
myproject/
└── .ganesha/
    └── mcp.toml       # Project-specific MCP config
```

```toml
# .ganesha/mcp.toml
[servers]
# Enable specific servers for this project
enabled = ["github", "postgres"]

# Project-specific config overrides
[servers.postgres]
database = "myproject_dev"
```

---

## Hot Loading Details

### Memory Management

```toml
# ~/.ganesha/config.toml
[mcp]
# Unload after idle (seconds)
idle_timeout = 300

# Max concurrent servers
max_loaded = 5

# Preload these always
preload = ["ganesha:search", "ganesha:fetch"]
```

### Load Events

```
Task: "check my github PRs"
  → Detected: github
  → Loading github server...
  → Server ready (1.2s)
  → Executing: list_prs
  → Result returned
  → Server remains loaded (idle timer starts)

[After 5 minutes of no github tasks]
  → Unloading github server (idle timeout)
```

---

## Creating Custom Servers

### Basic Template

```typescript
// my-server/index.ts
import { Server } from "@modelcontextprotocol/sdk";

const server = new Server({
  name: "my-server",
  version: "1.0.0",
});

server.addTool({
  name: "my_tool",
  description: "Does something useful",
  parameters: {
    type: "object",
    properties: {
      input: { type: "string" }
    }
  },
  handler: async ({ input }) => {
    return { result: `Processed: ${input}` };
  }
});

server.start();
```

### Register with Ganesha

```bash
# Install local server
ganesha mcp install ./my-server

# Or publish to npm and install
npm publish
ganesha mcp install npm:my-server
```

---

## Troubleshooting

### Server Won't Start

```bash
# Check server status
ganesha mcp status github

# View logs
ganesha mcp logs github

# Reinstall
ganesha mcp remove github && ganesha mcp install github
```

### Credential Issues

```bash
# Re-authenticate
ganesha mcp config github --reset

# Check stored credentials
ganesha mcp credentials
```

### Performance

```bash
# Check loaded servers
ganesha mcp loaded

# Force unload
ganesha mcp disable --all
```

---

## Server Reference

Full documentation for each official server:

- [Playwright](mcp-servers/playwright.md)
- [GitHub](mcp-servers/github.md)
- [Slack](mcp-servers/slack.md)
- [Kubernetes](mcp-servers/kubernetes.md)
- [PostgreSQL](mcp-servers/postgres.md)

---

## See Also

- [CLI Commands](cli.md)
- [Configuration](../getting-started/configuration.md)
- [Extending Ganesha](../guides/mcp-guide.md)
