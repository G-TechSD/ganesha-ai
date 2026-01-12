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

**The first cross-platform AI-powered system control tool.**

Originally developed in 2024 - predating Claude Code and OpenAI Codex CLI.

---

## What is Ganesha?

Ganesha translates natural language into safe, executable system commands. Just describe what you want to do, and Ganesha figures out how to do it.

```bash
ganesha "install docker and configure it to start on boot"
ganesha "find all files larger than 1GB and show me"
ganesha "set up nginx as a reverse proxy for port 3000"
```

### Key Features

- **Natural Language Control** - Speak plainly, get results
- **Local-First LLMs** - Works with LM Studio, Ollama - no cloud required
- **Safe by Default** - User consent required before execution
- **Rollback Support** - Undo any session's changes
- **Cross-Platform** - Linux, macOS, Windows
- **MCP Compatible** - Use as a tool in Claude Code or any MCP client
- **HTTP API** - Integrate with web apps like Claudia Admin
- **Flux Capacitor** - Time-boxed autonomous task execution

---

## Downloads

Pre-built binaries for the Rust version (v3.0.0):

| Platform | Architecture | Download |
|----------|-------------|----------|
| **Linux** | x86_64 | [ganesha-linux-x86_64.tar.gz](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-linux-x86_64.tar.gz) |
| **Linux** | ARM64 | [ganesha-linux-arm64.tar.gz](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-linux-arm64.tar.gz) |
| **macOS** | Intel | [ganesha-macos-x86_64.tar.gz](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-macos-x86_64.tar.gz) |
| **macOS** | Apple Silicon | [ganesha-macos-arm64.tar.gz](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-macos-arm64.tar.gz) |
| **Windows** | x86_64 | [ganesha-windows-x86_64.zip](https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-windows-x86_64.zip) |

### Quick Install (Linux/macOS)

```bash
# Download and install latest release
curl -fsSL https://raw.githubusercontent.com/G-TechSD/ganesha-ai/main/install.sh | bash
```

### Manual Install

```bash
# Download for your platform
wget https://github.com/G-TechSD/ganesha-ai/releases/latest/download/ganesha-linux-x86_64.tar.gz

# Extract
tar -xzf ganesha-linux-x86_64.tar.gz

# Move to PATH
sudo mv ganesha /usr/local/bin/

# Verify
ganesha --version
```

---

## Installation (From Source)

```bash
# Clone
git clone https://github.com/G-TechSD/ganesha-ai.git
cd ganesha-ai

# Install (basic)
pip install -e .

# Install (all features)
pip install -e ".[all]"
```

### Requirements

- Python 3.10+
- A local LLM server (LM Studio or Ollama) OR cloud API key

---

## Quick Start

### CLI Mode

```bash
# Execute a task
ganesha "show disk usage"

# Auto-approve (dangerous!)
ganesha --auto "update all packages"

# Interactive REPL
ganesha --interactive

# Code generation mode
ganesha --code "create a React login form"

# Rollback last session
ganesha --rollback
```

### Flux Capacitor (Autonomous Mode)

Run tasks autonomously for a specified duration:

```bash
# Run for 1 hour
ganesha --flux "1h" "optimize all database queries in this project"

# Run until a specific time
ganesha --until "6:00" "generate unit tests for all functions"

# Run forever (until Ctrl+C)
ganesha --flux auto "continuously monitor and fix linter errors"

# Generate 1000 items with creative temperature
ganesha -A --flux "30m" --temp 1.0 "Generate 1000 cat facts"

# Resume a previous session
ganesha --flux "1h" --resume flux_20260112 "Continue where we left off"
```

**Features:**
- **FluxCanvas** - Persistent workspace that accumulates items/files across iterations
- **Progress tracking** - Auto-detects targets ("1000 facts" -> shows 0/1000)
- **Session resume** - Continue from where you left off
- **Export** - Auto-exports to HTML and raw text at completion

