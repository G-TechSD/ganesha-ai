# Ganesha Project Context

## What is Ganesha?
Ganesha is an autonomous AI terminal assistant - "The Remover of Obstacles". It can:
- Execute any shell command
- Browse the web with puppeteer
- Create complete websites, applications, and scripts
- Accomplish virtually anything in the terminal

## Key Development Files

### System Prompt (CRITICAL)
```
ganesha-rs/ganesha4/crates/ganesha-cli/src/repl.rs
```
Function `agentic_system_prompt()` controls Ganesha's behavior and identity.

### Build & Install
```bash
cd ganesha-rs/ganesha4
cargo build --release -p ganesha-cli
cp target/release/ganesha ~/.local/bin/ganesha
```
Or use: `just install` (if justfile exists)

## ALWAYS Use /ganesha-test

**When working on Ganesha CLI or vision systems, ALWAYS run `/ganesha-test` first to:**
1. Check the latest session logs for failures
2. Understand what the user tested
3. Find any "I can't do X" failure phrases
4. Run smoke tests after changes

## Session Log Locations

| Type | Path | Format |
|------|------|--------|
| Session Logs | `~/.ganesha/sessions/*.txt` | Text with timestamps |
| Command History | `~/.local/share/ganesha/history.txt` | Plain list |
| Old Sessions | `~/.local/share/ganesha/sessions/*.json` | JSON (deprecated) |

## Common Failure Patterns

If Ganesha says any of these, the system prompt needs fixing:
- "I can't create websites"
- "I don't have access to..."
- "I'm just an AI that analyzes"
- "Would you like me to..."

## Testing After Changes

1. Build: `cargo build --release -p ganesha-cli`
2. Install: `cp target/release/ganesha ~/.local/bin/ganesha`
3. Test: `echo "make a website about cats" | ganesha`
4. Check log: `cat ~/.ganesha/sessions/$(ls -t ~/.ganesha/sessions/ | head -1)`
5. Verify: Ganesha CREATED files, didn't just analyze
