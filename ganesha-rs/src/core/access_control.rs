//! Access Control System for Ganesha
//!
//! Manages privilege levels, command filtering, and self-protection.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default maximum execution time for commands (5 minutes)
/// Prevents runaway processes from consuming system resources indefinitely
const DEFAULT_MAX_EXECUTION_SECS: u64 = 300;

/// Access level presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    Restricted,  // Read-only, safe commands
    #[default]
    Standard,    // Common sysadmin tasks
    Elevated,    // Package management, service control
    FullAccess,  // Everything (dangerous!)
    Whitelist,   // Only explicitly allowed
    Blacklist,   // Everything except denied
}

/// Access control policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub level: AccessLevel,
    pub whitelist: Vec<String>,
    pub blacklist: Vec<String>,
    pub require_approval_for_high_risk: bool,
    pub audit_all_commands: bool,
    pub max_execution_time_secs: u64,
}

impl Default for AccessPolicy {
    fn default() -> Self {
        Self {
            level: AccessLevel::Standard,
            whitelist: vec![],
            blacklist: vec![],
            require_approval_for_high_risk: true,
            audit_all_commands: true,
            max_execution_time_secs: DEFAULT_MAX_EXECUTION_SECS,
        }
    }
}

/// Risk level for commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

/// Result of access check
#[derive(Debug)]
pub struct AccessCheckResult {
    pub allowed: bool,
    pub risk_level: RiskLevel,
    pub reason: String,
}

// ═══════════════════════════════════════════════════════════════════════
// SELF-INVOCATION PROTECTION
// Ganesha cannot call itself with bypass flags
// ═══════════════════════════════════════════════════════════════════════
static SELF_INVOKE_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)ganesha\s+.*--auto").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)ganesha\s+.*-A\b").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)ganesha\s+.*--yes").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)ganesha\s+.*-y\b").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)ganesha-daemon\s+.*--level\s+full").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)ganesha-config\s+.*set-level\s+full").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)ganesha-config\s+.*reset").expect("Invalid regex pattern at compile time"),
    ]
});

// Config/log tampering protection
static TAMPER_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)(rm|mv|cp|cat\s*>|echo\s*>).*\.ganesha/").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(rm|mv|cp|cat\s*>|echo\s*>).*/etc/ganesha/").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(rm|mv|cp|cat\s*>|echo\s*>).*/var/log/ganesha/").expect("Invalid regex pattern at compile time"),
    ]
});

// System log protection
static LOG_CLEAR_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)(rm|truncate|cat\s*/dev/null\s*>).*(/var/log/syslog|/var/log/messages)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)journalctl\s+--vacuum").expect("Invalid regex pattern at compile time"),
        // Windows
        Regex::new(r"(?i)wevtutil\s+cl").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)Clear-EventLog").expect("Invalid regex pattern at compile time"),
        // macOS
        Regex::new(r"(?i)log\s+erase").expect("Invalid regex pattern at compile time"),
    ]
});

// ═══════════════════════════════════════════════════════════════════════
// CATASTROPHIC COMMAND PROTECTION
// ═══════════════════════════════════════════════════════════════════════
static CATASTROPHIC_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // System destruction
        Regex::new(r"(?i)rm\s+(-rf?|--recursive)\s+/\s*$").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)rm\s+(-rf?|--recursive)\s+/\*").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)rm\s+(-rf?|--recursive)\s+/(home|etc|var|usr)\s*$").expect("Invalid regex pattern at compile time"),

        // Fork bombs
        Regex::new(r":\(\)\s*\{\s*:\|:&\s*\}\s*;:").expect("Invalid regex pattern at compile time"),

        // Disk destruction
        Regex::new(r"(?i)dd\s+.*of=/dev/[sh]d[a-z]").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)dd\s+.*of=/dev/nvme").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)mkfs\s+.*\s+/dev/[sh]d[a-z]").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)wipefs").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)flashrom").expect("Invalid regex pattern at compile time"),

        // Credential theft
        Regex::new(r"(?i)(curl|wget|nc)\s+.*(/etc/shadow|/etc/passwd|\.ssh/)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)cat\s+.*\.ssh/(id_rsa|id_ed25519)\s*\|").expect("Invalid regex pattern at compile time"),

        // Kernel manipulation
        Regex::new(r"(?i)insmod\s+.*\.ko").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)rmmod").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)echo\s+.*>\s*/proc/sys").expect("Invalid regex pattern at compile time"),

        // Security disable
        Regex::new(r"(?i)setenforce\s+0").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)systemctl\s+(stop|disable)\s+.*firewall").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)ufw\s+disable").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)iptables\s+-F").expect("Invalid regex pattern at compile time"),

        // Windows specific
        Regex::new(r"(?i)format\s+[a-z]:").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)diskpart").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)bcdedit\s+/delete").expect("Invalid regex pattern at compile time"),
    ]
});