### MCP Server

Add to your Claude Desktop config:

```json
{
  "mcpServers": {
    "ganesha": {
      "command": "python",
      "args": ["-m", "ganesha.mcp.server"]
    }
  }
}
```

Now Claude Code can use Ganesha tools:
- `ganesha_execute` - Execute system tasks
- `ganesha_plan` - Plan without executing
- `ganesha_rollback` - Undo changes
- `ganesha_generate_code` - Generate code

### HTTP API

```bash
# Start the API server
ganesha-api

# Or with uvicorn
uvicorn ganesha.api.server:app --port 8420
```

Endpoints:
- `POST /execute` - Execute a task
- `POST /plan` - Plan without executing
- `POST /rollback` - Rollback session
- `GET /history` - Session history
- `GET /providers` - Available LLM providers
- `GET /health` - Health check

---

## Configuration

### Local LLM Setup

Ganesha works with any OpenAI-compatible local LLM server:

**LM Studio:**
1. Download from https://lmstudio.ai
2. Load a model (recommended: deepseek-coder, codellama, or similar)
3. Start the local server
4. Ganesha auto-detects at `http://localhost:1234`

**Ollama:**
1. Install from https://ollama.ai
2. Pull a model: `ollama pull llama3`
3. Ganesha auto-detects at `http://localhost:11434`

### Provider Priority

Ganesha tries providers in this order:
1. LM Studio (local)
2. Ollama (local)
3. Anthropic Claude (cloud)
4. OpenAI (cloud)

Set cloud API keys if you want fallback:
```bash
export ANTHROPIC_API_KEY="your-key"
export OPENAI_API_KEY="your-key"
```

---

## Safety

Ganesha is **safe by default**:

1. **Planning** - Shows exactly what will run before execution
2. **Consent** - Requires explicit approval for each action
3. **Risk Levels** - Flags dangerous operations (high/critical)
4. **Rollback** - Can undo any session's changes
5. **Logging** - Full session history in `~/.ganesha/sessions/`

To bypass consent (use carefully):
```bash
ganesha --auto "your task"  # Auto-approve all
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     GANESHA 3.0                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                     │
│  │   CLI   │  │   MCP   │  │   API   │   ← Interfaces      │
│  └────┬────┘  └────┬────┘  └────┬────┘                     │
│       │            │            │                           │
│       └────────────┼────────────┘                           │
│                    ▼                                        │
│           ┌───────────────┐                                 │
│           │  Core Engine  │  ← Planning, Execution, Safety │
│           └───────┬───────┘                                 │
│                   ▼                                        │
│           ┌───────────────┐                                 │
│           │   Providers   │  ← LLM Abstraction             │
│           └───────┬───────┘                                 │
│                   ▼                                         │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       │
│  │LM Studio│  │ Ollama  │  │Anthropic│  │ OpenAI  │       │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## History

Ganesha was originally developed in 2024, before:
- Claude Code (Anthropic)
- Codex CLI (OpenAI)

It pioneered concepts that are now standard:
- Natural language to system commands
- User consent flows before execution
- Session rollback capabilities
- Auto-approve flags for automation
- Cross-platform compatibility

This rewrite (v3.0) adds:
- Local LLM support (LM Studio, Ollama)
- MCP server protocol
- HTTP API for web integration
- Async-first architecture
- Clean, modern codebase

---

## Why "Ganesha"?

In Hindu tradition, **Ganesha** is the deity known as the **Remover of Obstacles**.

This tool removes the obstacles between:
- Your intentions → Executable commands
- Natural language → System control
- Non-technical users → Terminal power

---

## License

MIT License - See LICENSE file.

---

## Author

**G-Tech SD**
- GitHub: [@G-TechSD](https://github.com/G-TechSD)
- Repository: [ganesha-ai](https://github.com/G-TechSD/ganesha-ai)

*The first AI-powered system control tool. Predates the rest.*
