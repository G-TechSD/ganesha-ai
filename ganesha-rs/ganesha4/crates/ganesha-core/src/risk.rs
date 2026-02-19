//! # Risk Level System
//!
//! Human-readable risk levels that control what Ganesha can do.
//!
//! ## Levels
//!
//! - **Safe**: Read-only, no system changes
//! - **Normal**: Asks before risky operations (default)
//! - **Trusted**: Auto-approves routine tasks
//! - **Yolo**: Auto-approves everything

use serde::{Deserialize, Serialize};
use std::fmt;

/// Risk level for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Read-only, no system changes
    /// "I'll look but won't touch"
    Safe,

    /// Asks before risky operations (default)
    /// "I'll ask before anything risky"
    Normal,

    /// Auto-approves routine tasks
    /// "I'll handle routine tasks automatically"
    Trusted,

    /// Auto-approves everything
    /// "Full send, no questions asked"
    Yolo,
}

impl Default for RiskLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Safe => write!(f, "safe"),
            Self::Normal => write!(f, "normal"),
            Self::Trusted => write!(f, "trusted"),
            Self::Yolo => write!(f, "yolo"),
        }
    }
}

impl std::str::FromStr for RiskLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "safe" => Ok(Self::Safe),
            "normal" | "default" => Ok(Self::Normal),
            "trusted" => Ok(Self::Trusted),
            "yolo" | "all" | "a" => Ok(Self::Yolo),
            _ => Err(format!("Unknown risk level: {}", s)),
        }
    }
}

impl RiskLevel {
    /// Human-readable description of this risk level
    pub fn description(&self) -> &'static str {
        match self {
            Self::Safe => "I'll look but won't touch",
            Self::Normal => "I'll ask before anything risky",
            Self::Trusted => "I'll handle routine tasks automatically",
            Self::Yolo => "Full send, no questions asked",
        }
    }

    /// Icon for this risk level
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Safe => "🟢",
            Self::Normal => "🟡",
            Self::Trusted => "🟠",
            Self::Yolo => "🔴",
        }
    }

    /// Check if this level allows a given operation risk
    pub fn allows(&self, operation_risk: OperationRisk) -> bool {
        match self {
            Self::Safe => operation_risk == OperationRisk::ReadOnly,
            Self::Normal => operation_risk <= OperationRisk::Low,
            Self::Trusted => operation_risk <= OperationRisk::Medium,
            Self::Yolo => true, // Even critical with warning
        }
    }

    /// Check if this level auto-approves a given operation risk
    pub fn auto_approves(&self, operation_risk: OperationRisk) -> bool {
        match self {
            Self::Safe => operation_risk == OperationRisk::ReadOnly,
            Self::Normal => operation_risk == OperationRisk::ReadOnly,
            Self::Trusted => operation_risk <= OperationRisk::Medium,
            Self::Yolo => operation_risk < OperationRisk::Critical,
        }
    }
}

/// Risk level of an operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OperationRisk {
    /// No side effects (ls, cat, pwd)
    ReadOnly,
    /// Safe side effects (mkdir, touch, git status)
    Low,
    /// Reversible changes (npm install, file edits)
    Medium,
    /// Potentially dangerous (rm, sudo, system config)
    High,
    /// Always warn even in YOLO mode (rm -rf /, dd)
    Critical,
}

