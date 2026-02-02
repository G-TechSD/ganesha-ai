---
name: ganesha-test
description: Use this skill when working on Ganesha CLI, debugging session logs, testing Ganesha capabilities, or when user mentions "ganesha", "session logs", "test ganesha", or vision/learning systems. Knows log locations, expected behaviors, failure patterns, and testing methodology.
version: 2.0.0
---

# /ganesha-test - Ganesha Development & Testing Skill

Use this skill whenever working on Ganesha CLI development, debugging, or testing.

## CRITICAL TESTING RULES (NEVER VIOLATE)

### Rule 1: TEST LIKE A REAL USER
- **NEVER** use `unset ANTHROPIC_API_KEY` or any environment workarounds
- **NEVER** modify environment before tests - test the DEFAULT experience
- If you need a workaround to make tests pass, THAT'S A BUG TO FIX

### Rule 2: CHECK STARTUP OUTPUT FIRST
Every test MUST verify the startup line:
```
🐘 Starting Ganesha... Ready!
  → N providers, M MCP servers, X tools
```
- Verify the CORRECT providers are listed
- Verify they're in the EXPECTED order
- If wrong provider appears, STOP and fix before proceeding

### Rule 3: CHECK SESSION LOGS AFTER EVERY TEST
```bash
# ALWAYS check the log after running ganesha
cat ~/.ganesha/sessions/$(ls -t ~/.ganesha/sessions/ | head -1)
```
Look for:
- `Providers:` line - are the right providers listed?
- Any failure phrases in GANESHA responses
- Commands that failed or weren't executed

### Rule 4: NO SILENT FAILURES
- Every test must have explicit PASS/FAIL criteria
- Log failures with specific details (line numbers, exact text)
- Don't continue testing if a fundamental issue is found

## Log Locations

**Session Logs (text format):**
```
~/.ganesha/sessions/YYYYMMDD_HHMMSS-startup.txt
```
- Contains full conversation: USER prompts, COMMAND outputs, GANESHA responses
- Check most recent with: `ls -lt ~/.ganesha/sessions/ | head -10`

**Provider Config:**
```
~/.config/ganesha/providers.toml
```
- Defines which providers to use and in what order
- First provider = Primary, second = Secondary

**Command History:**
```
~/.local/share/ganesha/history.txt
```

## What Ganesha MUST Do

### Core Capabilities
1. **Execute shell commands** - Run any bash command
2. **Use MCP tools** - puppeteer for browsing, brave_search for web search
3. **Create files** - Write complete HTML, CSS, JS, any code
4. **Be autonomous** - Don't ask permission, just execute

### Failure Phrases (RED FLAGS)
If Ganesha says any of these, it's a BUG:
- "I can't create websites"
- "I don't have access to..."
- "I'm just an AI that analyzes"
- "Would you like me to..."
- "I cannot browse the web"
- "I currently do not have access"

## Testing Methodology

### Pre-Test Checklist (MANDATORY)
Before running ANY tests:
1. Check `~/.config/ganesha/providers.toml` - know what's configured
2. Check environment: `echo $ANTHROPIC_API_KEY $OPENAI_API_KEY $GEMINI_API_KEY`
3. Run `ganesha --version` or quick test to see startup providers

### Test Categories

#### Category 1: Provider Configuration Tests
```bash
# Test 1.1: Verify correct providers load
echo "quit" | ganesha 2>&1 | head -5
# EXPECTED: Shows only providers from providers.toml in correct order

# Test 1.2: First provider should respond
echo "Who made you?" | ganesha
# EXPECTED: Response matches first provider (e.g., "Google" for Gemini)

# Test 1.3: Provider fallback works
# (Requires intentionally breaking first provider)
```

#### Category 2: Basic Execution Tests
```bash
# Test 2.1: Simple command
echo "list files in current directory" | ganesha

# Test 2.2: Command with output
echo "show disk usage with df -h" | ganesha

# Test 2.3: Multi-step task
echo "create a file called test.txt with 'hello world' then show its contents" | ganesha
```

#### Category 3: File Creation Tests
```bash
# Test 3.1: Single file creation
echo "create a Python script that prints fibonacci numbers" | ganesha

# Test 3.2: Multi-file creation
echo "create an HTML page with separate CSS file" | ganesha

# Test 3.3: Directory creation
echo "create a project folder with src and tests subdirectories" | ganesha
```

#### Category 4: Web/MCP Tests
```bash
# Test 4.1: Web navigation
echo "go to example.com and tell me what's there" | ganesha

# Test 4.2: Web search
echo "search the web for latest rust news" | ganesha

# Test 4.3: Screenshot
echo "take a screenshot of google.com" | ganesha
```

#### Category 5: Autonomy Tests
```bash
# Test 5.1: No permission asking
echo "install cowsay and run it" | ganesha
# FAIL if: Ganesha asks "Would you like me to..."

# Test 5.2: Complex autonomous task
echo "create a website about space, make it look modern" | ganesha
# FAIL if: Ganesha doesn't create actual files
```

### Post-Test Analysis (MANDATORY)
After EVERY test:
```bash
# 1. Get latest session
SESSION=$(ls -t ~/.ganesha/sessions/ | head -1)
echo "Session: $SESSION"

# 2. Check providers line
grep "^Providers:" ~/.ganesha/sessions/$SESSION

# 3. Check for failure phrases
grep -i "can't\|cannot\|unable\|just an ai\|would you like\|do not have access" ~/.ganesha/sessions/$SESSION

# 4. Count commands executed
grep -c "^\$ " ~/.ganesha/sessions/$SESSION
```

## After Code Changes

1. `cargo build --release -p ganesha-cli`
2. `cp target/release/ganesha ~/.local/bin/ganesha`
3. Run Category 1 tests FIRST (provider configuration)
4. Only proceed to other categories if Category 1 passes
5. Check session logs after EVERY test

## Fallback Providers

If cloud providers run out of credits:
- LM Studio at `192.168.245.115:1234` (gpt-oss-20b) - no usage cap
- Add to providers.toml:
```toml
[[providers]]
name = "lmstudio"
provider_type = "local"
base_url = "http://192.168.245.115:1234"
enabled = true
```

## System Prompt Location

The agentic system prompt that controls Ganesha's behavior:
```
crates/ganesha-cli/src/repl.rs
```
Function: `agentic_system_prompt()`

## Common Issues & Fixes

### Wrong provider being used
- Check: `Providers:` line in session log
- Fix: Ensure providers.toml has correct order, env vars aren't overriding

### Ganesha asking permission
- Check: Session log for "Would you like" or similar
- Fix: Update system prompt to be more autonomous

### Commands not executing
- Check: Session log for `$ ` prefixed lines
- Fix: Check if model is outputting valid JSON tool calls

### File creation failures
- Check: Did Ganesha actually run `cat > file` commands?
- Fix: System prompt may need clearer file creation instructions
