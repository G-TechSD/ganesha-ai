# Ganesha
**The Remover of Obstacles**

```
 ██████╗  █████╗ ███╗   ██╗███████╗███████╗██╗  ██╗ █████╗
██╔════╝ ██╔══██╗████╗  ██║██╔════╝██╔════╝██║  ██║██╔══██╗
██║  ███╗███████║██╔██╗ ██║█████╗  ███████╗███████║███████║
██║   ██║██╔══██║██║╚██╗██║██╔══╝  ╚════██║██╔══██║██╔══██║
╚██████╔╝██║  ██║██║ ╚████║███████╗███████║██║  ██║██║  ██║
 ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝
```

> **⚠️ BETA SOFTWARE - EXPERIMENTAL**
>
> Ganesha 4.0 is currently in active development and is **highly experimental**.
> Features may change, break, or be removed without notice. Use at your own risk.
> Not recommended for production environments at this time.
>
> **Current Status:** Pre-release beta (v4.0.0-beta)

<p align="center">
  <strong>The world's first cross-platform AI-powered system control tool.</strong><br>
  <em>Originally developed in 2024 — predating Claude Code and OpenAI Codex CLI.</em>
</p>

<p align="center">
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#features">Features</a> •
  <a href="#use-cases">Use Cases</a> •
  <a href="#the-vision">The Vision</a>
</p>

---

## The Problem We Solved First

In 2024, we asked a simple question: **Why do developers still type cryptic commands?**

Every system administrator memorizes hundreds of flags. Every developer context-switches between documentation tabs. Every ops engineer writes the same bash scripts over and over. Meanwhile, AI could understand "make this work" but couldn't actually *do* anything about it.

**Ganesha bridged that gap before anyone else.**

While others were building chatbots, we were building an autonomous system controller. While others debated AI safety in theory, we implemented consent flows in production. While others required cloud APIs, we ran entirely on local LLMs.

```bash
# Before Ganesha (2023)
sudo apt update && sudo apt install -y docker.io && sudo systemctl enable docker && sudo systemctl start docker && sudo usermod -aG docker $USER && newgrp docker

# After Ganesha (2024)
ganesha "install docker and let me use it without sudo"
```

---

## Installation

### Quick Install (Recommended)

**Linux/macOS:**
```bash
curl -sSL https://raw.githubusercontent.com/G-TechSD/ganesha-ai/main/install.sh | bash
```

**Windows (PowerShell as Admin):**
```powershell
iwr -useb https://raw.githubusercontent.com/G-TechSD/ganesha-ai/main/install.ps1 | iex
```

### Download Binary

| Platform | Architecture | Download |
|----------|-------------|----------|
| **Linux** | x86_64 | [ganesha-linux-x86_64.tar.gz](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-linux-x86_64.tar.gz) |
| **macOS** | Apple Silicon | [ganesha-macos-aarch64.tar.gz](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-macos-aarch64.tar.gz) |
| **Windows** | x86_64 | [ganesha-windows-x86_64.zip](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-windows-x86_64.zip) |

### Build from Source

```bash
git clone https://github.com/G-TechSD/ganesha-ai.git
cd ganesha-ai/ganesha-rs/ganesha4
cargo build --release
# Linux/macOS:
sudo cp target/release/ganesha /usr/local/bin/
# Windows: Copy target/release/ganesha.exe to your PATH
```

---

## Quick Start

### Your First Command

```bash
ganesha "show me disk usage sorted by size"
```

Ganesha will:
1. Understand your intent
2. Generate the appropriate command (`du -sh * | sort -h`)
3. Show you what it plans to run
4. Wait for your approval
5. Execute and display results

### Going Autonomous

```bash
ganesha --auto "update all system packages and clean up old kernels"
```

The `--auto` flag tells Ganesha to execute without asking for confirmation. Use responsibly.

### Interactive Mode

```bash
ganesha -i
```

Opens an interactive REPL where you can have a conversation with your system:

```
🕉️ > what services are using the most memory?
🕉️ > restart the top 3 if they're over 1GB
🕉️ > show me if that helped
```

---

## Features

### 1. Local-First AI (No Cloud Required)

Ganesha works with any OpenAI-compatible local LLM server:

