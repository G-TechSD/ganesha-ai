# Ganesha Learning System

A learning-from-demonstration system with persistent memory for Ganesha. This crate enables Ganesha to watch user actions, learn patterns, and generalize skills to similar situations across different applications.

## Overview

The key insight: **Show Ganesha how to navigate Blender menus once, and it can navigate similar menus in GIMP, OBS, or any other application by generalizing the pattern.**

This is what makes Ganesha different from hardcoded automation tools - it learns and adapts.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Ganesha Learning System                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │   Capture    │    │   Learning   │    │  Controller  │     │
│  │   Module     │───▶│    Engine    │───▶│    (VLA)     │     │
│  │  (xcap)      │    │              │    │              │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│         │                   │                   │               │
│         ▼                   ▼                   ▼               │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │    Input     │    │   Database   │    │   Overlay    │     │
│  │  Simulator   │    │   (SQLite)   │    │  (Visual)    │     │
│  │  (enigo)     │    │              │    │              │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│                             │                                   │
│                             ▼                                   │
│                    ┌──────────────┐                            │
│                    │    Model     │                            │
│                    │ Integration  │                            │
│                    │ (Vision LLM) │                            │
│                    └──────────────┘                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Modules

### `capture` - Screen Capture
- Multi-monitor support via xcap
- Region and window capture
- Configurable image compression (PNG/JPEG/WebP)
- Screenshot buffer for sustained capture at 1fps+
- Base64 encoding for model input

### `db` - SQLite Persistence
- Stores demonstrations, skills, UI patterns, sessions
- Skill application logging with success/failure tracking
- Search and filtering capabilities
- Thread-safe with connection pooling

### `learning` - Learning Engine
- Recording demonstrations (mouse, keyboard, screenshots)
- Skill extraction from demonstrations
- Pattern matching to find relevant skills
- Skill application to new contexts
- Statistics and outcome tracking

### `model` - Vision Model Integration
- Support for local LLMs (ministral-3b, etc.)
- Dual-model approach: planning model + vision model
- Screen analysis and UI element detection
- Action planning based on visual context

### `overlay` - Desktop Overlay
- Red border indicates Ganesha is in control
- Status window shows current action/progress
- Self-identification so Ganesha ignores its own UI

### `controller` - Vision-Action Loop
- Main orchestrator for autonomous control
- Configurable speed modes (slow/normal/fast/beast)
- Safety checks and emergency stop (ESC key)
- Continuous capture → analyze → plan → execute → verify cycle

## CLI Commands

```bash
# Recording demonstrations
ganesha vision record <app-name> -d "description"
ganesha vision stop

# Managing skills
ganesha vision skills              # List all skills
ganesha vision skills -a Blender   # Filter by app
ganesha vision skill <id>          # Show skill details
ganesha vision delete <id>         # Delete a skill

# Testing and status
ganesha vision test                # Test capture system
ganesha vision status              # Show system status
ganesha vision capture             # Capture and save screenshot
ganesha vision stats               # Show learning statistics

# Autonomous control (WIP)
ganesha vision control "task description" -s fast
```

## Usage Example

```rust
use ganesha_learning::{Database, LearningEngine};
use ganesha_learning::db::MouseButton;

#[tokio::main]
async fn main() -> Result<(), ganesha_learning::Error> {
    // Initialize
    let db = Database::open("ganesha_learning.db")?;
    let engine = LearningEngine::new(db);

    // Start recording a demonstration
    let session_id = engine.start_recording("Blender", "Navigate to render settings")?;

    // Record user actions (in practice, from input monitoring)
    engine.record_click(100, 50, MouseButton::Left)?;
    engine.record_click(150, 100, MouseButton::Left)?;
    engine.record_text("settings")?;

    // Stop and extract skill
    let demo = engine.stop_recording()?;
    let skill = engine.extract_skill(&demo, "Navigate menu hierarchy")?;

    // Later, find and apply relevant skills
    let screenshot = capture_screen()?;
    let matches = engine.find_relevant_skills("open preferences", &screenshot)?;

    if let Some(best_match) = matches.first() {
        let actions = engine.apply_skill(&best_match.skill, &screenshot)?;
        // Execute actions...
    }

    Ok(())
}
```

## Database Schema

### Tables

| Table | Purpose |
|-------|---------|
| `demonstrations` | Recorded user demonstrations |
| `skills` | Extracted reusable skills |
| `ui_patterns` | Learned UI element patterns |
| `sessions` | Recording sessions |
| `skill_applications` | Log of skill usage with outcomes |

### Key Fields

**Demonstration:**
- `id`, `timestamp`, `app_name`, `task_description`
- `screenshots` (JSON array of base64)
- `actions` (JSON array of RecordedAction)
- `outcome`, `duration_ms`, `tags`, `notes`

**Skill:**
- `id`, `name`, `description`, `learned_from`
- `trigger_patterns` (when to suggest this skill)
- `action_template` (parameterized actions)
- `applicable_apps`, `confidence`, `success_count`

## Speed Modes

| Mode | Actions/sec | Use Case |
|------|-------------|----------|
| Slow | 0.5 | Debugging, demonstrations |
| Normal | 1.0 | Standard automation |
| Fast | 2.0 | Efficient automation |
| Beast | 5.0 | Maximum speed (use carefully) |

## Safety Features

1. **ESC to Stop**: Press Escape to immediately halt automation
2. **Red Border Overlay**: Visual indicator when Ganesha controls screen
3. **Action Limits**: Maximum actions per task
4. **Confidence Threshold**: Only apply skills above threshold
5. **Dry Run Mode**: Test without executing actions

## Testing

```bash
# Run all tests
cargo test -p ganesha-learning

# Run with display (for capture tests)
DISPLAY=:1 cargo test -p ganesha-learning

# Run specific test
cargo test -p ganesha-learning test_skill_matching

# Run example
DISPLAY=:1 cargo run -p ganesha-learning --example basic_test
```

## Configuration

Environment variables:
- `GANESHA_LEARNING_DB`: Custom database path
- `DISPLAY`: X11 display for screen capture

## Dependencies

- **rusqlite**: SQLite database
- **xcap**: Cross-platform screen capture
- **enigo**: Cross-platform input simulation
- **image**: Image processing
- **tokio**: Async runtime
- **serde**: Serialization

## Future Work

- [ ] Platform-specific overlay backends (X11, Wayland, Windows, macOS)
- [ ] Vision model fine-tuning on demonstrations
- [ ] Cross-application skill transfer learning
- [ ] Natural language skill invocation
- [ ] Collaborative skill sharing
