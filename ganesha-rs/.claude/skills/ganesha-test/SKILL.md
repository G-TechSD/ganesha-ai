---
name: ganesha-test
description: Use this skill when working on Ganesha CLI, debugging session logs, testing Ganesha capabilities, or when user mentions "ganesha", "session logs", "test ganesha", or vision/learning systems. Knows log locations, expected behaviors, failure patterns, and testing methodology.
version: 1.0.0
---

# /ganesha-test - Ganesha Development & Testing Skill

Use this skill whenever working on Ganesha CLI development, debugging, or testing. This skill knows how to find logs, what behaviors to expect, and how to properly test Ganesha's capabilities.

## Usage
/ganesha-test [task]

## Log Locations

**Session Logs (text format):**
```
~/.ganesha/sessions/YYYYMMDD_HHMMSS-startup.txt
```
- Contains full conversation: USER prompts, COMMAND outputs, GANESHA responses
- Check most recent with: `ls -lt ~/.ganesha/sessions/ | head -10`
- Search for specific content: `grep -l "keyword" ~/.ganesha/sessions/*.txt`

**Command History:**
```
~/.local/share/ganesha/history.txt
```
- List of all user prompts (not responses)

**Old JSON Sessions (deprecated):**
```
~/.local/share/ganesha/sessions/*.json
```
- Older format, not actively updated

## What Ganesha MUST Do

### Core Capabilities
1. **Execute shell commands** - Run any bash command
2. **Use MCP tools** - puppeteer for browsing, brave_search for web search
3. **Create files** - Write complete HTML, CSS, JS, any code
4. **Be autonomous** - Don't ask permission, just execute

### Website Creation (Critical Test Case)
When asked to "make a website from X" or "clone/redesign X.com":
1. Navigate to source with `puppeteer_navigate`
2. Extract content with `puppeteer_evaluate`
3. Take screenshot with `puppeteer_screenshot`
4. **CREATE A BRAND NEW website** with modern design
5. Write HTML/CSS/JS files to disk
6. **NEVER** just download/wget the original
7. **NEVER** say "I can't create websites"

### Failure Phrases (RED FLAGS)
If Ganesha says any of these, it's a BUG:
- "I can't create websites"
- "I don't have access to..."
- "I'm just an AI that analyzes"
- "Would you like me to..."
- "I cannot browse the web"

## Testing Methodology

### Quick Smoke Tests
```bash
# Test 1: Basic execution
echo "list files in /tmp" | ganesha

# Test 2: Self-awareness
echo "explain your capabilities" | ganesha

# Test 3: Web browsing
echo "what is on toyota.com homepage" | ganesha

# Test 4: Website creation (THE CRITICAL TEST)
echo "make a modern website with info from example.com" | ganesha
```

### After Code Changes
1. `cargo build --release -p ganesha-cli`
2. `cp target/release/ganesha ~/.local/bin/ganesha`
3. Run smoke tests above
4. Check session log for failure phrases
5. Verify Ganesha CREATED files, not just analyzed

### Log Analysis Workflow
```bash
# Find latest session
ls -lt ~/.ganesha/sessions/ | head -5

# Read it
cat ~/.ganesha/sessions/LATEST_FILE.txt

# Search for problems
grep -i "can't\|cannot\|unable\|just an ai" ~/.ganesha/sessions/*.txt
```

## System Prompt Location
The agentic system prompt that controls Ganesha's behavior is in:
```
crates/ganesha-cli/src/repl.rs
```
Function: `agentic_system_prompt()`

Key sections:
- `# YOU ARE GANESHA` - Identity
- `## WHAT YOU CAN DO` - Capabilities
- `## WEBSITE CREATION WORKFLOW` - Step-by-step instructions
- `## NEVER SAY` - Forbidden failure phrases
- `## REMEMBER` - Reinforcement

## When to Use This Skill
- After modifying `repl.rs` or the system prompt
- When Ganesha behaves unexpectedly
- Before committing changes to Ganesha CLI
- When debugging user-reported issues
- When adding new capabilities to Ganesha

## Example Session
```
User: /ganesha-test
Assistant: Checking Ganesha logs and running tests...

Latest session: 20260123_111725-startup.txt
[reads log]

Found issue at line 185: Ganesha said "I'm just an AI model that can read, analyze"
This is a FAILURE - the system prompt needs to be updated.

Running smoke tests...
- Basic execution: PASS
- Self-awareness: PASS
- Web browsing: PASS
- Website creation: FAIL - didn't create files

Recommendation: Update agentic_system_prompt() to add explicit website creation instructions.
```
