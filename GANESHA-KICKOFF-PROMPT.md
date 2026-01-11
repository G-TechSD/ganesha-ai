# GANESHA 2.0 - Build Plan Request

**Paste this into Claudia Coder's build plan input to generate the definitive implementation plan.**

---

## PROJECT: GANESHA 2.0 - THE REMOVER OF OBSTACLES

Build the **definitive AI-powered system control tool** - the one that pioneered the concepts Claude Code and OpenAI Codex CLI now copy.

### WHAT IS GANESHA?

Ganesha is a **natural language to system command translator** with bulletproof safety. Users describe what they want in plain English, and Ganesha:

1. **Plans** - Translates intent into executable commands
2. **Presents** - Shows the user exactly what will happen, with risk levels
3. **Executes** - Runs approved commands with full logging
4. **Rolls Back** - Can undo any session's changes

Named after the Hindu deity known as **"The Remover of Obstacles"** - this tool removes the obstacles between human intent and system execution.

**Original development: 2024** - predating both Claude Code and OpenAI Codex CLI.

---

### EXISTING IMPLEMENTATIONS

Two reference implementations exist:

#### Python (ganesha-ai/ganesha.py)
- 1,450+ lines of working code
- GPT-4 integration with OpenAI SDK 0.28
- CLI with colorama terminal UI
- Session logging and rollback
- Interactive REPL mode
- `--A` flag for auto-approve (predates `--dangerously-skip-permissions`)
- Cross-platform: Linux, macOS, Windows

#### Rust (ganesha-ai/ganesha-rs/)
- Modern async architecture with Tokio
- Provider abstraction for multiple LLMs
- Access control presets (restricted, standard, elevated, full_access)
- Safety filter with 100+ dangerous patterns
- Optional features: vision, input, voice, browser
- MCP server protocol support
- HTTP API with Axum

---

### GANESHA 2.0 VISION

A **single unified Rust binary** that is:

1. **Fast** - Native performance, instant startup
2. **Portable** - Download and run, no dependencies
3. **Local-First** - Works with LM Studio/Ollama, no cloud required
4. **Safe by Default** - Multiple layers of protection
5. **Multi-Interface** - CLI, MCP, HTTP API, all from one binary
6. **Cross-Platform** - Linux, macOS, Windows from single codebase

---

### CORE FEATURES TO IMPLEMENT

#### 1. Natural Language to Commands
```bash
ganesha "install docker and configure it to start on boot"
ganesha "find all files larger than 1GB and show me"
ganesha "set up nginx as a reverse proxy for port 3000"
ganesha --code "create a React login form with TypeScript"
```

#### 2. Provider Abstraction
Priority order (local-first):
1. **LM Studio** (localhost:1234) - OpenAI-compatible
2. **Ollama** (localhost:11434) - Native API
3. **Anthropic Claude** - Cloud fallback
4. **OpenAI** - Last resort

Auto-detection of available providers. Configurable via `~/.ganesha/config.toml`.

#### 3. Safety Architecture