impl OperationRisk {
    /// Classify a command's risk level
    pub fn classify_command(command: &str) -> Self {
        let parts: Vec<&str> = command.split_whitespace().collect();
        let cmd = parts.first().map(|s| *s).unwrap_or("");

        // Critical - always warn
        if command.contains("rm -rf /")
            || command.contains("dd if=")
            || command.contains("mkfs")
            || command.contains("fdisk")
            || command.contains("> /dev/")
        {
            return Self::Critical;
        }

        // High risk
        if cmd == "sudo"
            || cmd == "rm"
            || cmd == "chmod"
            || cmd == "chown"
            || command.contains("/etc/")
            || command.contains("/boot/")
            || command.contains("systemctl")
        {
            return Self::High;
        }

        // Medium risk
        if cmd == "mv"
            || cmd == "cp"
            || cmd == "mkdir"
            || cmd == "touch"
            || command.contains("install")
            || command.contains("npm")
            || command.contains("pip")
            || command.contains("cargo")
            || command.contains("apt")
            || command.contains("brew")
            || command.contains("git add")
            || command.contains("git commit")
            || command.contains("git push")
        {
            return Self::Medium;
        }

        // Low risk
        if command.contains("git")
            || cmd == "echo"
            || cmd == "printf"
            || cmd == "date"
            || cmd == "whoami"
        {
            return Self::Low;
        }

        // Read-only
        if cmd == "ls"
            || cmd == "cat"
            || cmd == "head"
            || cmd == "tail"
            || cmd == "grep"
            || cmd == "find"
            || cmd == "pwd"
            || cmd == "which"
            || cmd == "type"
            || cmd == "file"
            || cmd == "wc"
        {
            return Self::ReadOnly;
        }

        // Default to medium for unknown commands
        Self::Medium
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_parsing() {
        assert_eq!("safe".parse::<RiskLevel>().unwrap(), RiskLevel::Safe);
        assert_eq!("normal".parse::<RiskLevel>().unwrap(), RiskLevel::Normal);
        assert_eq!("trusted".parse::<RiskLevel>().unwrap(), RiskLevel::Trusted);
        assert_eq!("yolo".parse::<RiskLevel>().unwrap(), RiskLevel::Yolo);
        assert_eq!("a".parse::<RiskLevel>().unwrap(), RiskLevel::Yolo);
    }

    #[test]
    fn test_command_classification() {
        assert_eq!(OperationRisk::classify_command("ls -la"), OperationRisk::ReadOnly);
        assert_eq!(OperationRisk::classify_command("cat file.txt"), OperationRisk::ReadOnly);
        assert_eq!(OperationRisk::classify_command("npm install"), OperationRisk::Medium);
        assert_eq!(OperationRisk::classify_command("sudo apt update"), OperationRisk::High);
        assert_eq!(OperationRisk::classify_command("rm -rf /"), OperationRisk::Critical);
    }

    #[test]
    fn test_risk_level_allows() {
        assert!(RiskLevel::Safe.allows(OperationRisk::ReadOnly));
        assert!(!RiskLevel::Safe.allows(OperationRisk::Low));

        assert!(RiskLevel::Normal.allows(OperationRisk::Low));
        assert!(!RiskLevel::Normal.allows(OperationRisk::Medium));

        assert!(RiskLevel::Trusted.allows(OperationRisk::Medium));
        assert!(!RiskLevel::Trusted.allows(OperationRisk::High));

        assert!(RiskLevel::Yolo.allows(OperationRisk::Critical));
    }

    #[test]
    fn test_risk_level_descriptions() {
        assert!(!RiskLevel::Safe.description().is_empty());
        assert!(!RiskLevel::Normal.description().is_empty());
        assert!(!RiskLevel::Trusted.description().is_empty());
        assert!(!RiskLevel::Yolo.description().is_empty());
    }

    #[test]
    fn test_risk_level_icons() {
        assert_eq!(RiskLevel::Safe.icon(), "🟢");
        assert_eq!(RiskLevel::Normal.icon(), "🟡");
        assert_eq!(RiskLevel::Trusted.icon(), "🟠");
        assert_eq!(RiskLevel::Yolo.icon(), "🔴");
    }

    #[test]
    fn test_risk_level_auto_approves() {
        // Safe: only auto-approves ReadOnly
        assert!(RiskLevel::Safe.auto_approves(OperationRisk::ReadOnly));
        assert!(!RiskLevel::Safe.auto_approves(OperationRisk::Low));

        // Normal: only auto-approves ReadOnly
        assert!(RiskLevel::Normal.auto_approves(OperationRisk::ReadOnly));
        assert!(!RiskLevel::Normal.auto_approves(OperationRisk::Low));

        // Trusted: auto-approves up to Medium
        assert!(RiskLevel::Trusted.auto_approves(OperationRisk::Medium));
        assert!(!RiskLevel::Trusted.auto_approves(OperationRisk::High));

        // Yolo: auto-approves everything below Critical
        assert!(RiskLevel::Yolo.auto_approves(OperationRisk::High));
    }

    #[test]
    fn test_operation_risk_ordering() {
        assert!(OperationRisk::ReadOnly < OperationRisk::Low);
        assert!(OperationRisk::Low < OperationRisk::Medium);
        assert!(OperationRisk::Medium < OperationRisk::High);
        assert!(OperationRisk::High < OperationRisk::Critical);
    }

    #[test]
    fn test_command_classification_read_commands() {
        let read_cmds = ["ls", "cat foo.txt", "pwd", "grep pattern", "find .", "head -n 5"];
        for cmd in read_cmds {
            assert_eq!(OperationRisk::classify_command(cmd), OperationRisk::ReadOnly,
                "Expected ReadOnly for: {}", cmd);
        }
    }

    #[test]
    fn test_command_classification_dangerous() {
        let dangerous = ["rm -rf /tmp/old", "sudo reboot", "dd if=/dev/zero"];
        for cmd in dangerous {
            let risk = OperationRisk::classify_command(cmd);
            assert!(risk >= OperationRisk::High,
                "Expected High+ for: {} (got {:?})", cmd, risk);
        }
    }

    #[test]
    fn test_command_classification_medium() {
        let medium = ["npm install", "cargo build", "pip install requests"];
        for cmd in medium {
            let risk = OperationRisk::classify_command(cmd);
            assert!(risk >= OperationRisk::Medium,
                "Expected Medium+ for: {} (got {:?})", cmd, risk);
        }
    }

    #[test]
    fn test_risk_level_default() {
        assert_eq!(RiskLevel::default(), RiskLevel::Normal);
    }

    #[test]
    fn test_risk_level_display() {
        // RiskLevel should have a Display impl or at least Debug
        let safe_str = format!("{:?}", RiskLevel::Safe);
        assert!(safe_str.contains("Safe"));
    }

    #[test]
    fn test_operation_risk_display() {
        let read_str = format!("{:?}", OperationRisk::ReadOnly);
        assert!(read_str.contains("ReadOnly"));
    }

    #[test]
    fn test_risk_level_parsing_case_insensitive() {
        assert_eq!("SAFE".parse::<RiskLevel>().unwrap(), RiskLevel::Safe);
        assert_eq!("Normal".parse::<RiskLevel>().unwrap(), RiskLevel::Normal);
        assert_eq!("YOLO".parse::<RiskLevel>().unwrap(), RiskLevel::Yolo);
    }

}