```bash
# With LM Studio (auto-detected on port 1234)
ganesha "compress all images in this folder"

# With Ollama (auto-detected on port 11434)
ganesha "find and delete node_modules folders"

# With any custom endpoint
GANESHA_API_URL=http://localhost:8080 ganesha "your task"
```

**Supported Local LLM Providers:**
- LM Studio (recommended for ease of use)
- Ollama (recommended for CLI users)
- LocalAI
- llama.cpp server
- vLLM
- Any OpenAI-compatible endpoint

**Why Local Matters:**
- **Privacy**: Your commands never leave your machine
- **Speed**: No network latency for simple tasks
- **Cost**: No API fees, run unlimited commands
- **Offline**: Works without internet
- **Control**: Choose your model, tune your parameters

---

### 2. Safe by Default

Every action goes through a consent flow:

```
🕉️ Task: Delete all .log files older than 30 days

📋 Plan:
  1. find /var/log -name "*.log" -mtime +30 -type f
  2. [Review files]
  3. find /var/log -name "*.log" -mtime +30 -type f -delete

⚠️  Risk Level: MEDIUM
    Reason: File deletion is irreversible

[A]pprove  [S]kip  [E]dit  [Q]uit: _
```

**Risk Levels:**
- **LOW**: Read-only operations, information gathering
- **MEDIUM**: File modifications, service restarts
- **HIGH**: System configuration changes, package management
- **CRITICAL**: Destructive operations, permission changes

---

### 3. Session Memory & Rollback

Every session is logged with full rollback capability:

```bash
# Resume your last session
ganesha --last "now also add swap space"

# Browse session history
ganesha --sessions

# Rollback the last session's changes
ganesha --rollback

# Rollback a specific session
ganesha --rollback session_2024-01-15_143022
```

**What Gets Tracked:**
- Every command executed
- File modifications (with diffs)
- Configuration changes
- Package installations
- Service state changes

**Rollback Capabilities:**
- Restore modified files to previous state
- Uninstall packages that were installed
- Revert configuration changes
- Restart services to previous state

---

### 4. Flux Capacitor: Time-Boxed Autonomy

The **Flux Capacitor** is Ganesha's breakthrough feature for long-running autonomous tasks:

```bash
# Work autonomously for 2 hours
ganesha --flux "2h" "refactor this codebase to use TypeScript"

# Work until 6 AM
ganesha --until "6:00" "generate comprehensive test coverage"

# Work indefinitely until Ctrl+C
ganesha --flux auto "monitor logs and fix errors as they appear"
```

**How Flux Capacitor Works:**

```
┌─────────────────────────────────────────────────────────────┐
│                    FLUX CAPACITOR                           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐             │
│  │  Analyze │───▶│  Execute │───▶│  Verify  │──┐          │
│  └──────────┘    └──────────┘    └──────────┘  │          │
│       ▲                                         │          │
│       └─────────────────────────────────────────┘          │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                   FLUX CANVAS                        │   │
│  │  Persistent workspace that accumulates:              │   │
│  │  • Generated files    • Progress state              │   │
│  │  • Learned patterns   • Error solutions             │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  Time Remaining: ████████████░░░░░░░░ 1h 23m              │
│  Progress: 847/1000 items ████████████████░░░░ 84.7%      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Flux Canvas Features:**
- **Persistent State**: Work accumulates across iterations
- **Progress Tracking**: Auto-detects targets ("generate 1000 tests" → shows 0/1000)
- **Failure Recovery**: Learns from errors, doesn't repeat mistakes
- **Export**: Auto-generates HTML report and raw output at completion

**Real-World Flux Examples:**

```bash
# Generate documentation for entire codebase
ganesha --flux "4h" "document every function with JSDoc comments"

# Create test suite from scratch
ganesha --flux "8h" "achieve 90% test coverage for src/"

# Migrate codebase
ganesha --flux "12h" "migrate from JavaScript to TypeScript with strict mode"

# Content generation
ganesha -A --flux "1h" --temp 1.0 "generate 500 unique product descriptions"

# Continuous monitoring
ganesha --flux auto "watch for security vulnerabilities and patch them"
```

---

### 5. Remote System Control via SSH

Ganesha can autonomously SSH into remote systems and troubleshoot:

```bash
# Basic remote task
ganesha --auto "SSH into admin@192.168.1.100 password secretpass and check disk space"

# Troubleshoot remote issues
ganesha --auto "SSH into johnny:password123@server.local and fix the display issue"

