# Code Generation Error Handling

## Error Categories

| Type | Detection | Recovery |
|------|-----------|----------|
| Syntax Error | Parser/compiler diagnostics | Re-prompt LLM with error context |
| Runtime Error | Try/catch in sandbox | Add defensive code, retry |
| Infinite Loop | Timeout (configurable) | Kill process, analyze loop condition |
| Import Error | Module not found exception | Auto-install or prompt user |
| Type Error | Static analysis or runtime | Add type hints, conversions |

## Detection Flow

```
Generated Code
     │
     ▼
┌─────────────┐
│ Syntax Check │ ──fail──▶ Re-prompt with error
└─────────────┘
     │ pass
     ▼
┌─────────────┐
│ Static Lint  │ ──warn──▶ Log warnings, continue
└─────────────┘
     │
     ▼
┌─────────────┐
│ Sandbox Exec │ ──timeout──▶ Kill, report loop
└─────────────┘
     │
     ▼
┌─────────────┐
│ Check Exit   │ ──error──▶ Capture stack, re-prompt
└─────────────┘
     │ success
     ▼
   Output
```

## Retry Policy

```rust
const MAX_SYNTAX_RETRIES: u32 = 3;
const MAX_RUNTIME_RETRIES: u32 = 2;
const EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
```

## Error Prompt Templates

### Syntax Error
```
The following code has syntax errors:
{code}

Error: {error_message} at line {line}

Please fix the syntax error and regenerate.
```

### Runtime Error
```
The code crashed with:
{stack_trace}

Input that caused the error: {input}

Please add error handling and fix the issue.
```

### Infinite Loop
```
The code exceeded the {timeout}s timeout, likely an infinite loop.

Suspicious code section:
{loop_code}

Please add a loop termination condition.
```

## Implementation Status
- [ ] Syntax validation (per-language parsers)
- [ ] Sandbox execution (Docker/gVisor)
- [ ] Timeout enforcement
- [ ] Error-aware re-prompting
- [ ] Stack trace parsing
- [ ] Auto-fix suggestions
