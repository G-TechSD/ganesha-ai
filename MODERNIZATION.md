# Ganesha AI Modernization Plan

## Vision
Transform Ganesha from a GPT-4 dependent system administration tool into a **local-first, LLM-agnostic CLI** that integrates with Claudia for full autonomous development capabilities.

## What Ganesha Already Has (Ahead of its Time)
- Natural language to command translation
- User consent flow with explanation
- `--A` auto-approve mode (predates `--dangerously-skip-permissions`)
- Session logging and rollback
- Conversation history and feedback loops
- Cross-platform support (Linux, Mac, Windows)
- Interactive mode
- System context awareness
- Report generation

## What Needs Modernization

### Phase 1: Local LLM Support (Priority)
- [ ] Add LM Studio backend (OpenAI-compatible API)
- [ ] Add Ollama backend
- [ ] Make LLM provider configurable
- [ ] Fallback chain: Local -> Cloud
- [ ] Remove hard dependency on OpenAI SDK 0.28

### Phase 2: Code Generation Mode
- [ ] Add `--code` flag for code generation tasks
- [ ] Git integration (status, commit, branch)
- [ ] File read/write capabilities
- [ ] Project context awareness (package.json, etc.)
- [ ] Code diff preview before applying

### Phase 3: Claudia Integration
- [ ] Shared configuration with Claudia
- [ ] Execute Claudia packets via Ganesha
- [ ] Bidirectional: Claudia uses Ganesha for local execution
- [ ] Unified session management

### Phase 4: Installation Simplification
- [ ] Single binary distribution (PyInstaller)
- [ ] Homebrew formula
- [ ] Windows installer
- [ ] Minimal dependencies

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Ganesha CLI                          │
├─────────────────────────────────────────────────────────────┤
│  Natural Language Input                                      │
│  ↓                                                           │
│  LLM Provider (configurable)                                │
│  ├── Local: LM Studio / Ollama                              │
│  ├── Cloud: Anthropic / OpenAI / Google                     │
│  └── Hybrid: Try local first, fallback to cloud             │
│  ↓                                                           │
│  Command/Code Generation                                     │
│  ↓                                                           │
│  User Consent Flow (or --A to skip)                         │
│  ↓                                                           │
│  Execution + Logging                                         │
│  ↓                                                           │
│  Rollback Available                                          │
└─────────────────────────────────────────────────────────────┘
         ↕                                        ↕
┌─────────────────┐                    ┌─────────────────────┐
│   Claudia UI    │←──── Shared ──────→│  Claudia Agent      │
│   (Web Admin)   │      Config        │  Controller         │
└─────────────────┘                    └─────────────────────┘
```

## New Command Structure

```bash
# System administration (original mode)
ganesha "install docker and start it"
ganesha "find all large files over 1GB"
ganesha "configure nginx as reverse proxy"

# Code generation (new mode)
ganesha --code "add a login form to the React app"
ganesha --code "fix the TypeScript errors in src/utils"
ganesha --code "create unit tests for the API routes"

# Local LLM selection
ganesha --provider lmstudio "explain this error log"
ganesha --provider ollama --model codellama "refactor this function"

# Claudia integration
ganesha --claudia-packet PKT-001  # Execute a Claudia packet locally
ganesha --claudia-project hyperhealth "generate the dashboard"

# Interactive mode with local LLM
ganesha --interactive --provider local
```

## Configuration File (~/.ganesha/config.yaml)

```yaml
# LLM Providers
providers:
  default: local

  local:
    type: lmstudio
    url: http://192.168.245.155:1234
    model: openai/gpt-oss-20b

  backup:
    type: anthropic
    api_key: ${ANTHROPIC_API_KEY}
    model: claude-sonnet-4-20250514

# Behavior
auto_approve: false
max_retries: 3
timeout_seconds: 300

# Claudia Integration
claudia:
  enabled: true
  api_url: http://localhost:3000

# Logging
log_dir: ~/ganesha_logs
```

## Migration Path

### Step 1: Abstract LLM Interface
Create a provider interface that works with any LLM:
```python
class LLMProvider(ABC):
    @abstractmethod
    def generate(self, system: str, user: str, **kwargs) -> str:
        pass

class LMStudioProvider(LLMProvider):
    def generate(self, system: str, user: str, **kwargs) -> str:
        # OpenAI-compatible API call
        pass

class OllamaProvider(LLMProvider):
    # Similar implementation
    pass
```

### Step 2: Config-Driven Provider Selection
Load provider from config, allow CLI override.

### Step 3: Add Code Mode
New prompts and parsing for code generation.

### Step 4: Claudia Integration
Shared config loading, packet execution support.

## Timeline
- Phase 1: Immediate (local LLM support)
- Phase 2: This week (code generation)
- Phase 3: Next week (Claudia integration)
- Phase 4: Ongoing (simplify installation)

## Why "Ganesha"?
Ganesha is the Hindu deity known as the **Remover of Obstacles**. This tool removes the obstacles between your intentions and their execution on any system.

---

*Original concept and implementation: 2024, pre-dating Claude Code and OpenAI Codex CLI*