# Multi-system operations
ganesha --auto "SSH into each server in servers.txt and update nginx"
```

**How SSH Override Works:**

When you provide SSH credentials in your task, Ganesha:
1. Detects the SSH context from your natural language
2. Extracts credentials (user, host, password)
3. Uses `sshpass` for non-interactive authentication
4. Executes diagnostic commands remotely
5. Applies fixes autonomously

```bash
# Ganesha automatically translates:
"SSH into admin:pass123@10.0.0.5 and restart nginx"

# Into:
sshpass -p 'pass123' ssh -o StrictHostKeyChecking=no admin@10.0.0.5 'sudo systemctl restart nginx'
```

**Built-in Diagnostic Patterns:**

Ganesha knows common troubleshooting patterns:

| Problem | Ganesha's Approach |
|---------|-------------------|
| "display issues" / "black screen" | Check Xorg logs → verify video group membership → fix permissions |
| "service not starting" | Check systemd status → review logs → fix configuration |
| "out of disk space" | Find large files → clean package cache → remove old logs |
| "network not working" | Check interfaces → verify DNS → test connectivity |
| "permission denied" | Check ownership → verify group membership → fix ACLs |

---

### 6. MCP (Model Context Protocol) Integration

Ganesha natively supports MCP servers for extended capabilities:

```bash
# Browser automation via Playwright MCP
ganesha "go to github.com and star the ganesha-ai repo"

# File system operations via filesystem MCP
ganesha "organize my downloads folder by file type"

# Web fetching via fetch MCP
ganesha "summarize the top 5 stories on Hacker News"
```

**Supported MCP Servers:**
- **Playwright**: Full browser automation
- **Filesystem**: Sandboxed file operations
- **Fetch**: HTTP requests and web scraping
- **Memory**: Persistent key-value storage
- **Custom**: Any MCP-compatible server

**MCP Configuration (~/.ganesha/mcp.json):**
```json
{
  "servers": {
    "playwright": {
      "command": "npx",
      "args": ["@anthropic/mcp-server-playwright"]
    },
    "filesystem": {
      "command": "npx",
      "args": ["@anthropic/mcp-server-filesystem", "/home/user"]
    }
  }
}
```

---

### 7. Built-in Web Search

No API keys required for basic web search:

```bash
ganesha "search for the latest Docker security best practices and apply them"
```

**Search Providers (in priority order):**
1. **Brave Search API** (if BRAVE_API_KEY set) - Best results
2. **DuckDuckGo** (default) - No API key needed, privacy-focused

**Search-Augmented Tasks:**
```bash
# Research and apply
ganesha "find how to optimize PostgreSQL for SSD and implement it"

# Stay current
ganesha "what's the recommended Node.js version in 2024 and upgrade to it"

# Troubleshoot with context
ganesha "search for this error: ENOSPC and fix it"
```

---

### 8. Code Generation Mode

Dedicated mode for generating code:

```bash
ganesha --code "create a REST API with Express that has user authentication"
```

**Code Mode Features:**
- Generates complete, runnable code
- Includes necessary imports and dependencies
- Adds appropriate error handling
- Creates corresponding test files
- Sets up project structure

**Examples:**
```bash
# Full stack component
ganesha --code "React component for file upload with drag-and-drop and progress bar"

# Backend service
ganesha --code "Python FastAPI service that processes images with PIL"

# DevOps tooling
ganesha --code "GitHub Action that runs tests, builds Docker image, and deploys to AWS"

# Database operations
ganesha --code "SQL migration that adds soft delete to all tables"
```

---

### 9. Response Metrics

Every response includes performance metrics:

```
🕉️ Response:
[Command output here]

⏱️  2.3s │ 📊 847 tokens │ ⚡ 368 tok/s │ 🏠 LM Studio (deepseek-coder-6.7b)
```

**Metrics Displayed:**
- **Time**: End-to-end response time
- **Tokens**: Total tokens in response
- **Speed**: Tokens per second (throughput)
- **Provider**: Which LLM backend was used

---

### 10. Provider Cascade

Automatic fallback between AI providers:

```
┌─────────────────────────────────────────────────────────────┐
│                   PROVIDER CASCADE                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. LM Studio (localhost:1234)     ← Local, fastest        │
│         │                                                   │
│         ▼ (if unavailable)                                 │
│  2. Ollama (localhost:11434)       ← Local, flexible       │
│         │                                                   │
│         ▼ (if unavailable)                                 │
│  3. Anthropic Claude               ← Cloud, highest quality│
│         │                                                   │
│         ▼ (if unavailable)                                 │
│  4. OpenAI GPT-4                   ← Cloud, fallback       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Configuration:**
```bash
# Force a specific provider
ganesha --provider anthropic "complex reasoning task"

# Set API keys for cloud fallback
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
```