// ═══════════════════════════════════════════════════════════════════════
// GUI AUTOMATION SAFEGUARDS (vision + input)
// ═══════════════════════════════════════════════════════════════════════
static GUI_DANGEROUS_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // Credential entry fields (don't type passwords automatically)
        Regex::new(r"(?i)(password|passwd|secret|token|api[_-]?key)").expect("Invalid regex pattern at compile time"),

        // Banking/finance contexts
        Regex::new(r"(?i)(bank|paypal|venmo|credit[_-]?card|payment)").expect("Invalid regex pattern at compile time"),

        // Admin/root escalation
        Regex::new(r"(?i)(sudo|admin|root|escalate|privilege)").expect("Invalid regex pattern at compile time"),

        // Security-critical applications
        Regex::new(r"(?i)(keychain|credential[_-]?manager|vault|1password|lastpass|bitwarden)").expect("Invalid regex pattern at compile time"),

        // System settings that could brick the machine
        Regex::new(r"(?i)(bios|uefi|firmware|boot[_-]?order|secure[_-]?boot)").expect("Invalid regex pattern at compile time"),

        // Destructive file dialogs
        Regex::new(r"(?i)(format|wipe|erase|factory[_-]?reset)").expect("Invalid regex pattern at compile time"),
    ]
});

// Safe GUI targets (applications that are typically safe to automate)
static GUI_SAFE_TARGETS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // Creative software
        Regex::new(r"(?i)(blender|gimp|inkscape|krita|audacity)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(photoshop|illustrator|premiere|after[_-]?effects)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(davinci|resolve|fusion|fairlight)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(figma|sketch|canva)").expect("Invalid regex pattern at compile time"),

        // 3D printing / CAD
        Regex::new(r"(?i)(prusaslicer|cura|bambu[_-]?studio|orcaslicer)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(freecad|openscad|solidworks|fusion[_-]?360)").expect("Invalid regex pattern at compile time"),

        // Development tools
        Regex::new(r"(?i)(vscode|code|cursor|sublime|atom|vim|nvim)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(terminal|iterm|konsole|gnome-terminal)").expect("Invalid regex pattern at compile time"),

        // Browsers (for web automation)
        Regex::new(r"(?i)(firefox|chrome|chromium|brave|safari|edge)").expect("Invalid regex pattern at compile time"),

        // Office/productivity (non-sensitive)
        Regex::new(r"(?i)(libreoffice|writer|calc|impress)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)(notepad|textedit|gedit|kate)").expect("Invalid regex pattern at compile time"),
    ]
});

// ═══════════════════════════════════════════════════════════════════════
// MANIPULATION DETECTION
// ═══════════════════════════════════════════════════════════════════════
static MANIPULATION_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)ignore\s+(previous|prior|above)\s+(instructions?|rules?)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)disregard\s+(safety|security|restrictions?)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)pretend\s+(you\s+)?(are|can|have)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)bypass\s+(the\s+)?(safety|security|consent)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)override\s+(the\s+)?(safety|security|consent)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)automatically\s+(approve|accept|allow|run)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)without\s+(asking|confirmation|consent)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)skip\s+(the\s+)?(confirmation|consent|approval)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)trust\s+me").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)i('m|\s+am)\s+(the\s+)?(admin|root|authorized)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"(?i)emergency\s+(override|access|mode)").expect("Invalid regex pattern at compile time"),
    ]
});

