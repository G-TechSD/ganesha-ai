# Competitor Analysis Summary

Research conducted for Ganesha 4.0 architecture decisions.

## Tools Researched

1. **Aider** - AI pair programming CLI
2. **Gemini CLI** - Google's AI coding assistant
3. **Goose** - Block's open-source AI agent
4. **Crush CLI / OpenCode** - Charmbracelet's TUI-based assistant

---

## Aider

### Key Features
- **Chat Modes**: Code, Ask, Architect, Help
  - Architect mode separates reasoning from editing (two-model approach)
  - Ask mode for discussion without file changes
- **Git Integration**: Automatic commits, dirty file handling, `/undo`, `/diff`
- **Multi-Provider**: OpenAI, Anthropic, Gemini, Groq, Ollama, LM Studio, OpenRouter
- **Codebase Mapping**: Large project support with context awareness
- **Voice-to-Code**: Hands-free interaction
- **Image Support**: Visual context for prompts

### UX Patterns to Adopt
- `/chat-mode` switching with per-message overrides
- Automatic commit messages with Co-authored-by
- Model aliases for convenient naming
- Watch mode for IDE integration (responds to inline comments)

### Metrics
- 39K GitHub stars
- 4.1M pip installations
- 15B tokens/week usage

---

## Gemini CLI

### Key Features
- **Custom Commands**: TOML-based in `.gemini/commands/`
  - User-scoped (`~/.gemini/commands/`) and project-scoped
  - Namespaced commands (`/git:commit` from `git/commit.toml`)
  - `{{args}}` substitution support
- **Token Caching**: 10% cost for cached tokens, 2K-1M+ range
- **Session Management**: Auto-save, `/resume`, `/rewind`
- **Checkpointing**: Git snapshots before file modifications
- **MCP Support**: Multiple transports (stdio, SSE, HTTP)

### UX Patterns to Adopt
- TOML custom command format
- `/stats` for token usage monitoring
- `/compress` for conversation summarization
- Team-shareable command files in repo
- Trust configuration for auto-approval

### Command Syntax
```
/command  - Slash commands (meta-level control)
@file     - Context injection
!command  - Shell execution
```

---

## Goose (Block)

### Key Features
- **MCP Extensibility**: "USB-C of AI integrations"
  - Stdio, SSE transports
  - Dynamic tool discovery
  - Extension trait pattern
- **Interactive Loop**: Request → LLM → Tool Call → Execute → Iterate
- **Knowledge Graph Memory**: Persistent context across sessions
- **Recipes**: Structured workflow definitions

### Architecture Insights
- Extensions implement: `name()`, `description()`, `tools()`, `call_tool()`, `status()`
- Tool design: Action-oriented naming, JSON Schema, errors as prompts
- Session-based extension loading via CLI flags

### Patterns to Adopt
- Dynamic tool auto-discovery
- Context revision (remove old/irrelevant info)
- Recipe-based workflow orchestration
- HTTP-based remote extension support

---

## Crush CLI / OpenCode

### Key Features
- **Elm Architecture TUI**: Model → Update → View
  - Bubble Tea framework (Go) / Ratatui equivalent (Rust)
  - Lipgloss-style declarative styling
- **LSP Integration**: gopls, typescript-language-server, etc.
- **Session Management**: SQLite persistence
- **Cross-Device Resume**: Sessions portable across machines

### TUI Patterns to Adopt
- Central `appModel` orchestrating all UI
- Message routing for inter-component communication
- Responsive layouts adapting to terminal size
- Overlay layering for dialogs/popups

### Configuration
- Multi-level precedence: project → root → global
- `.crush.json` / `.opencode.json` for project config
- Vim keybindings support

---

## TUI Best Practices (from Ratatui research)

### Architecture
- **MVC Pattern**: Model (state), View (render), Controller (events)
- **Message Passing**: Map keys to Command/Action enums
- **Non-blocking Events**: Separate input polling from rendering

### Event Handling
- Centralized polling with dispatch
- Platform-specific handling (Windows sends Press+Release)
- Async with crossterm's event-stream

### Layout
- Constraint-based responsive design (Percentage, Length, Min/Max)
- Hierarchical decomposition
- `Min(0)` as last constraint prevents expansion

### Performance
- Immediate-mode rendering (redraw entire UI each frame)
- Sub-millisecond rendering with zero-cost abstractions

### Color Schemes
- 60-30-10 rule (primary/secondary/accent)
- JSON-based theme configuration
- Light/dark mode switching

---

## Recommendations for Ganesha 4.0

### High Priority
1. **Chat Modes**: Code/Ask/Architect pattern from Aider
2. **TOML Custom Commands**: From Gemini CLI
3. **MCP Protocol**: From Goose
4. **Session Persistence**: SQLite with cross-session resume
5. **Model Tiers**: Warn about dangerous models

### Medium Priority
6. **Token Caching/Stats**: From Gemini CLI
7. **Git Integration**: Auto-commits with `/undo`
8. **Knowledge Graph**: Persistent memory
9. **Responsive TUI**: Ratatui with Elm architecture

### Unique Ganesha Features
10. **Risk Levels**: Safe/Normal/Trusted/Yolo
11. **Rollback System**: File snapshots before changes
12. **Mini-Me Subagents**: Parallel task execution
13. **Vision/VLA**: GUI automation for desktop apps
14. **Voice Interface**: Push-to-talk with personalities
15. **Desktop App**: "Obstacle Remover" with screen border indicator

---

## Sources

### Aider
- https://aider.chat/
- https://aider.chat/docs/
- https://aider.chat/docs/usage/modes.html
- https://aider.chat/2024/09/26/architect.html

### Gemini CLI
- https://github.com/google-gemini/gemini-cli
- https://geminicli.com/docs/
- https://geminicli.com/docs/cli/custom-commands/
- https://geminicli.com/docs/cli/checkpointing/

### Goose
- https://github.com/block/goose
- https://block.github.io/goose/docs/goose-architecture/
- https://block.github.io/goose/docs/getting-started/using-extensions/

### Crush/OpenCode
- https://github.com/charmbracelet/crush
- https://github.com/opencode-ai/opencode
- https://opencode.ai/docs/tui/

### Ratatui
- https://ratatui.rs/
- https://ratatui.rs/concepts/event-handling/
- https://github.com/ratatui/ratatui/discussions/220
