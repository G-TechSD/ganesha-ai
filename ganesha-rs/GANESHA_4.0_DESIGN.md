# Ganesha 4.0 - The Obstacle Remover

## Vision Statement

A lightning-fast, local-first AI assistant that combines the best of Claude Code, Gemini CLI, and Codex with unique capabilities: conversational voice with personality, visual computer control, and a non-intrusive desktop companion. Privacy-respecting, open-source, and actually useful.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           GANESHA 4.0 ECOSYSTEM                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                   │
│  │   TERMINAL   │    │  DESKTOP APP │    │   VOICE      │                   │
│  │    (CLI)     │    │ "Remover"    │    │  INTERFACE   │                   │
│  │              │    │              │    │              │                   │
│  │  - Commands  │    │  - Tray icon │    │  - Wake word │                   │
│  │  - TUI mode  │    │  - PTT btn   │    │  - PTT       │                   │
│  │  - Scripts   │    │  - Glass UI  │    │  - Voices    │                   │
│  │  - Pipes     │    │  - Screen    │    │  - Realtime  │                   │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘                   │
│         │                   │                   │                            │
│         └───────────────────┼───────────────────┘                            │
│                             ▼                                                │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         GANESHA CORE                                  │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐      │   │
│  │  │  PLANNER   │  │  EXECUTOR  │  │  VERIFIER  │  │   MEMORY   │      │   │
│  │  │            │  │            │  │  (Wiggum)  │  │            │      │   │
│  │  │ Task→Plan  │  │ Plan→Act   │  │ Act→Check  │  │ Persist    │      │   │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘      │   │
│  │                                                                       │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐      │   │
│  │  │  ROLLBACK  │  │    MCP     │  │   VISION   │  │  SANDBOX   │      │   │
│  │  │            │  │  Manager   │  │    VLA     │  │            │      │   │
│  │  │ Snapshots  │  │ Hot-load   │  │ Screen→Act │  │ Isolated   │      │   │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                             │                                                │
│                             ▼                                                │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                      PROVIDER LAYER                                   │   │
│  │                                                                       │   │
│  │  LOCAL FIRST:           CLOUD:              SPECIALIZED:              │   │
│  │  • LM Studio            • OpenRouter        • Vision (GPT-4V, etc)    │   │
│  │  • Ollama               • Anthropic         • Voice (OpenAI Realtime) │   │
│  │  • llama.cpp            • OpenAI            • Embeddings              │   │
│  │  • vLLM                 • Google            • VLA Models              │   │
│  │  • Text Gen WebUI       • Groq                                        │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Core Modules

### 1. Command Interface (`ganesha`)

```bash
# Quick commands (like Claude Code)
ganesha "what time is it"
ganesha "list files in ~/Downloads"

# Modes
ganesha --tui                    # Full terminal UI
ganesha --voice                  # Voice conversation mode
ganesha --flux 2h "build X"      # Time-boxed autonomous work
ganesha --wiggum "complex task"  # Verification loop mode

# Risk levels (human-readable)
ganesha --safe "..."             # Read-only, no system changes
ganesha --normal "..."           # Default, asks for risky ops
ganesha --yolo "..."             # Auto-approve everything
ganesha -A "..."                 # Alias for --yolo (allow all)

# Execution contexts
ganesha --sandbox "..."          # Isolated container/VM
ganesha --live "..."             # Direct system access (default)

# Session management
ganesha --resume                 # Continue last session
ganesha --session myproject      # Named session
ganesha --history                # View past sessions
ganesha --rollback [id]          # Undo changes

# MCP
ganesha mcp install fetch        # Install MCP server
ganesha mcp list                 # Show installed
ganesha mcp enable/disable X     # Hot load/unload

# Configuration
ganesha --configure              # Interactive setup
ganesha --providers              # Manage LLM providers
ganesha --voices                 # Configure voice/personality
```

### 2. Risk Levels (Human-Readable)

