# Ganesha 4.0 Documentation

> **The Obstacle Remover** - A lightning-fast, local-first AI assistant

---

## Quick Navigation

| Section | Description |
|---------|-------------|
| [Getting Started](getting-started/) | Installation, first steps, basic usage |
| [Features](features/) | Detailed feature documentation |
| [Architecture](architecture/) | System design and internals |
| [API Reference](api/) | Programmatic interface docs |
| [Guides](guides/) | Tutorials and how-tos |

---

## What is Ganesha?

Ganesha is an AI-powered system assistant that combines:

- **Terminal CLI** - Fast, scriptable command interface
- **TUI Mode** - Rich terminal user interface
- **Desktop App** - Non-intrusive companion with push-to-talk
- **Voice Interface** - Conversational interaction with personality
- **Visual Computer Use** - Screen reading and GUI automation

### Core Principles

1. **Local First** - Works offline with local LLMs, cloud is optional
2. **Privacy Respecting** - No telemetry, no ads, no surveillance
3. **Lightning Fast** - Optimized for instant responses
4. **Human-Readable** - Clear risk levels, understandable commands
5. **Undo Everything** - Rollback any changes Ganesha makes

---

## Quick Start

```bash
# Install
curl -fsSL https://ganesha.dev/install.sh | bash

# Configure (interactive)
ganesha --configure

# Basic usage
ganesha "what files are in my downloads folder"
ganesha "install docker and set it up"
ganesha "help me fix this error" < error.log

# Voice mode
ganesha --voice

# TUI mode
ganesha --tui

# Desktop app
ganesha-desktop
```

---

## Documentation Index

### Getting Started
- [Installation](getting-started/installation.md)
- [Configuration](getting-started/configuration.md)
- [First Commands](getting-started/first-commands.md)
- [Provider Setup](getting-started/providers.md)

### Features
- [CLI Commands](features/cli.md)
- [Risk Levels](features/risk-levels.md)
- [Voice Interface](features/voice.md)
- [Visual Computer Use](features/vision.md)
- [MCP Servers](features/mcp.md)
- [Session Memory](features/memory.md)
- [Rollback System](features/rollback.md)
- [Flux Capacitor](features/flux.md)
- [Mini-Me Subagents](features/mini-me.md)

### Architecture
- [System Overview](architecture/overview.md)
- [Core Engine](architecture/core.md)
- [Provider Layer](architecture/providers.md)
- [Desktop App](architecture/desktop.md)
- [Security Model](architecture/security.md)

### Guides
- [Working with Code](guides/coding.md)
- [System Administration](guides/sysadmin.md)
- [Using Vision Features](guides/vision-guide.md)
- [Voice Conversations](guides/voice-guide.md)
- [Extending with MCP](guides/mcp-guide.md)

---

## Version History

| Version | Date | Highlights |
|---------|------|------------|
| 4.0.0 | TBD | Complete rewrite, desktop app, vision |
| 3.14.0 | 2026-01 | Beta release, MCP support |
