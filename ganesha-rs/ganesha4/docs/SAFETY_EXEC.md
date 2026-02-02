# Shell Execution Safety Design

## Risk Matrix

| Risk | Severity | Mitigation |
|------|----------|------------|
| Destructive commands (rm -rf, chmod) | Critical | Command whitelist + confirmation |
| Privilege escalation | Critical | Least-privilege, no sudo by default |
| Data exfiltration | High | File access filtering, output sanitization |
| Resource exhaustion | Medium | Timeouts, memory limits, rate limiting |
| Injection attacks | High | Input validation, parameterized commands |

## Ganesha Risk Levels (from GANESHA_4.0_DESIGN.md)

### Safe (default)
- Read-only file operations
- Non-destructive git commands
- Status/info queries

### Normal
- File creation/modification
- Package installs (with confirmation)
- Git commits

### Trusted
- System configuration changes
- Service management
- Network operations

### YOLO
- All operations permitted
- User accepts all risk
- Audit logging still active

## Implementation Checklist

- [x] Command parsing and validation
- [x] Risk level classification
- [ ] Sandbox execution (container/VM)
- [ ] Confirmation prompts for risky commands
- [ ] Output sanitization (secrets)
- [ ] Resource limits (timeout, memory)
- [ ] Audit logging with full context
- [ ] Rollback integration for destructive ops

## Dangerous Command Patterns

```
rm -rf /
chmod 777
> /etc/
curl | sh
wget | bash
sudo rm
mkfs
dd if=
```

## Safe Command Allow-list

```
ls, cat, head, tail, less, grep, find, wc
git status, git log, git diff, git branch
npm list, cargo check, rustc --version
pwd, whoami, env, echo
```