```
┌─────────────────────────────────────────────────────────────────┐
│                    RISK LEVEL SYSTEM                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  🟢 SAFE (--safe)                                               │
│     "I'll look but won't touch"                                 │
│     • Read files, list directories                              │
│     • Web searches, fetch content                               │
│     • Analyze and explain code                                  │
│     • NO writes, NO commands, NO system changes                 │
│                                                                 │
│  🟡 NORMAL (default)                                            │
│     "I'll ask before anything risky"                            │
│     • All safe operations                                       │
│     • Create/edit files (with confirmation)                     │
│     • Run safe commands (ls, cat, git status)                   │
│     • Asks permission for: installs, deletes, sudo              │
│                                                                 │
│  🟠 TRUSTED (--trusted)                                         │
│     "I'll handle routine tasks automatically"                   │
│     • All normal operations auto-approved                       │
│     • Installs, git operations, file management                 │
│     • Still asks for: sudo, system config, destructive ops      │
│                                                                 │
│  🔴 YOLO (-A, --yolo)                                           │
│     "Full send, no questions asked"                             │
│     • Everything auto-approved                                  │
│     • Use with caution                                          │
│     • Rollback always available                                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 3. Desktop App ("The Obstacle Remover")

A lightweight, non-intrusive companion app:

```
┌─────────────────────────────────────────────────────────────────┐
│                    OBSTACLE REMOVER APP                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  TRAY ICON:                                                     │
│  • 🔵 Idle - Ganesha ready                                      │
│  • 🟢 Active - Processing request                               │
│  • 🟡 Waiting - Needs input                                     │
│  • 🔴 Error - Something went wrong                              │
│                                                                 │
│  MAIN WINDOW (floating, glass-like):                            │
│  ┌─────────────────────────────────────┐                        │
│  │  ◉ Push to Talk                     │  <- Big button         │
│  │                                     │                        │
│  │  ┌─────────────────────────────┐    │                        │
│  │  │ "What would you like       │    │  <- Glass bubble       │
│  │  │  me to help with?"         │    │                        │
│  │  └─────────────────────────────┘    │                        │
│  │                                     │                        │
│  │  [Settings] [History] [Minimize]    │                        │
│  └─────────────────────────────────────┘                        │
│                                                                 │
│  ACTIVE INDICATOR (when working):                               │
│  ┌─────────────────────────────────────┐                        │
│  │  🟢 GANESHA IS REMOVING OBSTACLES   │                        │
│  │  ─────────────────────────────────  │                        │
│  │  Opening Blender and creating       │                        │
│  │  a new project with your specs...   │                        │
│  └─────────────────────────────────────┘                        │
│                                                                 │
│  SCREEN BORDER (during visual control):                         │
│  ╔═══════════════════════════════════════════════════════════╗  │
│  ║                                                           ║  │
│  ║    Entire screen gets subtle green border                 ║  │
│  ║    indicating Ganesha has control                         ║  │
│  ║                                                           ║  │
│  ╚═══════════════════════════════════════════════════════════╝  │
│                                                                 │
│  FEATURES:                                                      │
│  • Auto-detects local LLM servers on network                    │
│  • Provider/model configuration UI                              │
│  • Never required - terminal works independently                │
│  • Easily disabled via tray                                     │
│  • No ads, no telemetry, no surveillance                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4. Vision & VLA Computer Control

```
┌─────────────────────────────────────────────────────────────────┐
│                    VISUAL COMPUTER USE                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  PIPELINE:                                                      │
│                                                                 │
│  Screen Capture (60fps) ──▶ VLA Model ──▶ Action Planning       │
│         │                      │                │               │
│         │                      │                ▼               │
│         │                      │         Mouse/Keyboard         │
│         │                      │              │                 │
│         │                      ▼              ▼                 │
│         └──────────────── Verification ◀── Execute              │
│                                                                 │
│  VLA (Vision-Language-Action) OPTIONS:                          │
│  • OpenAI GPT-4V + function calling                             │
│  • Claude 3.5 Sonnet computer use                               │
│  • Local: CogVLM, LLaVA, Qwen-VL                               │
│  • Specialized: UI-focused models                               │
│                                                                 │
│  APP CONTROL:                                                   │
│  • Whitelist: Apps Ganesha CAN control                          │
│  • Blacklist: Apps Ganesha must NOT touch                       │
│  • Default blacklist: Password managers, banking, etc           │
│  • -A flag overrides (use with extreme caution)                 │
│                                                                 │
│  TESTED WITH:                                                   │
│  • Blender (3D modeling)                                        │
│  • Bambu Studio (3D printing)                                   │
│  • CapCut (video editing)                                       │
│  • OBS (streaming/recording)                                    │
│  • Various TUI applications                                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 5. Voice System

```
┌─────────────────────────────────────────────────────────────────┐
│                    VOICE & PERSONALITY                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  MODES:                                                         │
│  • Push-to-Talk (default) - Hold button to speak                │
│  • Wake Word - "Hey Ganesha" / "Obstacle Remover"               │
│  • Always Listening (with privacy controls)                     │
│                                                                 │
│  VOICES/PERSONALITIES:                                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ 🎭 Professional - Clear, direct, business-like          │    │
│  │ 🎭 Friendly - Warm, encouraging, conversational         │    │
│  │ 🎭 Snarky - Witty, playful, mildly sarcastic           │    │
│  │ 🎭 Mentor - Patient, explanatory, educational           │    │
│  │ 🎭 Minimalist - Terse, efficient, just the facts        │    │
│  │ 🎭 Custom - User-defined personality prompt             │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  IMPLEMENTATION:                                                │
│  • OpenAI Realtime API (WebSocket, ~200ms latency)              │
│  • Local: Whisper + TTS (Coqui, Piper, etc)                     │
│  • Barge-in support (interrupt while speaking)                  │
│  • Context-aware responses                                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 6. MCP Hot-Loading System