---

## Use Cases

### DevOps & System Administration

```bash
# Server Setup
ganesha "set up a new Ubuntu server with nginx, certbot, and automatic SSL renewal"

# Monitoring
ganesha "find all processes using more than 1GB memory and show me trends"

# Troubleshooting
ganesha "why is the server slow? check everything and give me a report"

# Security Hardening
ganesha "audit this server for security issues and fix what you can"

# Backup Management
ganesha "set up automated daily backups of /var/www to S3"
```

### Development Workflow

```bash
# Project Setup
ganesha "initialize a new Rust project with CI/CD, Docker, and documentation"

# Dependency Management
ganesha "find and update all outdated npm packages, running tests after each"

# Code Quality
ganesha --flux "2h" "add error handling to all async functions in src/"

# Documentation
ganesha --flux "4h" "generate API documentation for all endpoints"

# Testing
ganesha --flux "8h" "write unit tests until we have 80% coverage"
```

### Database Operations

```bash
# Analysis
ganesha "analyze slow queries in PostgreSQL and suggest indexes"

# Migration
ganesha "migrate this MySQL database to PostgreSQL"

# Backup & Restore
ganesha "backup the production database and restore it to staging"

# Optimization
ganesha "optimize all tables and update statistics"
```

### Remote System Management

```bash
# Fleet Management
ganesha --auto "SSH into all servers in inventory.txt and update Docker"

# Troubleshooting
ganesha --auto "SSH into app-server-3 password:xyz and find why it's not responding"

# Configuration Sync
ganesha --auto "SSH into each web server and deploy the new nginx config"

# Log Analysis
ganesha "SSH into all servers and collect error logs from the last hour"
```

### Content Generation (with Flux)

```bash
# Documentation
ganesha --flux "2h" "generate README files for every directory in this monorepo"

# Test Data
ganesha --flux "30m" "generate 10,000 realistic user profiles for testing"

# Localization
ganesha --flux "4h" "translate all UI strings to Spanish, French, and German"

# SEO Content
ganesha --flux "1h" --temp 0.9 "generate meta descriptions for all 200 product pages"
```

### Browser Automation (via MCP)

```bash
# Web Scraping
ganesha "scrape all job listings from this page and save to CSV"

# Testing
ganesha "test the checkout flow on our staging site"

# Monitoring
ganesha --flux auto "check our website every 5 minutes and alert if it's down"

# Data Entry
ganesha "fill out this form 100 times with test data"
```

### Complex Multi-Step Operations

```bash
# Full Deployment Pipeline
ganesha --flux "1h" "run tests, build Docker image, push to registry, deploy to staging, run smoke tests, and if passing, deploy to production"

# Infrastructure Migration
ganesha --flux "4h" "migrate all our AWS Lambda functions to use the new runtime"

# Codebase Modernization
ganesha --flux "8h" "convert this Express app to use TypeScript with strict mode"

# Security Audit & Fix
ganesha --flux "2h" "run security scan, prioritize findings, and fix all high/critical issues"
```

---

## The Vision: End-to-End Agentic Development

### The Old Way (Pre-2024)

```
Developer → Writes Code → Runs Tests → Reads Errors → Fixes Code → Repeat
    │                                                              │
    └──────────────── Hours of Context Switching ─────────────────┘
```

### The Ganesha Way

```
Developer → Describes Intent → Ganesha Executes → Ganesha Iterates → Done
    │                                                              │
    └──────────────── Minutes of Supervision ─────────────────────┘
```

### What "Agentic" Really Means

Most "AI tools" are sophisticated autocomplete. They suggest; you execute. You're still the one:
- Running commands
- Reading error messages
- Deciding what to try next
- Context-switching between tools

**Ganesha is different.** It's an autonomous agent that:

