# Risk Levels

## Overview

Ganesha uses a simple, human-readable risk level system. Unlike other AI tools that use confusing permission models, Ganesha's risk levels are designed so anyone can understand what they're allowing.

---

## The Four Levels

### 🟢 SAFE (`--safe`)

> *"I'll look but won't touch"*

**What it allows:**
- Read files and directories
- Search the web
- Analyze code
- Explain things
- Fetch web content

**What it blocks:**
- Writing any files
- Running any commands
- Making any system changes
- Installing anything

**Use when:**
- Exploring a new codebase
- Getting explanations
- Research tasks
- You're not sure what will happen

```bash
ganesha --safe "analyze this project structure"
ganesha --safe "explain what this script does"
```

---

### 🟡 NORMAL (default)

> *"I'll ask before anything risky"*

**What it allows automatically:**
- All SAFE operations
- Read-only commands (ls, cat, git status)
- Creating files in working directory

**What it asks permission for:**
- Installing packages
- Modifying system files
- Running commands with side effects
- Deleting files
- Using sudo

**Use when:**
- Normal day-to-day tasks
- You want control but don't want to approve everything
- Working on projects where mistakes are recoverable

```bash
ganesha "set up a new project"  # Will ask before installs
ganesha "clean up old files"     # Will ask before deletes
```

---

### 🟠 TRUSTED (`--trusted`)

> *"I'll handle routine tasks automatically"*

**What it allows automatically:**
- All NORMAL operations
- Package installations
- Git operations
- File management
- Service restarts (non-system)

**What it still asks for:**
- Sudo operations
- System configuration changes
- Destructive operations (rm -rf)
- Modifying critical files (/etc, /boot, etc)

**Use when:**
- You're doing hands-off work
- Tasks are well-defined
- You trust the LLM's judgment on routine ops
- Working in development environments

```bash
ganesha --trusted "update all dependencies"
ganesha --trusted "refactor and run tests"
```

---

### 🔴 YOLO (`-A` or `--yolo`)

> *"Full send, no questions asked"*

**What it allows:**
- Everything
- No confirmations
- No restrictions

**Use when:**
- Flux Capacitor time-boxed work
- You've reviewed the plan
- You're in a sandbox/container
- You absolutely know what you're doing

**Protections still in place:**
- Rollback always available
- Session logging
- Model quality warnings

```bash
ganesha -A "do whatever it takes to fix this"
ganesha --yolo "autonomous work for 2 hours"
```

---

## Visual Indicators

In the TUI and desktop app, risk levels are shown:

```
┌─────────────────────────────────────────┐
│  Risk: 🟡 NORMAL                        │
│  ─────────────────────────────────────  │
│  Ganesha will ask before:               │
│  • Installing packages                  │
│  • Modifying system files               │
│  • Deleting files                       │
│  • Using sudo                           │
│                                         │
│  [Change Level ▾]                       │
└─────────────────────────────────────────┘
```

---

## Setting Defaults

### Command Line

```bash
# For this session
ganesha --trusted "..."

# Persistent default
ganesha --config set risk.default trusted
```

### Config File (~/.ganesha/config.toml)

```toml
[risk]
default = "normal"  # safe, normal, trusted, yolo

# Per-directory overrides
[risk.overrides]
"~/projects/sandbox" = "yolo"
"~/projects/production" = "safe"
"/etc" = "safe"
"/" = "safe"
```

---

## Command Classification

Ganesha classifies commands into risk categories:

### Low Risk (auto-approved in NORMAL+)
```bash
ls, pwd, cat, head, tail, grep, find
git status, git log, git diff
echo, printf, date, whoami
```

### Medium Risk (asks in NORMAL, auto in TRUSTED)
```bash
mkdir, touch, cp, mv
npm install, pip install, cargo build
git add, git commit, git push
systemctl status, docker ps
```

### High Risk (asks in NORMAL/TRUSTED, auto in YOLO)
```bash
rm, rm -rf
sudo anything
apt install, brew install
systemctl start/stop/restart
chmod, chown
```

### Critical Risk (always warns, even in YOLO)
```bash
rm -rf /
dd if=... of=/dev/...
mkfs, fdisk
Anything touching /boot, /etc/passwd
```

---

## Emergency Stop

At any point, you can stop Ganesha:

- **Keyboard**: `Ctrl+C` (immediate stop)
- **Desktop**: Click the stop button
- **Voice**: Say "Stop" or "Cancel"

After stopping, you can:
- Review what was done
- Rollback changes
- Continue with different settings

---

## Rollback Protection

Regardless of risk level, Ganesha:

1. **Snapshots files** before modification
2. **Logs all commands** executed
3. **Enables rollback** for any session

```bash
# Oops, something went wrong
ganesha --rollback

# See what happened
ganesha --history
```

---

## Recommendations by Scenario

| Scenario | Recommended Level |
|----------|-------------------|
| Exploring new codebase | SAFE |
| Daily development | NORMAL |
| Automated CI/CD | TRUSTED |
| Sandbox testing | YOLO |
| Production systems | SAFE or NORMAL |
| Learning/demos | SAFE |
| Time-boxed autonomous work | TRUSTED or YOLO |

---

## See Also

- [Rollback System](rollback.md)
- [Security Model](../architecture/security.md)
- [Flux Capacitor](flux.md)