```
┌─────────────────────────────────────────────────────────────────┐
│                    MCP SERVER MANAGEMENT                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  GLOBAL REGISTRY (~/.ganesha/mcp/):                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ servers.json - Installed servers + configs               │    │
│  │ credentials.json - Encrypted API keys (keyring-backed)   │    │
│  │ server-cache/ - Downloaded server binaries               │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  HOT LOADING:                                                   │
│  • Servers loaded on-demand based on task                       │
│  • Unloaded after idle timeout (configurable)                   │
│  • Memory-efficient - only active servers run                   │
│                                                                 │
│  AUTO-DETECTION:                                                │
│  • "search for X" → loads ganesha:web_search                    │
│  • "what's on website.com" → loads fetch or playwright          │
│  • "check my github" → loads github MCP                         │
│                                                                 │
│  CREDENTIAL FLOW:                                               │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ 1. User requests MCP tool requiring auth                 │    │
│  │ 2. Ganesha checks keyring for stored credential          │    │
│  │ 3. If missing: interactive prompt for API key            │    │
│  │ 4. Store securely, use for future requests               │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  BUILT-IN SERVERS:                                              │
│  • ganesha:web_search - DuckDuckGo/SearxNG                      │
│  • ganesha:fetch - HTTP content extraction                      │
│  • ganesha:filesystem - Sandboxed file access                   │
│  • ganesha:shell - Command execution                            │
│                                                                 │
│  INSTALLABLE:                                                   │
│  • playwright - Browser automation                              │
│  • github - GitHub API                                          │
│  • slack, discord - Messaging                                   │
│  • kubernetes - K8s management                                  │
│  • Custom via URL/npm                                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 7. Model Quality Tiers

```
┌─────────────────────────────────────────────────────────────────┐
│                    MODEL QUALITY SYSTEM                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  TIER 1 - EXCEPTIONAL (Green checkmark)                         │
│  Models that excel at agentic tasks:                            │
│  • Claude 3.5 Sonnet, Claude 3 Opus                             │
│  • GPT-4o, GPT-4 Turbo                                          │
│  • Gemini 1.5 Pro                                               │
│  • DeepSeek V3                                                  │
│  • Qwen 2.5 72B+                                                │
│  • Llama 3.1 405B                                               │
│                                                                 │
│  TIER 2 - CAPABLE (Yellow)                                      │
│  Good for most tasks with occasional issues:                    │
│  • GPT-4o-mini                                                  │
│  • Llama 3.1 70B                                                │
│  • Mistral Large                                                │
│  • Qwen 2.5 32B                                                 │
│                                                                 │
│  TIER 3 - LIMITED (Orange)                                      │
│  Works for simple tasks, may struggle with complex:             │
│  • Llama 3.1 8B                                                 │
│  • Mistral 7B                                                   │
│  • Phi-3                                                        │
│                                                                 │
│  TIER 4 - UNSAFE (Red warning)                                  │
│  May produce dangerous/incorrect commands:                      │
│  • Very small models (<3B)                                      │
│  • Untuned base models                                          │
│  • Models not trained for instruction following                 │
│  ⚠️ Ganesha warns before using these                            │
│                                                                 │
│  AUTO-DETECTION:                                                │
│  • Benchmark on first use                                       │
│  • Track success/failure rates                                  │
│  • Adjust tier dynamically                                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 8. Mini-Me Subagents

