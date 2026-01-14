# Ganesha (Rust)

The high-performance Rust implementation of Ganesha - The Remover of Obstacles.

## Building from Source

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# With optional features
cargo build --release --features vision    # Screen capture
cargo build --release --features voice     # Voice conversation
cargo build --release --features full      # All features
```

## Features

### Core
- Natural language to system commands
- Local LLM support (LM Studio, Ollama)
- Safe execution with user consent
- Session rollback support

### MCP Integration
Connect to external tool servers:
- **Playwright** - Browser automation
- **Fetch** - HTTP requests
- **Filesystem** - File operations
- **Custom** - Any MCP-compatible server

### Web Search
Built-in search without external tools:
```bash
ganesha "search for rust async best practices"
```
Uses Brave Search API (if `BRAVE_SEARCH_API_KEY` set) or DuckDuckGo fallback.

### Session Management
```bash
ganesha --last      # Resume last session context
ganesha --sessions  # Browse session history
```

### Flux Capacitor
Time-boxed autonomous execution:
```bash
ganesha --flux "1h" "write unit tests for all functions"
ganesha --until "5:00 PM" "refactor the database module"
```

## Configuration

Config file: `~/.config/ganesha/config.toml`

```toml
[providers]
primary = "lm_studio"
fallback = ["ollama", "anthropic"]

[lm_studio]
endpoint = "http://localhost:1234/v1"

[mcp]
servers = ["playwright", "fetch"]
```

## CLI Reference

```
ganesha [OPTIONS] [TASK]

Options:
  -A, --auto              Auto-approve all commands (DANGEROUS)
      --code              Code generation mode
  -i, --interactive       Interactive REPL mode
      --no-interactive    Non-interactive mode
      --agent             Full coding assistant with tool use
  -r, --rollback          Rollback session
      --history           Show session history
      --last              Resume last session
      --sessions          Select from session history
      --provider          LLM provider (local/anthropic/openai)
      --flux <DURATION>   Run for duration (e.g., "1h", "30m")
      --until <TIME>      Run until time (e.g., "23:30")
      --temp <TEMP>       LLM temperature (0.0-2.0)
      --resume <SESSION>  Resume previous Flux session
      --install           Install ganesha system-wide
      --uninstall         Uninstall ganesha
      --debug             Show debug output
  -q, --quiet             Minimal output
      --bare              Raw output for scripting
  -h, --help              Print help
  -V, --version           Print version
```

## Architecture

```
src/
├── main.rs          # CLI entry point
├── core/            # Planning, execution, safety
├── providers/       # LLM provider abstraction
├── orchestrator/    # MCP, memory, providers
├── flux.rs          # Flux Capacitor (autonomous mode)
├── websearch.rs     # Web search integration
├── pretty.rs        # Terminal output formatting
└── cli/             # CLI utilities, consent handlers
```

## License

MIT License - See LICENSE file in the repository root.
