# Visual Computer Use

## Overview

Ganesha can see your screen and control your computer like a human would - reading UI elements, clicking buttons, typing text, and navigating applications. This enables automation of GUI tasks that can't be done via command line.

---

## Quick Start

```bash
# Enable vision for a task
ganesha --vision "open Blender and create a new cube"

# Desktop app automatically enables vision when needed
ganesha-desktop
```

---

## How It Works

```
┌────────────────────────────────────────────────────────────────┐
│                     VISION PIPELINE                            │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│   Screen Capture (60fps)                                       │
│         │                                                      │
│         ▼                                                      │
│   Vision Model (VLA)                                           │
│   "I see a Blender window with the default cube selected"      │
│         │                                                      │
│         ▼                                                      │
│   Action Planning                                              │
│   "Click File menu → New → General"                            │
│         │                                                      │
│         ▼                                                      │
│   Input Execution                                              │
│   [Mouse moves to (120, 45), clicks]                           │
│         │                                                      │
│         ▼                                                      │
│   Verification                                                 │
│   "File menu opened successfully"                              │
│         │                                                      │
│         └──── Loop until task complete                         │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

---

## Visual Indicators

When Ganesha is controlling your screen:

### Screen Border

A subtle green border appears around the entire screen:

```
╔══════════════════════════════════════════════════════════════╗
║                                                              ║
║                    Your normal screen                        ║
║                                                              ║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝
 ▲
 └── Green border indicates Ganesha has control
```

### Status Indicator

The desktop app shows current action:

```
┌─────────────────────────────────────────┐
│  🟢 GANESHA IS REMOVING OBSTACLES       │
│  ─────────────────────────────────────  │
│  Opening Blender preferences...         │
│                                         │
│  [Stop Control]                         │
└─────────────────────────────────────────┘
```

---

## Supported Actions

### Mouse

| Action | Description |
|--------|-------------|
| Move | Move cursor to coordinates or element |
| Click | Left click |
| Right-click | Context menu |
| Double-click | Open/activate |
| Drag | Click and drag |
| Scroll | Vertical/horizontal scroll |

### Keyboard

| Action | Description |
|--------|-------------|
| Type | Type text naturally |
| Key | Press single key |
| Shortcut | Key combinations (Ctrl+C, etc) |
| Hold | Hold key while doing something |

### Complex

| Action | Description |
|--------|-------------|
| Select | Click and drag to select |
| Navigate | Follow UI paths (Menu → Item → Submenu) |
| Wait | Wait for element/state |
| Assert | Verify screen state |

---

## Vision Models

### Cloud Options

| Model | Best For | Latency |
|-------|----------|---------|
| GPT-4V | Complex UI understanding | ~2-3s |
| Claude 3.5 Sonnet | Accurate element detection | ~1-2s |
| Gemini 1.5 Pro | Fast multi-turn | ~1-2s |

### Local Options

| Model | Size | Quality | Speed |
|-------|------|---------|-------|
| Qwen-VL-Chat | 10B | Good | Fast |
| LLaVA 1.6 | 13B | Good | Medium |
| CogVLM | 17B | Excellent | Slower |

```bash
# Use specific vision model
ganesha --config set vision.model "gpt-4v"
ganesha --config set vision.model "local:qwen-vl"
```

---

## App Safety System

### Whitelist

Apps Ganesha CAN control:

```toml
# ~/.ganesha/config.toml
[vision.whitelist]
apps = [
    "Blender",
    "Bambu Studio",
    "CapCut",
    "OBS Studio",
    "Firefox",
    "Chrome",
    "Terminal",
    "VS Code",
    "Finder",
    "File Manager",
]
```

### Blacklist

Apps Ganesha must NEVER touch:

```toml
[vision.blacklist]
apps = [
    "1Password",
    "Bitwarden",
    "LastPass",
    "Keychain Access",
    "Banking *",      # Wildcard
    "* Bank",         # Wildcard
    "Authenticator",
    "Signal",
    "WhatsApp",
]
```

### Allow All Override

```bash
# Bypass safety for specific session
ganesha -A --vision "control my password manager"
# ⚠️ WARNING: You're allowing control of blacklisted apps!
# Type "I understand" to continue:
```

---

## Tested Applications

Ganesha has been specifically tested and optimized for:

### Creative Software

| App | Capabilities |
|-----|--------------|
| **Blender** | Model creation, material setup, rendering |
| **Bambu Studio** | Slice models, configure prints |
| **CapCut** | Video editing, effects, export |
| **OBS Studio** | Scene setup, streaming config |

### Development

| App | Capabilities |
|-----|--------------|
| **VS Code** | File navigation, editing, debugging |
| **Xcode** | Project setup, build, deploy |
| **Android Studio** | Emulator control, builds |

### System

| App | Capabilities |
|-----|--------------|
| **System Settings** | Configuration changes |
| **Finder/Explorer** | File management |
| **Terminal** | Command execution |

---

## TUI Application Control

Ganesha can also control terminal-based applications:

```bash
# Control htop
ganesha --vision "use htop to find the process using most memory and kill it"

# Control vim
ganesha --vision "open config.yaml in vim and change the port to 8080"

# Control tmux
ganesha --vision "create a new tmux session with 3 panes"
```

---

## Performance

### Capture Rate

```toml
[vision.capture]
fps = 60           # Frames per second
quality = "high"   # high, medium, low
region = "full"    # full, active_window, custom
```

### Action Speed

```toml
[vision.execution]
mouse_speed = "normal"  # slow, normal, fast, instant
type_speed = "natural"  # slow, natural, fast, instant
delay_between = 100     # ms between actions
```

### Safety Delays

```toml
[vision.safety]
confirm_destructive = true  # Ask before delete/close
verify_each_action = true   # Verify screen after each action
max_retries = 3             # Retry failed actions
```

---

## Permissions

### macOS

1. System Preferences → Privacy & Security
2. **Screen Recording**: Add Ganesha
3. **Accessibility**: Add Ganesha

### Linux

**X11**: Usually works out of box

**Wayland**:
```bash
# May need portal permissions
xdg-desktop-portal
```

### Windows

Run as Administrator for full input control, or:
```powershell
# Grant UI Automation permission
# Usually automatic on first use
```

---

## Example Tasks

### Create 3D Model

```bash
ganesha --vision "open Blender and create a low-poly tree"
```

Ganesha will:
1. Launch Blender
2. Delete default cube
3. Add cone for tree top
4. Add cylinder for trunk
5. Apply materials
6. Position camera
7. Render preview

### Configure OBS

```bash
ganesha --vision "set up OBS to record my screen at 1080p60"
```

Ganesha will:
1. Open OBS
2. Create new scene
3. Add display capture source
4. Configure output settings
5. Set hotkeys for recording

### Edit Video

```bash
ganesha --vision "open this video in CapCut and add subtitles"
```

Ganesha will:
1. Open CapCut
2. Import video
3. Use auto-caption feature
4. Adjust subtitle styling
5. Export

---

## Limitations

- **Speed**: Not faster than a human (yet)
- **Complex UIs**: May struggle with unusual layouts
- **Accessibility**: Works best with standard UI patterns
- **Small text**: May miss very small UI elements
- **Dynamic content**: Rapidly changing screens challenging

---

## See Also

- [Desktop App](../architecture/desktop.md)
- [Security Model](../architecture/security.md)
- [Risk Levels](risk-levels.md)
