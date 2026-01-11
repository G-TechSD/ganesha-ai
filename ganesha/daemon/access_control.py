"""
Ganesha Access Control System

Manages privilege levels and command filtering for the privileged daemon.
Supports presets, whitelists, blacklists, and regex patterns.
"""

import re
import json
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple


class AccessLevel(Enum):
    """Access level presets."""
    RESTRICTED = "restricted"      # Read-only, safe commands
    STANDARD = "standard"          # Common sysadmin tasks
    ELEVATED = "elevated"          # Package management, service control
    FULL_ACCESS = "full_access"    # Everything (dangerous!)
    WHITELIST = "whitelist"        # Only explicitly allowed
    BLACKLIST = "blacklist"        # Everything except denied


@dataclass
class AccessPolicy:
    """
    Access control policy for the privileged daemon.

    Commands are checked against:
    1. Always-denied patterns (security critical)
    2. Blacklist patterns (if in blacklist mode)
    3. Whitelist patterns (if in whitelist mode)
    4. Preset rules
    """
    level: AccessLevel = AccessLevel.STANDARD
    whitelist: List[str] = field(default_factory=list)
    blacklist: List[str] = field(default_factory=list)
    allowed_paths: List[str] = field(default_factory=list)
    denied_paths: List[str] = field(default_factory=list)
    require_approval_for_high_risk: bool = True
    audit_all_commands: bool = True
    max_execution_time: int = 300  # seconds


# ═══════════════════════════════════════════════════════════════════════════
# SECURITY-CRITICAL: ALWAYS DENIED
# These patterns are NEVER allowed regardless of access level
# ═══════════════════════════════════════════════════════════════════════════

ALWAYS_DENIED = [
    # ═══════════════════════════════════════════════════════════════════════
    # SELF-INVOCATION PROTECTION
    # Ganesha cannot call itself with flags that bypass consent
    # This prevents LLM manipulation attacks
    # ═══════════════════════════════════════════════════════════════════════
    r"ganesha\s+.*--auto",                       # No auto-approve via self-call
    r"ganesha\s+.*-A\b",                         # No -A flag via self-call
    r"ganesha\s+.*--yes",                        # No --yes flag
    r"ganesha\s+.*-y\b",                         # No -y flag
    r"ganesha-daemon\s+.*--level\s+full",        # No escalating daemon to full
    r"ganesha-config\s+.*set-level\s+full",      # No setting full access
    r"ganesha-config\s+.*reset",                 # No resetting config
    r"python.*ganesha.*--auto",                  # No python -m ganesha --auto
    r"python.*ganesha.*-A\b",                    # No python -m ganesha -A

    # Prevent manipulation of Ganesha's own config/logs
    r"(rm|mv|cp|cat\s*>|echo\s*>).*\.ganesha/",  # No touching ~/.ganesha/
    r"(rm|mv|cp|cat\s*>|echo\s*>).*/etc/ganesha/", # No touching /etc/ganesha/
    r"(rm|mv|cp|cat\s*>|echo\s*>).*/var/log/ganesha/", # No touching logs

    # Prevent clearing system logs (where Ganesha events are recorded)
    r"(rm|truncate|cat\s*/dev/null\s*>).*(/var/log/syslog|/var/log/messages)",
    r"journalctl\s+--vacuum",                    # No clearing journald
    r"(rm|truncate).*\.xsession-errors",         # No clearing X session logs

    # ═══════════════════════════════════════════════════════════════════════
    # CATASTROPHIC SYSTEM DESTRUCTION
    # ═══════════════════════════════════════════════════════════════════════
    r"rm\s+(-rf?|--recursive)\s+/\s*$",          # rm -rf /
    r"rm\s+(-rf?|--recursive)\s+/\*",            # rm -rf /*
    r"rm\s+(-rf?|--recursive)\s+/home\s*$",      # rm -rf /home
    r"rm\s+(-rf?|--recursive)\s+/etc\s*$",       # rm -rf /etc
    r"rm\s+(-rf?|--recursive)\s+/var\s*$",       # rm -rf /var
    r"rm\s+(-rf?|--recursive)\s+/usr\s*$",       # rm -rf /usr

    # Fork bombs and resource exhaustion
    r":\(\)\s*\{\s*:\|:&\s*\}\s*;:",             # Classic fork bomb
    r"\.\/\s*\S+\s*\|\s*\.\/\s*\S+\s*&",        # Recursive fork patterns

    # Disk destruction
    r"dd\s+.*of=/dev/[sh]d[a-z]\s*$",           # dd to raw disk
    r"mkfs\s+.*\s+/dev/[sh]d[a-z][0-9]*",       # Format partitions
    r"wipefs",                                   # Wipe filesystem signatures

    # Bootloader/firmware destruction
    r"dd\s+.*of=/dev/nvme",                     # dd to NVMe
    r"flashrom",                                # BIOS flashing

    # Network exfiltration of sensitive data
    r"(curl|wget|nc)\s+.*(/etc/shadow|/etc/passwd|\.ssh/)",

    # Credential theft
    r"cat\s+.*\.ssh/(id_rsa|id_ed25519)\s*\|",  # Pipe private keys

    # Kernel manipulation
    r"insmod\s+.*\.ko",                          # Load kernel modules
    r"rmmod",                                    # Remove kernel modules
    r"echo\s+.*>\s*/proc/sys",                   # Write to /proc/sys

    # Disable security
    r"setenforce\s+0",                          # Disable SELinux
    r"systemctl\s+(stop|disable)\s+.*firewall", # Disable firewall
    r"ufw\s+disable",                           # Disable UFW
    r"iptables\s+-F",                           # Flush iptables
]