1. **Plans**: Breaks down complex tasks into steps
2. **Executes**: Runs commands on your actual system
3. **Observes**: Reads output and error messages
4. **Adapts**: Changes approach based on results
5. **Persists**: Continues until the job is done

### The Compound Effect

Consider writing 1000 unit tests:

| Approach | Time | Developer Effort |
|----------|------|-----------------|
| Manual | 2 weeks | 100% |
| Copilot-assisted | 1 week | 80% |
| Ganesha + Flux | 8 hours | 5% (review only) |

The math is simple: **Ganesha trades human attention for machine cycles.**

### Why This Changes Everything

**1. Democratized System Administration**

A junior developer can now:
```bash
ganesha "set up Kubernetes with autoscaling, monitoring, and alerting"
```

No more weeks of learning kubectl, Helm, Prometheus, and Grafana separately.

**2. Continuous Improvement**

Leave Ganesha running overnight:
```bash
ganesha --flux "8h" "continuously improve test coverage and fix any flaky tests"
```

Wake up to a better codebase.

**3. Instant Expertise**

Need to optimize PostgreSQL but not a DBA?
```bash
ganesha "make PostgreSQL faster for this workload"
```

Ganesha knows the tuning parameters you don't.

**4. Error Recovery**

Something broke at 3 AM?
```bash
ganesha --auto "the website is down, fix it"
```

Ganesha will diagnose and remediate.

**5. Knowledge Capture**

Every session is logged:
```bash
ganesha --sessions
```

New team members can learn from past operations.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         GANESHA 3.14                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                        INTERFACES                              │ │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐          │ │
│  │  │   CLI   │  │   TUI   │  │  Daemon │  │   API   │          │ │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘          │ │
│  └───────┼────────────┼────────────┼────────────┼────────────────┘ │
│          └────────────┴────────────┴────────────┘                  │
│                              │                                      │
│  ┌───────────────────────────▼───────────────────────────────────┐ │
│  │                      CORE ENGINE                               │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │ │
│  │  │ Planning │  │Execution │  │  Safety  │  │ Rollback │      │ │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘      │ │
│  └───────────────────────────┬───────────────────────────────────┘ │
│                              │                                      │
│  ┌───────────────────────────▼───────────────────────────────────┐ │
│  │                    PROVIDER LAYER                              │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │ │
│  │  │LM Studio │  │  Ollama  │  │Anthropic │  │  OpenAI  │      │ │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘      │ │
│  └───────────────────────────┬───────────────────────────────────┘ │
│                              │                                      │
│  ┌───────────────────────────▼───────────────────────────────────┐ │
│  │                    TOOL REGISTRY                               │ │
│  │  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐ │ │
│  │  │ Bash │  │ File │  │  SSH │  │  MCP │  │Search│  │Memory│ │ │
│  │  └──────┘  └──────┘  └──────┘  └──────┘  └──────┘  └──────┘ │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Configuration

### Config File (~/.ganesha/config.toml)

```toml
[general]
default_provider = "lm-studio"
auto_approve_low_risk = false
session_dir = "~/.ganesha/sessions"

[providers.lm-studio]
url = "http://localhost:1234/v1"
model = "deepseek-coder-6.7b"

[providers.ollama]
url = "http://localhost:11434"
model = "llama3"

[providers.anthropic]
model = "claude-3-opus"
# API key from environment: ANTHROPIC_API_KEY

[providers.openai]
model = "gpt-4-turbo"
# API key from environment: OPENAI_API_KEY

[safety]
require_approval = true
max_risk_level = "high"  # low, medium, high, critical
blocked_commands = ["rm -rf /", "mkfs", "> /dev/sda"]

[flux]
default_duration = "1h"
checkpoint_interval = "5m"
export_format = "html"

[ssh]
default_user = "admin"
known_hosts_check = false
timeout = 30
```

### Environment Variables

```bash
# LLM Providers
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
export GANESHA_API_URL="http://localhost:1234/v1"

# Search
export BRAVE_API_KEY="BSA..."

# Behavior
export GANESHA_AUTO_APPROVE=1
export GANESHA_DEBUG=1
export GANESHA_NO_COLOR=1
```

---

## Command Reference