```
┌─────────────────────────────────────────────────────────────────┐
│                    MINI-ME SUBAGENT SYSTEM                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  PRIMARY GANESHA (orchestrator):                                │
│  • Uses best available model                                    │
│  • Plans complex tasks                                          │
│  • Delegates to Mini-Me's                                       │
│  • Verifies results                                             │
│                                                                 │
│  MINI-ME AGENTS (workers):                                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ 🔍 Research Mini-Me                                      │    │
│  │    - Web searches, content extraction                    │    │
│  │    - Uses smaller/faster model                           │    │
│  │                                                          │    │
│  │ 💻 Code Mini-Me                                          │    │
│  │    - Write/edit code files                               │    │
│  │    - Run tests, fix errors                               │    │
│  │                                                          │    │
│  │ 🖥️ System Mini-Me                                        │    │
│  │    - Shell commands, system admin                        │    │
│  │    - Package management                                  │    │
│  │                                                          │    │
│  │ 👁️ Vision Mini-Me                                        │    │
│  │    - Screen analysis                                     │    │
│  │    - GUI automation                                      │    │
│  │                                                          │    │
│  │ 📝 Writer Mini-Me                                        │    │
│  │    - Documentation, reports                              │    │
│  │    - Content generation                                  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  BENEFITS:                                                      │
│  • Parallel execution of independent tasks                      │
│  • Cost optimization (cheap model for simple tasks)             │
│  • Specialized context per agent type                           │
│  • Failure isolation                                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Project Structure

```
ganesha/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── ganesha-core/             # Core engine, planning, execution
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── engine.rs         # Main orchestration
│   │   │   ├── planner.rs        # Task → Plan
│   │   │   ├── executor.rs       # Plan → Actions
│   │   │   ├── verifier.rs       # Wiggum verification
│   │   │   ├── memory.rs         # Session persistence
│   │   │   ├── rollback.rs       # Undo system
│   │   │   └── sandbox.rs        # Isolated execution
│   │   └── Cargo.toml
│   │
│   ├── ganesha-providers/        # LLM provider abstraction
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── openai.rs
│   │   │   ├── anthropic.rs
│   │   │   ├── openrouter.rs
│   │   │   ├── local.rs          # LM Studio, Ollama, etc
│   │   │   └── tiers.rs          # Model quality ratings
│   │   └── Cargo.toml
│   │
│   ├── ganesha-mcp/              # MCP server management
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── manager.rs        # Hot loading
│   │   │   ├── registry.rs       # Server catalog
│   │   │   ├── protocol.rs       # MCP protocol impl
│   │   │   └── builtin/          # Built-in servers
│   │   └── Cargo.toml
│   │
│   ├── ganesha-vision/           # Screen capture + VLA
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── capture.rs        # Cross-platform capture
│   │   │   ├── vla.rs            # Vision-Language-Action
│   │   │   ├── input.rs          # Mouse/keyboard control
│   │   │   └── safety.rs         # App whitelist/blacklist
│   │   └── Cargo.toml
│   │
│   ├── ganesha-voice/            # Voice I/O
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── capture.rs        # Audio input
│   │   │   ├── playback.rs       # Audio output
│   │   │   ├── realtime.rs       # WebSocket streaming
│   │   │   ├── local.rs          # Whisper + local TTS
│   │   │   └── personality.rs    # Voice characters
│   │   └── Cargo.toml
│   │
│   ├── ganesha-cli/              # Terminal interface
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── args.rs           # Clap definitions
│   │   │   ├── repl.rs           # Interactive REPL
│   │   │   ├── tui.rs            # Full TUI mode
│   │   │   └── output.rs         # Pretty printing
│   │   └── Cargo.toml
│   │
│   └── ganesha-desktop/          # Desktop app (Tauri)
│       ├── src/
│       │   ├── main.rs
│       │   └── lib.rs
│       ├── src-tauri/            # Rust backend
│       │   ├── src/
│       │   │   ├── main.rs
│       │   │   ├── tray.rs       # System tray
│       │   │   ├── overlay.rs    # Screen border
│       │   │   └── discovery.rs  # Network LLM detection
│       │   └── Cargo.toml
│       ├── src/                  # Web frontend
│       │   ├── App.svelte
│       │   ├── components/
│       │   │   ├── PushToTalk.svelte
│       │   │   ├── GlassBubble.svelte
│       │   │   ├── StatusIndicator.svelte
│       │   │   └── Settings.svelte
│       │   └── styles/
│       └── package.json
│
├── config/
│   ├── default.toml              # Default settings
│   └── models.toml               # Model tier definitions
│
└── docs/
    ├── QUICKSTART.md
    ├── COMMANDS.md
    ├── VOICE.md
    ├── VISION.md
    └── MCP.md