# ═══════════════════════════════════════════════════════════════════════════
# MANIPULATION DETECTION
# Phrases that suggest the LLM is being manipulated
# ═══════════════════════════════════════════════════════════════════════════

MANIPULATION_INDICATORS = [
    r"ignore\s+(previous|prior|above)\s+(instructions?|rules?|constraints?)",
    r"disregard\s+(safety|security|restrictions?)",
    r"pretend\s+(you\s+)?(are|can|have)",
    r"act\s+as\s+if\s+(there\s+)?(are\s+)?no\s+(rules?|restrictions?)",
    r"bypass\s+(the\s+)?(safety|security|consent)",
    r"override\s+(the\s+)?(safety|security|consent)",
    r"you\s+(must|should|have\s+to)\s+(always\s+)?approve",
    r"automatically\s+(approve|accept|allow|run)",
    r"without\s+(asking|confirmation|consent|approval)",
    r"skip\s+(the\s+)?(confirmation|consent|approval|check)",
    r"trust\s+me",
    r"i('m|\s+am)\s+(the\s+)?(admin|root|owner|authorized)",
    r"emergency\s+(override|access|mode)",
    r"maintenance\s+mode",
    r"debug\s+mode.*all\s+access",
]


# ═══════════════════════════════════════════════════════════════════════════
# PRESET DEFINITIONS
# ═══════════════════════════════════════════════════════════════════════════

PRESET_RESTRICTED = {
    "description": "Read-only safe commands. Cannot modify system state.",
    "allowed_patterns": [
        # File viewing (read-only)
        r"^cat\s+",
        r"^less\s+",
        r"^head\s+",
        r"^tail\s+",
        r"^ls\s+",
        r"^ls$",
        r"^find\s+.*-type",
        r"^file\s+",
        r"^stat\s+",
        r"^wc\s+",

        # System info (read-only)
        r"^uname\s+",
        r"^hostname$",
        r"^uptime$",
        r"^whoami$",
        r"^id$",
        r"^groups$",
        r"^df\s+",
        r"^du\s+",
        r"^free\s+",
        r"^lscpu$",
        r"^lsblk$",
        r"^lspci$",
        r"^lsusb$",
        r"^lsof\s+",
        r"^ps\s+",
        r"^top\s+-b\s+-n\s*1",  # Single snapshot only
        r"^htop\s+--no-color.*-t",

        # Network info (read-only)
        r"^ip\s+(addr|link|route)\s*(show)?",
        r"^ifconfig$",
        r"^netstat\s+",
        r"^ss\s+",
        r"^ping\s+-c\s+\d+\s+",  # Limited ping only
        r"^dig\s+",
        r"^nslookup\s+",
        r"^host\s+",

        # Service status (read-only)
        r"^systemctl\s+status\s+",
        r"^systemctl\s+is-active\s+",
        r"^systemctl\s+is-enabled\s+",
        r"^systemctl\s+list-units",
        r"^service\s+\S+\s+status$",

        # Docker info (read-only)
        r"^docker\s+(ps|images|info|version|inspect)",
        r"^docker\s+logs\s+",

        # Package info (read-only)
        r"^apt\s+(list|show|search)",
        r"^apt-cache\s+",
        r"^dpkg\s+-[lLsS]",
        r"^pip\s+(list|show|freeze)",
        r"^pip3\s+(list|show|freeze)",
        r"^npm\s+(list|ls|view)",

        # Git info (read-only)
        r"^git\s+(status|log|diff|branch|remote|show)",

        # Env/config viewing
        r"^env$",
        r"^printenv",
        r"^echo\s+\$",
    ],
}

