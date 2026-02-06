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

## VLA - Vision-Language-Action (Computer Use)

Ganesha sees your screen, understands it, and controls your desktop. Closed-loop GUI automation: **perceive -> plan -> act -> verify -> repeat**.

```bash
# Tell Ganesha what to do - it figures out how
ganesha vla "Open Firefox and navigate to github.com/G-TechSD" \
  --criteria "GitHub profile page is visible" \
  --max-actions 15 --save-screenshots

# Operate complex applications
ganesha vla "In Blender, switch to Scripting workspace and run the solar system script" \
  --app Blender --timeout 180
```

### How It Works

1. **Screen Capture** - Grabs the display at 1280x720, encodes as JPEG
2. **Vision Analysis** - Local VLM (ministral-3-14b) reads the screen: identifies UI elements, text, dialogs, window state
3. **Action Planning** - Plans the next single action: click, type, key press, scroll. Keyboard-first strategy (Ctrl+L, Ctrl+F, Tab) over mouse when possible
4. **Input Execution** - xdotool sends the input to X11 with proper key mapping and timing (80ms inter-key delay)
5. **Verification** - Captures again, compares before/after, detects if the action hit its target
6. **Task Memory** - SQLite database tracks every action, records failures, injects context into future planning prompts so Ganesha learns from mistakes

### Blender Benchmark: Astronomically Accurate Solar System

Ganesha's first major creative benchmark. Starting from a blank Blender 4.0 scene, Ganesha wrote and executed a Python script that procedurally generated a complete animated solar system:

- **The Sun** blazing at center with 80,000W emission lighting, golden glow radiating across the scene
- **All 8 planets** on Kepler-accurate orbits - Mercury whipping around in 145 frames while Neptune crawls through its 98,400-frame year. Each with procedural two-tone noise textures: Earth's blue-green oceans and continents, Mars's rusty iron oxide terrain, Jupiter's swirling bands
- **Saturn and Uranus rings** as flattened torus geometry with semi-transparent materials catching the sunlight
- **Earth's Moon** orbiting faithfully every 30 frames
- **150 asteroid belt objects** distributed in a Gaussian ring between Mars and Jupiter, each on its own Keplerian orbit
- **8 stray asteroids** on hyperbolic flythrough trajectories, tumbling as they streak past the inner planets
- **Orbital path visualizations** as dim glowing circles for each planet
- **Animated camera** slowly sweeping 60 degrees over the 20-second animation, locked onto the Sun
- **Procedural star field** via world shader noise with nebula backdrop

342 objects. 600 frames at 30fps. Astronomically proportional. One Python script, one command.

### Blender Benchmark: Black Hole - Journey Through the Event Horizon

The showstopper. A cinematic visualization of falling into a supermassive black hole:

- **Event horizon** as a perfect absorber - zero-emission black sphere that swallows everything
- **Multi-layered accretion disk** - five concentric torus rings with differential rotation (inner screaming around in 80 frames, outer drifting at 720). Each layer has turbulent noise-textured emission materials: the innermost ring blazes blue-white at the ISCO (innermost stable circular orbit), transitioning through orange plasma to deep red at the outer edge
- **Einstein ring** - a razor-thin torus of blinding white light bent around the photon sphere, plus a vertical ring showing light curved over the poles
- **Relativistic jets** - twin cones of blue-purple emission shooting from the poles, fading to transparency with gradient + noise turbulence
- **Photon sphere** with Fresnel edge-glow (light trapped just outside the horizon)
- **Gravitational lensing** approximated by a subtle glass refraction sphere
- **30 debris particles** spiraling inward on Bezier-interpolated paths, glowing orange as they're devoured
- **The camera flies IN**: starting 60 units out, it spirals through the disk plane, crosses the photon sphere, punches through the event horizon, and plunges toward the singularity. Inside, animated noise distortion visualizes spacetime tearing apart.
- **EEVEE bloom** makes every emission material glow like it should - the accretion disk practically burns through the screen

47 objects. 720 frames. 24 seconds of pure cosmic terror.

### Local Model Comparison (ministral-3-14b-reasoning)

The all-local pipeline (`blender_scripts/ministral_blender_pipeline.py`) tests whether a 14B parameter model running on local hardware can generate equivalent Blender scenes with zero cloud dependency:

- **Auto-generates** Blender Python scripts from natural language descriptions
- **Auto-patches** 15+ known Blender 4.0 API incompatibilities (deprecated nodes, renamed inputs, missing imports)
- **Iterative error correction** - feeds Blender tracebacks back to the model for self-repair (up to 5 attempts)
- **Multi-phase generation** (`ministral_push.py`) - geometry first, then materials, then animation, each phase building on verified working code

Results: ministral produces working scenes (192 lines, 4 materials, correct geometry) but lacks the cinematic depth of larger models (no turbulent noise textures, no transparency blending, no volumetric effects). The pipeline proves the concept - local-only creative generation is real and improving.

## Architecture

```
src/
├── main.rs              # CLI entry point
├── core/                # Planning, execution, safety
├── providers/           # LLM provider abstraction
├── orchestrator/        # MCP, memory, vision analysis
│   └── vision.rs        # Screen analysis via VLM
├── vla/                 # Vision-Language-Action (computer use)
│   ├── loop_controller.rs  # Closed-loop: capture→plan→act→verify
│   ├── action_planner.rs   # LLM-driven action planning
│   ├── element_locator.rs  # UI element coordinate estimation
│   └── task_db.rs          # SQLite task/action/failure tracking
├── input/               # X11 input simulation (xdotool + enigo)
├── vision/              # Screen capture (xcap → JPEG)
├── flux.rs              # Flux Capacitor (autonomous mode)
├── websearch.rs         # Web search integration
├── pretty.rs            # Terminal output formatting
└── cli/                 # CLI utilities, consent handlers

blender_scripts/         # Procedural 3D scene generators
├── solar_system.py      # Animated solar system (342 objects)
├── black_hole.py        # Black hole event horizon journey (47 objects)
├── ministral_blender_pipeline.py  # Local model → Blender pipeline
└── ministral_push.py    # Multi-phase local generation (push limits)
```

## License

MIT License - See LICENSE file in the repository root.
