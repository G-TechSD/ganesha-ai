# Voice Interface

## Overview

Ganesha's voice interface enables natural, conversational interaction. Talk to Ganesha like you would a human assistant, with support for different voices and personalities.

---

## Quick Start

```bash
# Start voice mode
ganesha --voice

# Or with specific settings
ganesha --voice --personality friendly
ganesha --voice --voice-model alloy
```

---

## Interaction Modes

### Push-to-Talk (Default)

Hold a key to speak:

| Platform | Default Key |
|----------|-------------|
| Desktop App | Big round button |
| Terminal | Space bar |
| Global Hotkey | Cmd/Ctrl + Shift + G |

```bash
# Configure PTT key
ganesha --config set voice.ptt_key "ctrl+shift+space"
```

### Wake Word

Activate by saying a wake phrase:

| Wake Phrase | Notes |
|-------------|-------|
| "Hey Ganesha" | Default |
| "Obstacle Remover" | Alternative |
| Custom | Set in config |

```bash
# Enable wake word
ganesha --config set voice.wake_word enabled

# Set custom wake word
ganesha --config set voice.wake_phrase "Computer"
```

### Always Listening

For hands-free operation (with privacy controls):

```bash
ganesha --voice --always-listen

# With visual indicator requirement
ganesha --config set voice.always_listen.require_indicator true
```

---

## Personalities

Choose how Ganesha responds:

### Professional (Default)

> Clear, direct, business-like

Good for: Work tasks, serious projects, presentations

```bash
ganesha --voice --personality professional
```

### Friendly

> Warm, encouraging, conversational

Good for: Learning, casual use, longer sessions

```bash
ganesha --voice --personality friendly
```

### Mentor

> Patient, explanatory, educational

Good for: Learning new skills, complex explanations

```bash
ganesha --voice --personality mentor
```

### Minimalist

> Terse, efficient, just the facts

Good for: Quick tasks, experienced users

```bash
ganesha --voice --personality minimalist
```

### Snarky

> Witty, playful, mildly sarcastic

Good for: Entertainment, making tedious tasks fun

```bash
ganesha --voice --personality snarky
```

### Custom

Define your own personality:

```toml
# ~/.ganesha/config.toml
[voice.personalities.custom]
name = "Pirate"
system_prompt = """
You are a helpful assistant who speaks like a pirate.
Use nautical terms and say 'arr' occasionally.
Still be helpful and accurate.
"""
voice_model = "fable"  # Storyteller voice
```

```bash
ganesha --voice --personality pirate
```

---

## Voice Models

### Cloud Voices (OpenAI)

| Voice | Character |
|-------|-----------|
| `alloy` | Neutral, balanced |
| `echo` | Warm, friendly |
| `fable` | Expressive, narrative |
| `onyx` | Deep, authoritative |
| `nova` | Energetic, upbeat |
| `shimmer` | Soft, gentle |

```bash
ganesha --config set voice.model alloy
```

### Local Voices

For offline use with local TTS:

| Engine | Quality | Speed |
|--------|---------|-------|
| Piper | Good | Fast |
| Coqui | Excellent | Medium |
| eSpeak | Basic | Very Fast |

```bash
# Use local TTS
ganesha --config set voice.engine local
ganesha --config set voice.local.model piper-en-us
```

---

## Conversation Features

### Barge-In

Interrupt Ganesha mid-sentence:

```
🗣️ You: "What's the capital of--"
🤖 Ganesha: "The capital of France is Paris, which is--"
🗣️ You: "No wait, what about Spain?"
🤖 Ganesha: "The capital of Spain is Madrid."
```

### Context Awareness

Ganesha remembers the conversation:

```
🗣️ You: "Create a new React project"
🤖 Ganesha: "I'll create a React project for you..."
🗣️ You: "Add TypeScript to it"
🤖 Ganesha: "Adding TypeScript to the React project..."
🗣️ You: "And install axios"
🤖 Ganesha: "Installing axios in the React TypeScript project..."
```

### Multi-Turn Tasks

Complex tasks with back-and-forth:

```
🗣️ You: "Help me debug this server issue"
🤖 Ganesha: "I'll check the logs. What service is having trouble?"
🗣️ You: "nginx"
🤖 Ganesha: "Checking nginx logs... I see a 502 error. Should I check the upstream service?"
🗣️ You: "Yes please"
🤖 Ganesha: "The Node.js process isn't running. Want me to start it?"
🗣️ You: "Go ahead"
🤖 Ganesha: "Started the service. Nginx is now responding correctly."
```

---

## Voice Commands

Special commands in voice mode:

| Say | Action |
|-----|--------|
| "Stop" / "Cancel" | Stop current operation |
| "Undo" / "Roll back" | Undo last action |
| "What did you do?" | Explain last action |
| "Repeat that" | Repeat last response |
| "Slower" / "Faster" | Adjust speech rate |
| "Switch to text" | Exit voice mode |
| "Mute" | Stop listening temporarily |

---

## Technical Details

### Latency

| Mode | Typical Latency |
|------|-----------------|
| OpenAI Realtime | ~200-400ms |
| Cloud STT + TTS | ~1-2 seconds |
| Local (Whisper + Piper) | ~500ms-1s |

### Audio Quality

```toml
# ~/.ganesha/config.toml
[voice.audio]
sample_rate = 24000      # Hz
channels = 1             # Mono
input_device = "default" # Or specific device name
output_device = "default"
noise_reduction = true
echo_cancellation = true
```

### Privacy

Voice data handling:

| Setting | Default | Description |
|---------|---------|-------------|
| Local STT | Off | Process speech locally |
| Transcript logging | On | Save text transcripts |
| Audio logging | Off | Save audio recordings |
| Cloud processing | On | Use cloud STT/TTS |

```bash
# Maximum privacy mode
ganesha --config set voice.engine local
ganesha --config set voice.logging.audio false
ganesha --config set voice.logging.transcript false
```

---

## Troubleshooting

### No Audio Input

```bash
# List audio devices
ganesha --voice --list-devices

# Set specific device
ganesha --config set voice.input_device "MacBook Pro Microphone"
```

### Voice Not Recognized

- Speak clearly
- Reduce background noise
- Adjust sensitivity:
  ```bash
  ganesha --config set voice.sensitivity 0.7
  ```

### High Latency

- Switch to local processing for faster response
- Check network connection for cloud services
- Use a faster TTS model

---

## See Also

- [Desktop App](../architecture/desktop.md)
- [Configuration](../getting-started/configuration.md)
- [Privacy Settings](../architecture/security.md)