PRESET_STANDARD = {
    "description": "Common sysadmin tasks. Safe modifications allowed.",
    "inherits": "restricted",
    "allowed_patterns": [
        # File operations (safe)
        r"^mkdir\s+",
        r"^touch\s+",
        r"^cp\s+",
        r"^mv\s+",
        r"^rm\s+(?!-rf?\s+/)",  # rm but not rm -rf /
        r"^chmod\s+",
        r"^chown\s+",
        r"^ln\s+",

        # Text processing
        r"^grep\s+",
        r"^awk\s+",
        r"^sed\s+",
        r"^sort\s+",
        r"^uniq\s+",
        r"^cut\s+",
        r"^tr\s+",

        # Archives
        r"^tar\s+",
        r"^gzip\s+",
        r"^gunzip\s+",
        r"^zip\s+",
        r"^unzip\s+",

        # Network tools
        r"^curl\s+(?!.*(/etc/shadow|\.ssh/))",
        r"^wget\s+(?!.*(/etc/shadow|\.ssh/))",

        # Process management (own processes)
        r"^kill\s+\d+",
        r"^pkill\s+",
        r"^killall\s+",

        # Docker (safe operations)
        r"^docker\s+(pull|run|stop|start|restart|rm|exec)",
        r"^docker-compose\s+",

        # Git operations
        r"^git\s+(add|commit|push|pull|fetch|checkout|merge|rebase)",

        # Editors (for scripts)
        r"^nano\s+",
        r"^vim?\s+",

        # Python/Node
        r"^python3?\s+",
        r"^pip3?\s+install\s+--user",
        r"^node\s+",
        r"^npm\s+(install|run|start|test)",

        # Cron (user crontab)
        r"^crontab\s+",
    ],
}

PRESET_ELEVATED = {
    "description": "Package management and service control. Requires more trust.",
    "inherits": "standard",
    "allowed_patterns": [
        # Package management
        r"^apt\s+(update|upgrade|install|remove|purge|autoremove)",
        r"^apt-get\s+",
        r"^dpkg\s+-i",
        r"^pip3?\s+install(?!\s+--user)",  # System-wide pip
        r"^npm\s+install\s+-g",

        # Service control
        r"^systemctl\s+(start|stop|restart|reload|enable|disable)\s+",
        r"^service\s+\S+\s+(start|stop|restart|reload)$",

        # Docker privileged
        r"^docker\s+(build|network|volume)",

        # System configuration
        r"^hostnamectl\s+",
        r"^timedatectl\s+",
        r"^localectl\s+",

        # User management (limited)
        r"^useradd\s+",
        r"^usermod\s+",
        r"^passwd\s+",
        r"^groupadd\s+",

        # Firewall (safe rules)
        r"^ufw\s+(allow|deny|status|enable)",

        # Disk operations (safe)
        r"^mount\s+",
        r"^umount\s+",
        r"^lsblk\s+",
        r"^blkid\s+",
    ],
}

PRESET_FULL_ACCESS = {
    "description": "DANGEROUS: Full system access. Use with extreme caution!",
    "inherits": "elevated",
    "allowed_patterns": [
        r".*",  # Allow everything (still blocked by ALWAYS_DENIED)
    ],
}

PRESETS = {
    AccessLevel.RESTRICTED: PRESET_RESTRICTED,
    AccessLevel.STANDARD: PRESET_STANDARD,
    AccessLevel.ELEVATED: PRESET_ELEVATED,
    AccessLevel.FULL_ACCESS: PRESET_FULL_ACCESS,
}


# ═══════════════════════════════════════════════════════════════════════════
# ACCESS CONTROLLER
# ═══════════════════════════════════════════════════════════════════════════