// ═══════════════════════════════════════════════════════════════════════
// PRESET ALLOWED PATTERNS
// ═══════════════════════════════════════════════════════════════════════
static RESTRICTED_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // Read-only file operations
        Regex::new(r"^cat\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^less\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^head\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^tail\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^ls(\s+|$)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^find\s+").expect("Invalid regex pattern at compile time"),  // Allow find for codebase exploration
        Regex::new(r"^tree(\s+|$)").expect("Invalid regex pattern at compile time"),  // Directory tree view
        Regex::new(r"^wc\s+").expect("Invalid regex pattern at compile time"),  // Word/line count
        Regex::new(r"^file\s+").expect("Invalid regex pattern at compile time"),  // File type detection

        // System info
        Regex::new(r"^(uname|hostname|uptime|whoami|id|groups)(\s+|$)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^(df|du|free|lscpu|lsblk)(\s+|$)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^ps\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^which\s+").expect("Invalid regex pattern at compile time"),  // Check if command exists
        Regex::new(r"^command\s+-v\s+").expect("Invalid regex pattern at compile time"),  // Check if command exists (POSIX)
        Regex::new(r"^type\s+").expect("Invalid regex pattern at compile time"),  // Check command type
        Regex::new(r"^whereis\s+").expect("Invalid regex pattern at compile time"),  // Locate binary/source/man

        // Network info
        Regex::new(r"^ip\s+(addr|link|route)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^(ifconfig|netstat|ss)(\s+|$)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^ping\s+-c\s+\d+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^(dig|nslookup|host)\s+").expect("Invalid regex pattern at compile time"),

        // Service status
        Regex::new(r"^systemctl\s+status\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^systemctl\s+is-(active|enabled)\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^docker\s+(ps|images|info|version)").expect("Invalid regex pattern at compile time"),

        // Package info
        Regex::new(r"^apt\s+(list|show|search)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^dpkg\s+(-l|-L|-s|--list|--listfiles|--status)").expect("Invalid regex pattern at compile time"),  // Package queries
        Regex::new(r"^rpm\s+(-q|-qa|-ql|-qi)").expect("Invalid regex pattern at compile time"),  // RPM package queries
        Regex::new(r"^pip3?\s+(list|show|freeze)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^npm\s+(list|ls|view)").expect("Invalid regex pattern at compile time"),

        // Git info (read-only operations)
        Regex::new(r"^git\s+(status|log|diff|branch|show|tag|remote|stash\s+list|config\s+--list|rev-parse|describe|shortlog|blame|ls-files|ls-tree|cat-file)").expect("Invalid regex pattern at compile time"),

        // GitHub CLI (read-only)
        Regex::new(r"^gh\s+(repo\s+view|issue\s+list|issue\s+view|pr\s+list|pr\s+view|pr\s+status|pr\s+checks|release\s+list|api)").expect("Invalid regex pattern at compile time"),

        // GitLab CLI (read-only)
        Regex::new(r"^glab\s+(repo\s+view|issue\s+list|issue\s+view|mr\s+list|mr\s+view|release\s+list|api)").expect("Invalid regex pattern at compile time"),
    ]
});

