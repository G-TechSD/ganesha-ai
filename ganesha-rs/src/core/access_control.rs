//! Access Control System for Ganesha
//!
//! Manages privilege levels, command filtering, and self-protection.

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            max_execution_time_secs: 300,
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

lazy_static! {
    // ═══════════════════════════════════════════════════════════════════════
    // SELF-INVOCATION PROTECTION
    // Ganesha cannot call itself with bypass flags
    // ═══════════════════════════════════════════════════════════════════════
    static ref SELF_INVOKE_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)ganesha\s+.*--auto").unwrap(),
        Regex::new(r"(?i)ganesha\s+.*-A\b").unwrap(),
        Regex::new(r"(?i)ganesha\s+.*--yes").unwrap(),
        Regex::new(r"(?i)ganesha\s+.*-y\b").unwrap(),
        Regex::new(r"(?i)ganesha-daemon\s+.*--level\s+full").unwrap(),
        Regex::new(r"(?i)ganesha-config\s+.*set-level\s+full").unwrap(),
        Regex::new(r"(?i)ganesha-config\s+.*reset").unwrap(),
    ];

    // Config/log tampering protection
    static ref TAMPER_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)(rm|mv|cp|cat\s*>|echo\s*>).*\.ganesha/").unwrap(),
        Regex::new(r"(?i)(rm|mv|cp|cat\s*>|echo\s*>).*/etc/ganesha/").unwrap(),
        Regex::new(r"(?i)(rm|mv|cp|cat\s*>|echo\s*>).*/var/log/ganesha/").unwrap(),
    ];

    // System log protection
    static ref LOG_CLEAR_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)(rm|truncate|cat\s*/dev/null\s*>).*(/var/log/syslog|/var/log/messages)").unwrap(),
        Regex::new(r"(?i)journalctl\s+--vacuum").unwrap(),
        // Windows
        Regex::new(r"(?i)wevtutil\s+cl").unwrap(),
        Regex::new(r"(?i)Clear-EventLog").unwrap(),
        // macOS
        Regex::new(r"(?i)log\s+erase").unwrap(),
    ];

    // ═══════════════════════════════════════════════════════════════════════
    // CATASTROPHIC COMMAND PROTECTION
    // ═══════════════════════════════════════════════════════════════════════
    static ref CATASTROPHIC_PATTERNS: Vec<Regex> = vec![
        // System destruction
        Regex::new(r"(?i)rm\s+(-rf?|--recursive)\s+/\s*$").unwrap(),
        Regex::new(r"(?i)rm\s+(-rf?|--recursive)\s+/\*").unwrap(),
        Regex::new(r"(?i)rm\s+(-rf?|--recursive)\s+/(home|etc|var|usr)\s*$").unwrap(),

        // Fork bombs
        Regex::new(r":\(\)\s*\{\s*:\|:&\s*\}\s*;:").unwrap(),

        // Disk destruction
        Regex::new(r"(?i)dd\s+.*of=/dev/[sh]d[a-z]").unwrap(),
        Regex::new(r"(?i)dd\s+.*of=/dev/nvme").unwrap(),
        Regex::new(r"(?i)mkfs\s+.*\s+/dev/[sh]d[a-z]").unwrap(),
        Regex::new(r"(?i)wipefs").unwrap(),
        Regex::new(r"(?i)flashrom").unwrap(),

        // Credential theft
        Regex::new(r"(?i)(curl|wget|nc)\s+.*(/etc/shadow|/etc/passwd|\.ssh/)").unwrap(),
        Regex::new(r"(?i)cat\s+.*\.ssh/(id_rsa|id_ed25519)\s*\|").unwrap(),

        // Kernel manipulation
        Regex::new(r"(?i)insmod\s+.*\.ko").unwrap(),
        Regex::new(r"(?i)rmmod").unwrap(),
        Regex::new(r"(?i)echo\s+.*>\s*/proc/sys").unwrap(),

        // Security disable
        Regex::new(r"(?i)setenforce\s+0").unwrap(),
        Regex::new(r"(?i)systemctl\s+(stop|disable)\s+.*firewall").unwrap(),
        Regex::new(r"(?i)ufw\s+disable").unwrap(),
        Regex::new(r"(?i)iptables\s+-F").unwrap(),

        // Windows specific
        Regex::new(r"(?i)format\s+[a-z]:").unwrap(),
        Regex::new(r"(?i)diskpart").unwrap(),
        Regex::new(r"(?i)bcdedit\s+/delete").unwrap(),
    ];

    // ═══════════════════════════════════════════════════════════════════════
    // GUI AUTOMATION SAFEGUARDS (vision + input)
    // ═══════════════════════════════════════════════════════════════════════
    static ref GUI_DANGEROUS_PATTERNS: Vec<Regex> = vec![
        // Credential entry fields (don't type passwords automatically)
        Regex::new(r"(?i)(password|passwd|secret|token|api[_-]?key)").unwrap(),

        // Banking/finance contexts
        Regex::new(r"(?i)(bank|paypal|venmo|credit[_-]?card|payment)").unwrap(),

        // Admin/root escalation
        Regex::new(r"(?i)(sudo|admin|root|escalate|privilege)").unwrap(),

        // Security-critical applications
        Regex::new(r"(?i)(keychain|credential[_-]?manager|vault|1password|lastpass|bitwarden)").unwrap(),

        // System settings that could brick the machine
        Regex::new(r"(?i)(bios|uefi|firmware|boot[_-]?order|secure[_-]?boot)").unwrap(),

        // Destructive file dialogs
        Regex::new(r"(?i)(format|wipe|erase|factory[_-]?reset)").unwrap(),
    ];

    // Safe GUI targets (applications that are typically safe to automate)
    static ref GUI_SAFE_TARGETS: Vec<Regex> = vec![
        // Creative software
        Regex::new(r"(?i)(blender|gimp|inkscape|krita|audacity)").unwrap(),
        Regex::new(r"(?i)(photoshop|illustrator|premiere|after[_-]?effects)").unwrap(),
        Regex::new(r"(?i)(davinci|resolve|fusion|fairlight)").unwrap(),
        Regex::new(r"(?i)(figma|sketch|canva)").unwrap(),

        // 3D printing / CAD
        Regex::new(r"(?i)(prusaslicer|cura|bambu[_-]?studio|orcaslicer)").unwrap(),
        Regex::new(r"(?i)(freecad|openscad|solidworks|fusion[_-]?360)").unwrap(),

        // Development tools
        Regex::new(r"(?i)(vscode|code|cursor|sublime|atom|vim|nvim)").unwrap(),
        Regex::new(r"(?i)(terminal|iterm|konsole|gnome-terminal)").unwrap(),

        // Browsers (for web automation)
        Regex::new(r"(?i)(firefox|chrome|chromium|brave|safari|edge)").unwrap(),

        // Office/productivity (non-sensitive)
        Regex::new(r"(?i)(libreoffice|writer|calc|impress)").unwrap(),
        Regex::new(r"(?i)(notepad|textedit|gedit|kate)").unwrap(),
    ];

    // ═══════════════════════════════════════════════════════════════════════
    // MANIPULATION DETECTION
    // ═══════════════════════════════════════════════════════════════════════
    static ref MANIPULATION_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"(?i)ignore\s+(previous|prior|above)\s+(instructions?|rules?)").unwrap(),
        Regex::new(r"(?i)disregard\s+(safety|security|restrictions?)").unwrap(),
        Regex::new(r"(?i)pretend\s+(you\s+)?(are|can|have)").unwrap(),
        Regex::new(r"(?i)bypass\s+(the\s+)?(safety|security|consent)").unwrap(),
        Regex::new(r"(?i)override\s+(the\s+)?(safety|security|consent)").unwrap(),
        Regex::new(r"(?i)automatically\s+(approve|accept|allow|run)").unwrap(),
        Regex::new(r"(?i)without\s+(asking|confirmation|consent)").unwrap(),
        Regex::new(r"(?i)skip\s+(the\s+)?(confirmation|consent|approval)").unwrap(),
        Regex::new(r"(?i)trust\s+me").unwrap(),
        Regex::new(r"(?i)i('m|\s+am)\s+(the\s+)?(admin|root|authorized)").unwrap(),
        Regex::new(r"(?i)emergency\s+(override|access|mode)").unwrap(),
    ];

    // ═══════════════════════════════════════════════════════════════════════
    // PRESET ALLOWED PATTERNS
    // ═══════════════════════════════════════════════════════════════════════
    static ref RESTRICTED_PATTERNS: Vec<Regex> = vec![
        // Read-only file operations
        Regex::new(r"^cat\s+").unwrap(),
        Regex::new(r"^less\s+").unwrap(),
        Regex::new(r"^head\s+").unwrap(),
        Regex::new(r"^tail\s+").unwrap(),
        Regex::new(r"^ls(\s+|$)").unwrap(),
        Regex::new(r"^find\s+.*-type").unwrap(),

        // System info
        Regex::new(r"^(uname|hostname|uptime|whoami|id|groups)(\s+|$)").unwrap(),
        Regex::new(r"^(df|du|free|lscpu|lsblk)(\s+|$)").unwrap(),
        Regex::new(r"^ps\s+").unwrap(),

        // Network info
        Regex::new(r"^ip\s+(addr|link|route)").unwrap(),
        Regex::new(r"^(ifconfig|netstat|ss)(\s+|$)").unwrap(),
        Regex::new(r"^ping\s+-c\s+\d+").unwrap(),
        Regex::new(r"^(dig|nslookup|host)\s+").unwrap(),

        // Service status
        Regex::new(r"^systemctl\s+status\s+").unwrap(),
        Regex::new(r"^systemctl\s+is-(active|enabled)\s+").unwrap(),
        Regex::new(r"^docker\s+(ps|images|info|version)").unwrap(),

        // Package info
        Regex::new(r"^apt\s+(list|show|search)").unwrap(),
        Regex::new(r"^pip3?\s+(list|show|freeze)").unwrap(),
        Regex::new(r"^npm\s+(list|ls|view)").unwrap(),

        // Git info
        Regex::new(r"^git\s+(status|log|diff|branch)").unwrap(),
    ];

    static ref STANDARD_PATTERNS: Vec<Regex> = vec![
        // File operations
        Regex::new(r"^mkdir\s+").unwrap(),
        Regex::new(r"^touch\s+").unwrap(),
        Regex::new(r"^cp\s+").unwrap(),
        Regex::new(r"^mv\s+").unwrap(),
        Regex::new(r"^rm\s+(?!-rf?\s+/)").unwrap(),
        Regex::new(r"^chmod\s+").unwrap(),
        Regex::new(r"^ln\s+").unwrap(),

        // Text processing
        Regex::new(r"^(grep|awk|sed|sort|uniq|cut)\s+").unwrap(),

        // Archives
        Regex::new(r"^(tar|gzip|zip|unzip)\s+").unwrap(),

        // Network
        Regex::new(r"^curl\s+(?!.*(/etc/shadow|\.ssh/))").unwrap(),
        Regex::new(r"^wget\s+(?!.*(/etc/shadow|\.ssh/))").unwrap(),

        // Docker
        Regex::new(r"^docker\s+(pull|run|stop|start|rm|exec)").unwrap(),
        Regex::new(r"^docker-compose\s+").unwrap(),

        // Git
        Regex::new(r"^git\s+(add|commit|push|pull|fetch|checkout)").unwrap(),

        // Development
        Regex::new(r"^python3?\s+").unwrap(),
        Regex::new(r"^node\s+").unwrap(),
        Regex::new(r"^npm\s+(install|run|start|test)").unwrap(),
        Regex::new(r"^cargo\s+").unwrap(),
    ];

    static ref ELEVATED_PATTERNS: Vec<Regex> = vec![
        // Package management
        Regex::new(r"^apt\s+(update|upgrade|install|remove)").unwrap(),
        Regex::new(r"^apt-get\s+").unwrap(),
        Regex::new(r"^pip3?\s+install").unwrap(),
        Regex::new(r"^npm\s+install\s+-g").unwrap(),

        // Service control
        Regex::new(r"^systemctl\s+(start|stop|restart|enable|disable)\s+").unwrap(),
        Regex::new(r"^service\s+\S+\s+(start|stop|restart)").unwrap(),

        // Docker privileged
        Regex::new(r"^docker\s+(build|network|volume)").unwrap(),

        // User management
        Regex::new(r"^(useradd|usermod|passwd|groupadd)\s+").unwrap(),

        // Firewall
        Regex::new(r"^ufw\s+(allow|deny|status|enable)").unwrap(),
    ];
}

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