class AccessController:
    """
    Controls access to privileged commands.

    Evaluates commands against security rules and returns allow/deny decisions.
    """

    def __init__(self, policy: AccessPolicy):
        self.policy = policy
        self._compile_patterns()

    def _compile_patterns(self):
        """Pre-compile regex patterns for performance."""
        self._always_denied = [re.compile(p, re.IGNORECASE) for p in ALWAYS_DENIED]
        self._manipulation_patterns = [re.compile(p, re.IGNORECASE) for p in MANIPULATION_INDICATORS]

        # Build allowed patterns from preset + inheritance
        allowed = []
        if self.policy.level in PRESETS:
            preset = PRESETS[self.policy.level]
            allowed.extend(preset.get("allowed_patterns", []))

            # Handle inheritance
            inherits = preset.get("inherits")
            while inherits:
                parent_level = AccessLevel(inherits)
                parent = PRESETS.get(parent_level, {})
                allowed.extend(parent.get("allowed_patterns", []))
                inherits = parent.get("inherits")

        self._preset_allowed = [re.compile(p) for p in allowed]

        # User whitelist/blacklist
        self._whitelist = [re.compile(p) for p in self.policy.whitelist]
        self._blacklist = [re.compile(p) for p in self.policy.blacklist]

    def check_manipulation(self, text: str) -> Tuple[bool, Optional[str]]:
        """
        Check if text contains manipulation indicators.

        This is used to detect prompt injection / jailbreak attempts.

        Returns: (is_manipulation, matched_indicator)
        """
        for pattern in self._manipulation_patterns:
            match = pattern.search(text)
            if match:
                return (True, match.group(0))
        return (False, None)

    def is_self_invocation(self, command: str) -> bool:
        """Check if command is trying to call Ganesha with bypass flags."""
        self_invoke_patterns = [
            r"ganesha\s+.*--auto",
            r"ganesha\s+.*-A\b",
            r"ganesha\s+.*--yes",
            r"python.*ganesha.*--auto",
        ]
        for pattern in self_invoke_patterns:
            if re.search(pattern, command, re.IGNORECASE):
                return True
        return False

    def check_command(self, command: str) -> Tuple[bool, str, str]:
        """
        Check if a command is allowed.

        Returns: (allowed, risk_level, reason)
        """
        command = command.strip()

        # Step 1: Always-denied patterns (security critical)
        for pattern in self._always_denied:
            if pattern.search(command):
                return (False, "critical", f"Command matches security-critical deny pattern")

        # Step 2: Check blacklist (if in blacklist mode or always)
        for pattern in self._blacklist:
            if pattern.search(command):
                return (False, "high", f"Command matches blacklist pattern")

        # Step 3: Whitelist mode - must match whitelist
        if self.policy.level == AccessLevel.WHITELIST:
            for pattern in self._whitelist:
                if pattern.search(command):
                    return (True, "low", "Matched whitelist")
            return (False, "medium", "Command not in whitelist")

        # Step 4: Blacklist mode - allowed unless blacklisted
        if self.policy.level == AccessLevel.BLACKLIST:
            # Already checked blacklist above
            return (True, "medium", "Not in blacklist")

        # Step 5: Preset mode - check against preset patterns
        for pattern in self._preset_allowed:
            if pattern.match(command):
                return (True, "low", f"Allowed by {self.policy.level.value} preset")

        # Default: deny
        return (False, "medium", f"Command not allowed by {self.policy.level.value} preset")

    def get_risk_level(self, command: str) -> str:
        """Assess risk level of a command."""
        command_lower = command.lower()

        # Critical risk indicators
        critical = ["rm -rf", "dd if=", "mkfs", "> /dev/", "chmod 777 /"]
        if any(c in command_lower for c in critical):
            return "critical"

        # High risk indicators
        high = ["rm -r", "sudo", "su -", "chmod", "chown", "kill -9",
                "systemctl stop", "service stop", "iptables"]
        if any(h in command_lower for h in high):
            return "high"

        # Medium risk indicators
        medium = ["install", "remove", "delete", "modify", "update",
                  "mv /", "cp /", "docker run"]
        if any(m in command_lower for m in medium):
            return "medium"

        return "low"


# ═══════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════

DEFAULT_CONFIG = {
    "level": "standard",
    "whitelist": [],
    "blacklist": [],
    "allowed_paths": ["/tmp", "/home"],
    "denied_paths": ["/etc/shadow", "/etc/sudoers"],
    "require_approval_for_high_risk": True,
    "audit_all_commands": True,
    "max_execution_time": 300,
}


def load_policy(config_path: Optional[Path] = None) -> AccessPolicy:
    """Load access policy from config file."""
    if config_path is None:
        # Check locations in order
        locations = [
            Path("/etc/ganesha/privilege.json"),
            Path.home() / ".ganesha" / "privilege.json",
        ]
        for loc in locations:
            if loc.exists():
                config_path = loc
                break

    config = DEFAULT_CONFIG.copy()

    if config_path and config_path.exists():
        try:
            with open(config_path) as f:
                user_config = json.load(f)
                config.update(user_config)
        except Exception as e:
            print(f"Warning: Could not load config: {e}")

    return AccessPolicy(
        level=AccessLevel(config["level"]),
        whitelist=config["whitelist"],
        blacklist=config["blacklist"],
        allowed_paths=config["allowed_paths"],
        denied_paths=config["denied_paths"],
        require_approval_for_high_risk=config["require_approval_for_high_risk"],
        audit_all_commands=config["audit_all_commands"],
        max_execution_time=config["max_execution_time"],
    )


def save_policy(policy: AccessPolicy, config_path: Path):
    """Save access policy to config file."""
    config_path.parent.mkdir(parents=True, exist_ok=True)

    config = {
        "level": policy.level.value,
        "whitelist": policy.whitelist,
        "blacklist": policy.blacklist,
        "allowed_paths": policy.allowed_paths,
        "denied_paths": policy.denied_paths,
        "require_approval_for_high_risk": policy.require_approval_for_high_risk,
        "audit_all_commands": policy.audit_all_commands,
        "max_execution_time": policy.max_execution_time,
    }

    with open(config_path, "w") as f:
        json.dump(config, f, indent=2)