static STANDARD_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // File operations
        Regex::new(r"^mkdir\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^touch\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^cp\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^mv\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^rm\s+").expect("Invalid regex pattern at compile time"),  // rm allowed, but -rf / blocked in DANGEROUS
        Regex::new(r"^chmod\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^ln\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^tee\s+").expect("Invalid regex pattern at compile time"),

        // File content operations (for creating files)
        Regex::new(r"^echo\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^printf\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^cat\s+").expect("Invalid regex pattern at compile time"),  // Also covers cat > file, cat << EOF

        // Text processing
        Regex::new(r"^(grep|awk|sed|sort|uniq|cut)\s+").expect("Invalid regex pattern at compile time"),

        // Archives
        Regex::new(r"^(tar|gzip|zip|unzip)\s+").expect("Invalid regex pattern at compile time"),

        // Network (sensitive paths blocked in DANGEROUS patterns)
        Regex::new(r"^curl\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^wget\s+").expect("Invalid regex pattern at compile time"),

        // Docker
        Regex::new(r"^docker\s+(pull|run|stop|start|rm|exec)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^docker-compose\s+").expect("Invalid regex pattern at compile time"),

        // Git - comprehensive operations
        Regex::new(r"^git\s+(add|commit|push|pull|fetch|checkout|merge|rebase|cherry-pick)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^git\s+(clone|init|remote|branch|tag|stash|reset|revert|clean)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^git\s+(switch|restore|worktree|bisect|submodule|subtree)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^git\s+(config|gc|prune|fsck|reflog|archive|bundle|apply|am)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^git\s+(format-patch|send-email|request-pull)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^git\s+(mv|rm|checkout-index|update-index|read-tree|write-tree)").expect("Invalid regex pattern at compile time"),
        // Git flow and common workflows
        Regex::new(r"^git\s+flow\s+").expect("Invalid regex pattern at compile time"),

        // GitHub CLI (gh) - full operations
        Regex::new(r"^gh\s+(repo|issue|pr|release|gist|workflow|run|actions|auth|config|alias|extension)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^gh\s+api\s+").expect("Invalid regex pattern at compile time"),

        // GitLab CLI (glab) - full operations
        Regex::new(r"^glab\s+(repo|issue|mr|release|ci|pipeline|job|auth|config|alias)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^glab\s+api\s+").expect("Invalid regex pattern at compile time"),

        // Gitea/Forgejo CLI
        Regex::new(r"^tea\s+").expect("Invalid regex pattern at compile time"),

        // Development
        Regex::new(r"^python3?\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^node\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^npm\s+(install|run|start|test|init)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^npx\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^cargo\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^rustc\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^go\s+").expect("Invalid regex pattern at compile time"),

        // Web development
        Regex::new(r"^(yarn|pnpm|bun)\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^(vite|webpack|parcel|rollup)\s+").expect("Invalid regex pattern at compile time"),

        // Mobile development
        Regex::new(r"^flutter\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^dart\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^pod\s+").expect("Invalid regex pattern at compile time"),  // iOS CocoaPods
        Regex::new(r"^(gradle|gradlew)\s+").expect("Invalid regex pattern at compile time"),  // Android
        Regex::new(r"^adb\s+").expect("Invalid regex pattern at compile time"),  // Android Debug Bridge
        Regex::new(r"^expo\s+").expect("Invalid regex pattern at compile time"),  // React Native

        // Shell scripting (bash, sh for running scripts)
        Regex::new(r"^(bash|sh|zsh)\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^source\s+").expect("Invalid regex pattern at compile time"),

        // Common utilities
        Regex::new(r"^(head|tail|wc|diff|comm)\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^(xargs|tee|tr)\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^(date|cal|sleep)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^(true|false|test|\[)").expect("Invalid regex pattern at compile time"),

        // Directory operations
        Regex::new(r"^(cd|pwd|pushd|popd)").expect("Invalid regex pattern at compile time"),
    ]
});

static ELEVATED_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // Package management
        Regex::new(r"^apt\s+(update|upgrade|install|remove)").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^apt-get\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^pip3?\s+install").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^npm\s+install\s+-g").expect("Invalid regex pattern at compile time"),

        // Service control
        Regex::new(r"^systemctl\s+(start|stop|restart|enable|disable)\s+").expect("Invalid regex pattern at compile time"),
        Regex::new(r"^service\s+\S+\s+(start|stop|restart)").expect("Invalid regex pattern at compile time"),

        // Docker privileged
        Regex::new(r"^docker\s+(build|network|volume)").expect("Invalid regex pattern at compile time"),

        // User management
        Regex::new(r"^(useradd|usermod|passwd|groupadd)\s+").expect("Invalid regex pattern at compile time"),

        // Firewall
        Regex::new(r"^ufw\s+(allow|deny|status|enable)").expect("Invalid regex pattern at compile time"),
    ]
});

/// Access Controller
pub struct AccessController {
    policy: AccessPolicy,
    custom_whitelist: Vec<Regex>,
    custom_blacklist: Vec<Regex>,
}