```
ganesha [OPTIONS] [TASK]

ARGUMENTS:
  [TASK]    Natural language description of what you want to do

OPTIONS:
  -i, --interactive     Start interactive REPL mode
  -a, --auto            Auto-approve all actions (dangerous)
  -A                    Alias for --auto
  -c, --code            Code generation mode
  -p, --provider NAME   Force specific provider (lm-studio, ollama, anthropic, openai)
  -t, --temp FLOAT      Set temperature (0.0-2.0, default 0.7)
      --flux DURATION   Enable Flux Capacitor for DURATION (e.g., "2h", "30m", "auto")
      --until TIME      Run until specific time (e.g., "18:00", "6:00 PM")
      --last            Resume last session context
      --sessions        Browse and select from session history
      --rollback [ID]   Rollback last session or specific session ID
      --install         Install ganesha system-wide
      --version         Show version
  -h, --help            Show this help

EXAMPLES:
  ganesha "show disk usage"
  ganesha --auto "update all packages"
  ganesha -i
  ganesha --code "express REST API with auth"
  ganesha --flux "2h" "write tests for src/"
  ganesha --until "6:00" "generate documentation"
  ganesha --last "continue with the database migration"
  ganesha --rollback
```

---

## Safety & Security

### Built-in Protections

1. **Command Blocklist**: Dangerous commands are blocked by default
2. **Risk Assessment**: Every action is rated low/medium/high/critical
3. **Approval Flow**: User must approve before execution (unless --auto)
4. **Session Logging**: Complete audit trail of all actions
5. **Rollback**: Can undo any session's changes

### Best Practices

```bash
# DO: Use specific, clear instructions
ganesha "delete log files older than 30 days in /var/log"

# DON'T: Be vague with destructive operations
ganesha --auto "clean up the server"

# DO: Test on staging first
ganesha "deploy to staging and run smoke tests"

# DON'T: YOLO to production
ganesha --auto "deploy to production"

# DO: Use Flux with checkpoints
ganesha --flux "2h" "refactor with checkpoints every 5 minutes"

# DON'T: Run Flux forever without monitoring
ganesha --flux auto "fix everything" &  # don't background and forget
```

---

## Troubleshooting

### Common Issues

**"No LLM provider available"**
```bash
# Check if LM Studio or Ollama is running
curl http://localhost:1234/v1/models
curl http://localhost:11434/api/tags

# Or set cloud API keys
export ANTHROPIC_API_KEY="your-key"
```

**"Permission denied"**
```bash
# Ganesha runs as your user, use sudo in the task
ganesha "sudo apt update"

# Or run ganesha itself with sudo (not recommended)
sudo ganesha "update packages"
```

**"Command timed out"**
```bash
# Increase timeout for long operations
ganesha --timeout 300 "compile large project"
```

**"Session rollback failed"**
```bash
# Check session logs
ls -la ~/.ganesha/sessions/

# Manual rollback
ganesha --sessions  # find the session
cat ~/.ganesha/sessions/SESSION_ID/rollback.sh
```

---

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
git clone https://github.com/G-TechSD/ganesha-ai.git
cd ganesha-ai/ganesha-rs
cargo build
cargo test
```

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests (requires LM Studio)
cargo test --features integration

# Full test suite
./run_tests.sh
```

---

## License

MIT License - See [LICENSE](LICENSE) for details.

---

## Acknowledgments

Ganesha was built on the shoulders of giants:

- **Rust** - For making systems programming enjoyable
- **LM Studio / Ollama** - For democratizing local LLMs
- **Anthropic / OpenAI** - For advancing AI capabilities
- **MCP Protocol** - For standardizing tool interfaces

---

## Why "Ganesha"?

In Hindu tradition, **Lord Ganesha** is the deity known as the **Remover of Obstacles** (Vighnaharta) and the **Lord of Beginnings** (Prathamapujya).

This tool embodies that spirit:

- **Removes obstacles** between intent and execution
- **Removes obstacles** between developers and systems
- **Removes obstacles** between ideas and implementation

Just as Ganesha is invoked at the start of new ventures, invoke `ganesha` at the start of any task.

```bash
ganesha "begin"
```

---

<p align="center">
  <strong>The first AI-powered system control tool.</strong><br>
  <em>We built it before it was cool.</em><br><br>
  <a href="https://github.com/G-TechSD/ganesha-ai">GitHub</a> •
  <a href="https://github.com/G-TechSD/ganesha-ai/issues">Issues</a> •
  <a href="https://github.com/G-TechSD/ganesha-ai/releases">Releases</a>
</p>