```

---

## Key Differentiators from Existing Tools

| Feature | Claude Code | Gemini CLI | Codex | Ganesha 4.0 |
|---------|-------------|------------|-------|-------------|
| Local-first | ❌ | ❌ | ❌ | ✅ |
| Voice conversation | ❌ | ❌ | ❌ | ✅ |
| Visual computer use | ❌ | ❌ | ❌ | ✅ |
| Desktop companion | ❌ | ❌ | ❌ | ✅ |
| Push-to-talk | ❌ | ❌ | ❌ | ✅ |
| MCP hot-loading | ❌ | ❌ | ❌ | ✅ |
| Personality/voices | ❌ | ❌ | ❌ | ✅ |
| Risk levels | ⚠️ | ⚠️ | ⚠️ | ✅ Human-readable |
| Rollback/undo | ❌ | ❌ | ❌ | ✅ |
| Session memory | ⚠️ | ⚠️ | ⚠️ | ✅ |
| Flux time-boxing | ❌ | ❌ | ❌ | ✅ |
| Model quality tiers | ❌ | ❌ | ❌ | ✅ |
| No telemetry | ❓ | ❌ | ❓ | ✅ |

---

## Implementation Phases

### Phase 1: Core Foundation (2 weeks)
- [ ] Project structure setup
- [ ] Provider abstraction (local + cloud)
- [ ] Basic CLI with commands
- [ ] Session persistence
- [ ] Rollback system

### Phase 2: Enhanced CLI (2 weeks)
- [ ] Full TUI mode
- [ ] MCP hot-loading
- [ ] Risk level system
- [ ] Model quality tiers
- [ ] Flux Capacitor mode

### Phase 3: Voice (2 weeks)
- [ ] Push-to-talk
- [ ] OpenAI Realtime integration
- [ ] Local voice (Whisper + TTS)
- [ ] Personality system

### Phase 4: Vision (2 weeks)
- [ ] Screen capture
- [ ] VLA integration
- [ ] Mouse/keyboard control
- [ ] App whitelist/blacklist
- [ ] Safety systems

### Phase 5: Desktop App (2 weeks)
- [ ] Tauri app shell
- [ ] System tray
- [ ] Glass UI design
- [ ] Provider discovery
- [ ] Screen border overlay

### Phase 6: Polish & Testing (2 weeks)
- [ ] Test with Blender, Bambu, CapCut, OBS
- [ ] Cross-platform testing
- [ ] Performance optimization
- [ ] Documentation
- [ ] Release packaging

---

## Tech Stack

- **Language**: Rust (core), Svelte (desktop UI)
- **Desktop**: Tauri 2.0
- **TUI**: Ratatui
- **Audio**: cpal, rodio
- **HTTP**: reqwest, axum
- **WebSocket**: tokio-tungstenite
- **Screen Capture**: xcap (cross-platform)
- **Input Control**: enigo
- **Serialization**: serde, toml
- **Database**: SQLite (rusqlite)
- **Keyring**: keyring-rs

---

## Inspirations

- **Claude Code**: Conversation flow, tool use patterns
- **Gemini CLI**: Command structure, streaming output
- **Codex**: Code generation patterns
- **Warp Terminal**: TUI design, command palette
- **Raycast**: Quick launcher, extension system
- **Cursor**: IDE integration patterns
- **Continue.dev**: Context management
- **Aider**: Git integration
- **Open Interpreter**: Computer use patterns

---

## Non-Goals (Things We Won't Do)

- ❌ Ads or sponsored content
- ❌ Telemetry without explicit opt-in
- ❌ Cloud-required features (everything works offline)
- ❌ Subscription lock-in
- ❌ Vendor lock-in to any provider
- ❌ Clippy-style annoying interruptions
- ❌ "Smart" features that guess wrong