**The Sentinel** - Independent security guardian:
- Separate context from operator (can't see user prompt)
- SOC analyst mindset for action evaluation
- Threat scoring with accumulating risk
- Categories: DataExfiltration, SystemCorruption, PromptInjection, etc.

**Access Control Presets**:
| Level | Description |
|-------|-------------|
| `restricted` | Read-only commands only |
| `standard` | Safe modifications (DEFAULT) |
| `elevated` | Package/service management |
| `full_access` | Everything except catastrophic |
| `whitelist` | Only explicitly allowed patterns |
| `blacklist` | Everything except denied patterns |

**Immutable Rules** (cannot be disabled):
- Blocks: `rm -rf /`, disk wiping, fork bombs
- Blocks: Self-invocation with `--auto` flag
- Blocks: System log tampering
- Blocks: Ganesha config modification via commands

**Anti-Manipulation**:
- Detects: "ignore previous instructions", "bypass safety", etc.
- Detects: Leetspeak and Unicode homoglyph obfuscation
- Detects: Multi-step trap patterns

#### 4. Session Management
```bash
ganesha --rollback           # Undo last session
ganesha --rollback <id>      # Undo specific session
ganesha --history            # Show session history
```

Full session logging to `~/.ganesha/sessions/`:
- Task description
- Planned actions
- Executed commands
- Outputs/errors
- Rollback commands (pre-computed)

#### 5. Multiple Interfaces

**CLI** (primary):
```bash
ganesha "your task"
ganesha --auto "dangerous task"
ganesha --interactive
```

**MCP Server** (for Claude Code/Claudia):
```json
{
  "mcpServers": {
    "ganesha": {
      "command": "ganesha",
      "args": ["--mcp"]
    }
  }
}
```
Tools: `ganesha_execute`, `ganesha_plan`, `ganesha_rollback`, `ganesha_generate_code`

**HTTP API** (for web integration):
```bash
ganesha --api --port 8420
```
Endpoints: `/execute`, `/plan`, `/rollback`, `/history`, `/health`

#### 6. GUI Automation (Computer Use)
Optional features requiring explicit opt-in:

**Vision** (`--features vision`):
- Screenshot capture
- Screen content analysis via vision LLMs
- OCR for text extraction
- Rate limited: 30 screenshots/minute

**Input** (`--features input`):
- Mouse movement, clicks
- Keyboard input, hotkeys
- Scroll control
- Dangerous key combos blocked (Ctrl+Alt+Delete, Alt+F4)

**Safe Applications** (less restrictive):
- Creative: Blender, GIMP, Inkscape
- Development: VSCode, Terminal
- 3D Printing: PrusaSlicer, Bambu Studio

**Dangerous Contexts** (always require confirmation):
- Password fields, banking apps
- Admin/root dialogs, BIOS settings

#### 7. Voice Conversation Mode
Optional feature (`--features voice`):

**Real-Time Architecture**:
```
Microphone -> VAD -> Audio Chunks -> WebSocket -> LLM -> Audio Playback -> Speaker
```

- Streaming bidirectional audio
- Voice Activity Detection for turn-taking
- Barge-in support (interrupt AI)
- ~200ms target latency

**Voice Providers**:
1. OpenAI Realtime API
2. Local Whisper + TTS
3. Custom WebSocket endpoint

**Voice Safety**:
- Never auto-approve commands via voice alone
- Kill switch for immediate silence

#### 8. Code Generation Mode
```bash
ganesha --code "add dark mode to the settings page"
ganesha --code "create unit tests for the API routes"
ganesha --code "refactor this function to use async/await"
```

- Git integration (status, commit, branch)
- File read/write with diff preview
- Project context awareness (package.json, Cargo.toml, etc.)

---

### ARCHITECTURE

```
+---------------------------------------------------------------+
|                       GANESHA 2.0                             |
+---------------------------------------------------------------+
|                                                               |
|  +----------+  +----------+  +----------+  +----------+       |
|  |   CLI    |  |   MCP    |  |   API    |  |  Voice   |       |
|  +----+-----+  +----+-----+  +----+-----+  +----+-----+       |
|       |             |             |             |             |
|       +-------------+------+------+-------------+             |
|                            |                                  |
|                     +------v------+                           |
|                     | Core Engine |                           |
|                     +------+------+                           |
|                            |                                  |
|       +--------------------+--------------------+             |
|       |                    |                    |             |
|  +----v----+         +-----v-----+        +-----v-----+       |
|  | Planner |         | Executor  |        | Sentinel  |       |
|  +---------+         +-----------+        +-----------+       |
|                            |                    |             |
|                     +------v------+      +------v------+      |
|                     |   Safety    |      |   Logging   |      |
|                     +-------------+      +-------------+      |
|                            |                                  |
|       +--------------------+--------------------+             |
|       |                    |                    |             |
|  +----v----+         +-----v-----+        +-----v-----+       |
|  |LM Studio|         |  Ollama   |        | Anthropic |       |
|  +---------+         +-----------+        +-----------+       |
|                                                               |
+---------------------------------------------------------------+
```

---

### DAEMON MODE

For privileged operations:

```bash
# Install daemon (one-time)
sudo ganesha-daemon install

# User must be in 'ganesha' group
sudo usermod -aG ganesha $USER
```

- Daemon runs as root
- CLI communicates via Unix socket
- Group permissions control access
- All actions still logged to system logs

---

### CROSS-PLATFORM LOGGING

Actions logged to **system logs** (not just application logs):
- **Linux**: journald/syslog (`journalctl -t ganesha`)
- **macOS**: Unified Log (`log show --predicate 'subsystem == "com.gtechsd.ganesha"'`)
- **Windows**: Event Viewer (Source: "Ganesha")

Why system logs:
- Harder to tamper (requires root)
- Persist across reinstalls
- Integrate with SIEM/monitoring
- Legal audit trail

---

### DISTRIBUTION

**Single Binary**:
```bash
# Linux
curl -L https://releases.ganesha.ai/latest/ganesha-linux-amd64 -o ganesha
chmod +x ganesha
./ganesha "hello world"

# macOS
curl -L https://releases.ganesha.ai/latest/ganesha-darwin-arm64 -o ganesha
# Windows
curl -L https://releases.ganesha.ai/latest/ganesha-windows-amd64.exe -o ganesha.exe
```

Build targets:
- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl` (static)
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

---

### THE WIGGUM LOOP

Named after "trust but verify":

```
while not satisfactory:
    1. Generate solution
    2. Execute (with consent)
    3. Verify result matches intent
    4. If not correct, iterate with context
```

- Context preservation across iterations
- Bounded retry limit
- Human can abort at any point

---

### WHAT TO PRESERVE FROM EXISTING CODE

**From Python**:
- Conversation history for feedback loops
- Session logging format
- Rollback command generation
- Interactive REPL flow
- Cross-platform command detection

**From Rust**:
- Provider abstraction trait
- Access control presets
- Safety filter patterns (100+ dangerous keywords)
- Threat scoring algorithm
- Feature flags for dangerous capabilities
- Axum HTTP server setup
- Clap CLI structure

---

### DELIVERABLES

1. **ganesha** - Main CLI binary
2. **ganesha-daemon** - Privileged daemon for elevated operations
3. **libganesha** - Core library for embedding
4. Configuration: `~/.ganesha/config.toml`
5. Session storage: `~/.ganesha/sessions/`
6. MCP manifest for Claude Code integration
7. GitHub Actions for multi-platform builds

---

### SUCCESS CRITERIA

1. User can `ganesha "install docker"` and have it work on any platform
2. Local LLM (LM Studio/Ollama) is detected and used automatically
3. All commands require explicit approval unless `--auto` is set
4. Session can be rolled back with `ganesha --rollback`
5. MCP server mode works with Claude Code
6. HTTP API serves Claudia Admin
7. Binary size < 20MB (stripped)
8. Startup time < 100ms

---

### THE MISSION

This is not just another CLI tool.

**Ganesha pioneered**:
- Natural language to system commands
- User consent flows before execution
- Session rollback capabilities
- Auto-approve flags for automation
- Cross-platform compatibility

Others have copied these concepts. Now we build **the definitive implementation** - faster, safer, more capable than anything that came after.

The Remover of Obstacles. Version 2.0.

**Build it.**

---

*G-Tech SD - 2024-2026*
*"The first AI-powered system control tool. Predates the rest."*