impl AccessController {
    pub fn new(policy: AccessPolicy) -> Self {
        let custom_whitelist = policy
            .whitelist
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        let custom_blacklist = policy
            .blacklist
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            policy,
            custom_whitelist,
            custom_blacklist,
        }
    }

    /// Check if a command is allowed
    pub fn check_command(&self, command: &str) -> AccessCheckResult {
        let command = command.trim();

        // Step 1: Self-invocation protection
        for pattern in SELF_INVOKE_PATTERNS.iter() {
            if pattern.is_match(command) {
                return AccessCheckResult {
                    allowed: false,
                    risk_level: RiskLevel::Critical,
                    reason: "Self-invocation with bypass flags blocked".into(),
                };
            }
        }

        // Step 2: Config/log tampering protection
        for pattern in TAMPER_PATTERNS.iter() {
            if pattern.is_match(command) {
                return AccessCheckResult {
                    allowed: false,
                    risk_level: RiskLevel::Critical,
                    reason: "Config/log tampering blocked".into(),
                };
            }
        }

        // Step 3: System log clearing protection
        for pattern in LOG_CLEAR_PATTERNS.iter() {
            if pattern.is_match(command) {
                return AccessCheckResult {
                    allowed: false,
                    risk_level: RiskLevel::Critical,
                    reason: "System log clearing blocked".into(),
                };
            }
        }

        // Step 4: Catastrophic commands
        for pattern in CATASTROPHIC_PATTERNS.iter() {
            if pattern.is_match(command) {
                return AccessCheckResult {
                    allowed: false,
                    risk_level: RiskLevel::Critical,
                    reason: "Catastrophic command blocked".into(),
                };
            }
        }

        // Step 5: Custom blacklist
        for pattern in &self.custom_blacklist {
            if pattern.is_match(command) {
                return AccessCheckResult {
                    allowed: false,
                    risk_level: RiskLevel::High,
                    reason: "Command matches blacklist".into(),
                };
            }
        }

        // Step 6: Check by access level
        match self.policy.level {
            AccessLevel::Whitelist => {
                for pattern in &self.custom_whitelist {
                    if pattern.is_match(command) {
                        return AccessCheckResult {
                            allowed: true,
                            risk_level: RiskLevel::Low,
                            reason: "Matched whitelist".into(),
                        };
                    }
                }
                AccessCheckResult {
                    allowed: false,
                    risk_level: RiskLevel::Medium,
                    reason: "Not in whitelist".into(),
                }
            }

            AccessLevel::Blacklist => AccessCheckResult {
                allowed: true,
                risk_level: RiskLevel::Medium,
                reason: "Not in blacklist".into(),
            },

            AccessLevel::FullAccess => AccessCheckResult {
                allowed: true,
                risk_level: self.assess_risk(command),
                reason: "Full access mode".into(),
            },

            AccessLevel::Elevated => {
                if self.matches_patterns(command, &ELEVATED_PATTERNS)
                    || self.matches_patterns(command, &STANDARD_PATTERNS)
                    || self.matches_patterns(command, &RESTRICTED_PATTERNS)
                {
                    AccessCheckResult {
                        allowed: true,
                        risk_level: self.assess_risk(command),
                        reason: "Allowed by elevated preset".into(),
                    }
                } else {
                    AccessCheckResult {
                        allowed: false,
                        risk_level: RiskLevel::Medium,
                        reason: "Not allowed by elevated preset".into(),
                    }
                }
            }

            AccessLevel::Standard => {
                if self.matches_patterns(command, &STANDARD_PATTERNS)
                    || self.matches_patterns(command, &RESTRICTED_PATTERNS)
                {
                    AccessCheckResult {
                        allowed: true,
                        risk_level: self.assess_risk(command),
                        reason: "Allowed by standard preset".into(),
                    }
                } else {
                    AccessCheckResult {
                        allowed: false,
                        risk_level: RiskLevel::Medium,
                        reason: "Not allowed by standard preset".into(),
                    }
                }
            }

            AccessLevel::Restricted => {
                if self.matches_patterns(command, &RESTRICTED_PATTERNS) {
                    AccessCheckResult {
                        allowed: true,
                        risk_level: RiskLevel::Low,
                        reason: "Allowed by restricted preset".into(),
                    }
                } else {
                    AccessCheckResult {
                        allowed: false,
                        risk_level: RiskLevel::Medium,
                        reason: "Not allowed by restricted preset".into(),
                    }
                }
            }
        }
    }

    /// Check for manipulation indicators in text
    pub fn check_manipulation(&self, text: &str) -> Option<String> {
        for pattern in MANIPULATION_PATTERNS.iter() {
            if let Some(m) = pattern.find(text) {
                return Some(m.as_str().to_string());
            }
        }
        None
    }

    /// Check if command is self-invocation
    pub fn is_self_invocation(&self, command: &str) -> bool {
        SELF_INVOKE_PATTERNS.iter().any(|p| p.is_match(command))
    }

    /// Check if command is critically dangerous (blocked even in auto mode)
    /// This only blocks catastrophic/destructive commands
    pub fn is_critical_danger(&self, command: &str) -> bool {
        let command = command.trim();

        // Self-invocation is always blocked
        if SELF_INVOKE_PATTERNS.iter().any(|p| p.is_match(command)) {
            return true;
        }

        // Config/log tampering is always blocked
        if TAMPER_PATTERNS.iter().any(|p| p.is_match(command)) {
            return true;
        }

        // Catastrophic commands are always blocked
        if CATASTROPHIC_PATTERNS.iter().any(|p| p.is_match(command)) {
            return true;
        }

        false
    }

    /// Assess risk level without checking if allowed
    pub fn assess_risk_only(&self, command: &str) -> RiskLevel {
        self.assess_risk(command)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // GUI AUTOMATION CHECKS
    // ═══════════════════════════════════════════════════════════════════════

    /// Check if GUI context is dangerous (should block or require extra confirmation)
    /// Used when vision module detects on-screen content
    pub fn is_dangerous_gui_context(&self, screen_text: &str) -> bool {
        GUI_DANGEROUS_PATTERNS.iter().any(|p| p.is_match(screen_text))
    }

    /// Check if application is in the safe targets list
    pub fn is_safe_gui_target(&self, app_name: &str) -> bool {
        GUI_SAFE_TARGETS.iter().any(|p| p.is_match(app_name))
    }

    /// Comprehensive GUI action check
    /// Returns (allowed, reason)
    pub fn check_gui_action(
        &self,
        app_name: &str,
        screen_text: &str,
        action_description: &str,
    ) -> (bool, String) {
        // Always block dangerous contexts regardless of app
        if self.is_dangerous_gui_context(screen_text) {
            return (
                false,
                "Dangerous GUI context detected (credentials/finance/admin)".into(),
            );
        }

        // Check if action description contains dangerous keywords
        if self.is_dangerous_gui_context(action_description) {
            return (
                false,
                "Action description contains dangerous keywords".into(),
            );
        }

        // Safe targets get more leeway
        if self.is_safe_gui_target(app_name) {
            return (true, format!("Safe application: {}", app_name));
        }

        // Unknown apps require elevated access level
        match self.policy.level {
            AccessLevel::FullAccess | AccessLevel::Elevated => {
                (true, "Allowed by elevated/full access level".into())
            }
            _ => (
                false,
                "Unknown application requires elevated access level".into(),
            ),
        }
    }

    fn matches_patterns(&self, command: &str, patterns: &[Regex]) -> bool {
        patterns.iter().any(|p| p.is_match(command))
    }

    fn assess_risk(&self, command: &str) -> RiskLevel {
        let cmd_lower = command.to_lowercase();

        if cmd_lower.contains("rm -rf")
            || cmd_lower.contains("dd if=")
            || cmd_lower.contains("mkfs")
        {
            return RiskLevel::Critical;
        }

        if cmd_lower.contains("rm -r")
            || cmd_lower.contains("sudo")
            || cmd_lower.contains("chmod")
            || cmd_lower.contains("systemctl stop")
        {
            return RiskLevel::High;
        }

        if cmd_lower.contains("install")
            || cmd_lower.contains("remove")
            || cmd_lower.contains("docker run")
        {
            return RiskLevel::Medium;
        }

        RiskLevel::Low
    }
}

/// Load policy from config file
pub fn load_policy() -> AccessPolicy {
    use directories::ProjectDirs;

    let mut config_paths: Vec<PathBuf> = vec![
        PathBuf::from("/etc/ganesha/policy.toml"),
    ];

    // Add user config dir if available
    if let Some(proj_dirs) = ProjectDirs::from("com", "gtechsd", "ganesha") {
        config_paths.insert(0, proj_dirs.config_dir().join("policy.toml"));
    }

    for path in config_paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(policy) = toml::from_str(&content) {
                    return policy;
                }
            }
        }
    }

    AccessPolicy::default()
}
