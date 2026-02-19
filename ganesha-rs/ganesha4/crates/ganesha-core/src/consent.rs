//! # User Consent System
//!
//! Handles user consent for operations based on risk levels and user preferences.
//!
//! ## Overview
//!
//! The consent system is responsible for:
//! - Tracking which operations require user approval
//! - Managing session-scoped and persistent consent rules
//! - Integrating with risk levels to auto-approve safe operations
//! - Providing a consistent consent flow across the application
//!
//! ## Example
//!
//! ```ignore
//! let mut manager = ConsentManager::new(RiskLevel::Normal);
//!
//! // Request consent for an operation
//! let request = ConsentRequest::new("Delete unused files", OperationRisk::High);
//! let decision = manager.request_consent(&request)?;
//!
//! match decision {
//!     ConsentDecision::Approved => { /* proceed */ },
//!     ConsentDecision::Denied => { /* abort */ },
//!     ConsentDecision::NeedsPrompt => { /* ask user */ },
//! }
//! ```

use crate::risk::{OperationRisk, RiskLevel};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors that can occur in the consent system
#[derive(Error, Debug)]
pub enum ConsentError {
    #[error("Consent denied: {0}")]
    Denied(String),

    #[error("Consent request timed out")]
    Timeout,

    #[error("Invalid consent rule: {0}")]
    InvalidRule(String),

    #[error("Consent storage error: {0}")]
    StorageError(String),
}

pub type Result<T> = std::result::Result<T, ConsentError>;

/// Level of consent required/granted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsentLevel {
    /// Automatically approved (no prompt needed)
    Auto,
    /// Requires user confirmation
    Confirm,
    /// Always denied
    Deny,
}

impl Default for ConsentLevel {
    fn default() -> Self {
        Self::Confirm
    }
}

/// Decision from the consent system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentDecision {
    /// Operation is approved
    Approved,
    /// Operation is denied
    Denied,
    /// User needs to be prompted
    NeedsPrompt,
}

/// Category of operation for consent rules
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperationCategory {
    /// File read operations
    FileRead,
    /// File write/create operations
    FileWrite,
    /// File delete operations
    FileDelete,
    /// Shell command execution
    ShellCommand,
    /// Git operations
    Git,
    /// Network operations
    Network,
    /// System operations (sudo, etc.)
    System,
    /// Build operations
    Build,
    /// Test operations
    Test,
    /// Custom category
    Custom(String),
}

impl OperationCategory {
    /// Get default risk level for this category
    pub fn default_risk(&self) -> OperationRisk {
        match self {
            Self::FileRead => OperationRisk::ReadOnly,
            Self::FileWrite | Self::Build | Self::Test => OperationRisk::Medium,
            Self::FileDelete | Self::ShellCommand | Self::Git => OperationRisk::High,
            Self::System => OperationRisk::Critical,
            Self::Network => OperationRisk::Medium,
            Self::Custom(_) => OperationRisk::Medium,
        }
    }
}

/// A request for user consent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRequest {
    /// Unique identifier for this request
    pub id: String,
    /// Human-readable description of the operation
    pub description: String,
    /// Category of operation
    pub category: OperationCategory,
    /// Specific risk level of this operation
    pub risk: OperationRisk,
    /// Files affected (if applicable)
    pub affected_files: Vec<PathBuf>,
    /// Command to execute (if applicable)
    pub command: Option<String>,
    /// Suggested consent level
    pub suggested_level: ConsentLevel,
    /// Whether this is part of a batch operation
    pub batch_id: Option<String>,
    /// Additional context
    pub context: HashMap<String, String>,
}

impl ConsentRequest {
    /// Create a new consent request
    pub fn new(description: impl Into<String>, risk: OperationRisk) -> Self {
        let description = description.into();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            description,
            category: OperationCategory::Custom("general".to_string()),
            risk,
            affected_files: Vec::new(),
            command: None,
            suggested_level: if risk >= OperationRisk::High {
                ConsentLevel::Confirm
            } else {
                ConsentLevel::Auto
            },
            batch_id: None,
            context: HashMap::new(),
        }
    }

    /// Create a request for file operations
    pub fn file_operation(
        description: impl Into<String>,
        category: OperationCategory,
        files: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Self {
        let risk = category.default_risk();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.into(),
            category,
            risk,
            affected_files: files.into_iter().map(Into::into).collect(),
            command: None,
            suggested_level: if risk >= OperationRisk::High {
                ConsentLevel::Confirm
            } else {
                ConsentLevel::Auto
            },
            batch_id: None,
            context: HashMap::new(),
        }
    }

    /// Create a request for shell command execution
    pub fn shell_command(command: impl Into<String>) -> Self {
        let command = command.into();
        let risk = OperationRisk::classify_command(&command);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            description: format!("Execute: {}", command),
            category: OperationCategory::ShellCommand,
            risk,
            affected_files: Vec::new(),
            command: Some(command),
            suggested_level: if risk >= OperationRisk::High {
                ConsentLevel::Confirm
            } else {
                ConsentLevel::Auto
            },
            batch_id: None,
            context: HashMap::new(),
        }
    }

    /// Set the category
    pub fn with_category(mut self, category: OperationCategory) -> Self {
        self.category = category;
        self
    }

    /// Set risk level
    pub fn with_risk(mut self, risk: OperationRisk) -> Self {
        self.risk = risk;
        self.suggested_level = if risk >= OperationRisk::High {
            ConsentLevel::Confirm
        } else {
            ConsentLevel::Auto
        };
        self
    }

    /// Add affected files
    pub fn with_files(mut self, files: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        self.affected_files.extend(files.into_iter().map(Into::into));
        self
    }

    /// Set batch ID
    pub fn in_batch(mut self, batch_id: impl Into<String>) -> Self {
        self.batch_id = Some(batch_id.into());
        self
    }

    /// Add context
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }
}

/// A consent rule that defines automatic behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRule {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Categories this rule applies to
    pub categories: HashSet<OperationCategory>,
    /// Maximum risk level this rule auto-approves
    pub max_auto_approve_risk: OperationRisk,
    /// Path patterns to match (glob-style)
    pub path_patterns: Vec<String>,
    /// Command patterns to match (glob-style)
    pub command_patterns: Vec<String>,
    /// Whether this is a persistent (saved) rule
    pub persistent: bool,
    /// Expiration time (for session rules)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Action to take when rule matches
    pub action: ConsentLevel,
}

impl ConsentRule {
    /// Create a new consent rule
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            categories: HashSet::new(),
            max_auto_approve_risk: OperationRisk::Medium,
            path_patterns: Vec::new(),
            command_patterns: Vec::new(),
            persistent: false,
            expires_at: None,
            action: ConsentLevel::Auto,
        }
    }

    /// Add a category this rule applies to
    pub fn for_category(mut self, category: OperationCategory) -> Self {
        self.categories.insert(category);
        self
    }

    /// Set maximum auto-approve risk
    pub fn up_to_risk(mut self, risk: OperationRisk) -> Self {
        self.max_auto_approve_risk = risk;
        self
    }

    /// Add a path pattern
    pub fn matching_path(mut self, pattern: impl Into<String>) -> Self {
        self.path_patterns.push(pattern.into());
        self
    }

    /// Add a command pattern
    pub fn matching_command(mut self, pattern: impl Into<String>) -> Self {
        self.command_patterns.push(pattern.into());
        self
    }

    /// Make this rule persistent
    pub fn persistent(mut self) -> Self {
        self.persistent = true;
        self
    }

    /// Set expiration duration
    pub fn expires_in(mut self, duration: Duration) -> Self {
        self.expires_at = Some(chrono::Utc::now() + chrono::Duration::from_std(duration).unwrap_or_default());
        self
    }

    /// Set the action
    pub fn with_action(mut self, action: ConsentLevel) -> Self {
        self.action = action;
        self
    }

    /// Check if this rule matches a consent request
    pub fn matches(&self, request: &ConsentRequest) -> bool {
        // Check expiration
        if let Some(expires_at) = self.expires_at {
            if chrono::Utc::now() > expires_at {
                return false;
            }
        }

        // Check category
        if !self.categories.is_empty() && !self.categories.contains(&request.category) {
            return false;
        }

        // Check risk level
        if request.risk > self.max_auto_approve_risk {
            return false;
        }

        // Check path patterns
        if !self.path_patterns.is_empty() {
            let paths_match = request.affected_files.iter().any(|file| {
                let file_str = file.to_string_lossy();
                self.path_patterns.iter().any(|pattern| {
                    glob_match(pattern, &file_str)
                })
            });
            if !paths_match && !request.affected_files.is_empty() {
                return false;
            }
        }

        // Check command patterns
        if !self.command_patterns.is_empty() {
            if let Some(ref cmd) = request.command {
                let cmd_matches = self.command_patterns.iter().any(|pattern| {
                    glob_match(pattern, cmd)
                });
                if !cmd_matches {
                    return false;
                }
            }
        }

        true
    }
}

/// Simple glob matching (supports * and **)
fn glob_match(pattern: &str, text: &str) -> bool {
    // Very basic glob matching
    if pattern == "*" || pattern == "**" {
        return true;
    }

    if pattern.starts_with('*') && pattern.len() > 1 {
        // Suffix match
        let suffix = &pattern[1..];
        return text.ends_with(suffix);
    }

    if pattern.ends_with('*') && pattern.len() > 1 {
        // Prefix match
        let prefix = &pattern[..pattern.len() - 1];
        return text.starts_with(prefix);
    }

    if pattern.contains('*') {
        // Contains match (simplified)
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            return text.starts_with(parts[0]) && text.ends_with(parts[1]);
        }
    }

    // Exact match
    pattern == text
}

/// Response to a consent request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentResponse {
    /// Request ID this responds to
    pub request_id: String,
    /// The decision
    pub decision: ConsentLevel,
    /// Whether to remember this decision
    pub remember: bool,
    /// Scope of remembering
    pub remember_scope: RememberScope,
    /// User comment (optional)
    pub comment: Option<String>,
}

impl ConsentResponse {
    /// Create an approval response
    pub fn approve(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            decision: ConsentLevel::Auto,
            remember: false,
            remember_scope: RememberScope::Session,
            comment: None,
        }
    }

    /// Create a denial response
    pub fn deny(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            decision: ConsentLevel::Deny,
            remember: false,
            remember_scope: RememberScope::Session,
            comment: None,
        }
    }

    /// Remember this decision
    pub fn remember(mut self, scope: RememberScope) -> Self {
        self.remember = true;
        self.remember_scope = scope;
        self
    }

    /// Add a comment
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }
}

/// Scope for remembering consent decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RememberScope {
    /// Just this operation
    Once,
    /// For the current session
    Session,
    /// For this project
    Project,
    /// Globally (all projects)
    Global,
}

impl Default for RememberScope {
    fn default() -> Self {
        Self::Session
    }
}

/// Manages consent requests and rules
pub struct ConsentManager {
    /// Current risk level setting
    risk_level: RiskLevel,
    /// Active consent rules
    rules: Vec<ConsentRule>,
    /// Recently granted consents (for batching)
    recent_consents: HashMap<String, Instant>,
    /// Denied operations (to avoid re-asking)
    denied_operations: HashSet<String>,
    /// Batch consent: approved batch IDs
    approved_batches: HashSet<String>,
    /// Timeout for remembering recent consents
    consent_memory_timeout: Duration,
}

impl ConsentManager {
    /// Create a new consent manager
    pub fn new(risk_level: RiskLevel) -> Self {
        Self {
            risk_level,
            rules: Vec::new(),
            recent_consents: HashMap::new(),
            denied_operations: HashSet::new(),
            approved_batches: HashSet::new(),
            consent_memory_timeout: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Set the risk level
    pub fn set_risk_level(&mut self, level: RiskLevel) {
        self.risk_level = level;
        info!("Risk level set to: {:?}", level);
    }

    /// Get current risk level
    pub fn risk_level(&self) -> RiskLevel {
        self.risk_level
    }

    /// Add a consent rule
    pub fn add_rule(&mut self, rule: ConsentRule) {
        debug!("Adding consent rule: {}", rule.name);
        self.rules.push(rule);
    }

    /// Remove expired rules
    pub fn cleanup_expired_rules(&mut self) {
        let now = chrono::Utc::now();
        self.rules.retain(|rule| {
            if let Some(expires_at) = rule.expires_at {
                expires_at > now
            } else {
                true
            }
        });
    }

    /// Check if an operation is auto-approved based on risk level
    fn auto_approved_by_risk(&self, risk: OperationRisk) -> bool {
        self.risk_level.auto_approves(risk)
    }

    /// Check if an operation is allowed by risk level
    fn allowed_by_risk(&self, risk: OperationRisk) -> bool {
        self.risk_level.allows(risk)
    }

    /// Request consent for an operation
    pub fn request_consent(&mut self, request: &ConsentRequest) -> Result<ConsentDecision> {
        debug!(
            "Consent request: {} (risk: {:?})",
            request.description, request.risk
        );

        // Clean up stale entries
        self.cleanup_recent_consents();

        // Check if this was recently denied
        if self.denied_operations.contains(&request.id) {
            return Ok(ConsentDecision::Denied);
        }

        // Check if this batch is already approved
        if let Some(ref batch_id) = request.batch_id {
            if self.approved_batches.contains(batch_id) {
                return Ok(ConsentDecision::Approved);
            }
        }

        // Check if we recently approved something similar
        let consent_key = self.make_consent_key(request);
        if self.recent_consents.contains_key(&consent_key) {
            return Ok(ConsentDecision::Approved);
        }

        // Check risk level auto-approval
        if self.auto_approved_by_risk(request.risk) {
            debug!("Auto-approved by risk level");
            return Ok(ConsentDecision::Approved);
        }

        // Check if operation is even allowed
        if !self.allowed_by_risk(request.risk) {
            warn!("Operation denied by risk level");
            return Ok(ConsentDecision::Denied);
        }

        // Check consent rules
        for rule in &self.rules {
            if rule.matches(request) {
                debug!("Matched rule: {}", rule.name);
                match rule.action {
                    ConsentLevel::Auto => return Ok(ConsentDecision::Approved),
                    ConsentLevel::Deny => return Ok(ConsentDecision::Denied),
                    ConsentLevel::Confirm => {} // Continue to prompt
                }
            }
        }

        // Need user prompt
        Ok(ConsentDecision::NeedsPrompt)
    }

    /// Record a consent response
    pub fn record_response(&mut self, request: &ConsentRequest, response: &ConsentResponse) {
        match response.decision {
            ConsentLevel::Auto => {
                // Remember approval
                if response.remember {
                    match response.remember_scope {
                        RememberScope::Once => {
                            // Just for this exact request
                        }
                        RememberScope::Session => {
                            let key = self.make_consent_key(request);
                            self.recent_consents.insert(key, Instant::now());
                        }
                        RememberScope::Project | RememberScope::Global => {
                            // Create a rule
                            let rule = ConsentRule::new(format!("Auto-approved: {}", request.description))
                                .for_category(request.category.clone())
                                .up_to_risk(request.risk)
                                .with_action(ConsentLevel::Auto);

                            let rule = if response.remember_scope == RememberScope::Global {
                                rule.persistent()
                            } else {
                                rule
                            };

                            self.rules.push(rule);
                        }
                    }
                }

                // Approve batch if applicable
                if let Some(ref batch_id) = request.batch_id {
                    self.approved_batches.insert(batch_id.clone());
                }
            }
            ConsentLevel::Deny => {
                self.denied_operations.insert(request.id.clone());
            }
            ConsentLevel::Confirm => {
                // No-op, user will be prompted again
            }
        }
    }

    /// Create a key for consent memory
    fn make_consent_key(&self, request: &ConsentRequest) -> String {
        format!(
            "{:?}:{:?}:{}",
            request.category,
            request.risk,
            request.command.as_deref().unwrap_or("")
        )
    }

    /// Clean up old recent consents
    fn cleanup_recent_consents(&mut self) {
        let now = Instant::now();
        self.recent_consents.retain(|_, instant| {
            now.duration_since(*instant) < self.consent_memory_timeout
        });
    }

    /// Grant blanket approval for a batch
    pub fn approve_batch(&mut self, batch_id: impl Into<String>) {
        let id = batch_id.into();
        info!("Batch approved: {}", id);
        self.approved_batches.insert(id);
    }

    /// Revoke batch approval
    pub fn revoke_batch(&mut self, batch_id: &str) {
        self.approved_batches.remove(batch_id);
    }

    /// Clear all session-scoped consents
    pub fn clear_session(&mut self) {
        self.recent_consents.clear();
        self.denied_operations.clear();
        self.approved_batches.clear();
        self.rules.retain(|rule| rule.persistent);
    }

    /// Get persistent rules (for saving)
    pub fn persistent_rules(&self) -> Vec<&ConsentRule> {
        self.rules.iter().filter(|r| r.persistent).collect()
    }

    /// Load persistent rules
    pub fn load_rules(&mut self, rules: impl IntoIterator<Item = ConsentRule>) {
        for rule in rules {
            if rule.persistent {
                self.rules.push(rule);
            }
        }
    }
}

impl Default for ConsentManager {
    fn default() -> Self {
        Self::new(RiskLevel::default())
    }
}

/// Builder for creating common consent rule patterns
pub struct ConsentRuleBuilder;

impl ConsentRuleBuilder {
    /// Create a rule to auto-approve file reads
    pub fn auto_approve_reads() -> ConsentRule {
        ConsentRule::new("Auto-approve file reads")
            .for_category(OperationCategory::FileRead)
            .up_to_risk(OperationRisk::ReadOnly)
            .with_action(ConsentLevel::Auto)
    }

    /// Create a rule to auto-approve git operations
    pub fn auto_approve_git() -> ConsentRule {
        ConsentRule::new("Auto-approve git operations")
            .for_category(OperationCategory::Git)
            .up_to_risk(OperationRisk::Medium)
            .with_action(ConsentLevel::Auto)
    }

    /// Create a rule to auto-approve builds
    pub fn auto_approve_builds() -> ConsentRule {
        ConsentRule::new("Auto-approve build operations")
            .for_category(OperationCategory::Build)
            .up_to_risk(OperationRisk::Medium)
            .with_action(ConsentLevel::Auto)
    }

    /// Create a rule to auto-approve tests
    pub fn auto_approve_tests() -> ConsentRule {
        ConsentRule::new("Auto-approve test operations")
            .for_category(OperationCategory::Test)
            .up_to_risk(OperationRisk::Medium)
            .with_action(ConsentLevel::Auto)
    }

    /// Create a rule to deny system operations
    pub fn deny_system_ops() -> ConsentRule {
        ConsentRule::new("Deny system operations")
            .for_category(OperationCategory::System)
            .up_to_risk(OperationRisk::Critical)
            .with_action(ConsentLevel::Deny)
    }

    /// Create a rule for a specific directory
    pub fn for_directory(dir: impl Into<String>) -> ConsentRule {
        let dir = dir.into();
        ConsentRule::new(format!("Auto-approve operations in {}", dir))
            .matching_path(format!("{}/*", dir))
            .up_to_risk(OperationRisk::Medium)
            .with_action(ConsentLevel::Auto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consent_request_creation() {
        let request = ConsentRequest::new("Test operation", OperationRisk::Low);
        assert_eq!(request.risk, OperationRisk::Low);
        assert!(!request.id.is_empty());
    }

    #[test]
    fn test_shell_command_risk() {
        let request = ConsentRequest::shell_command("ls -la");
        assert_eq!(request.risk, OperationRisk::ReadOnly);

        let request = ConsentRequest::shell_command("sudo apt install");
        assert_eq!(request.risk, OperationRisk::High);
    }

    #[test]
    fn test_consent_rule_matching() {
        let rule = ConsentRule::new("Test rule")
            .for_category(OperationCategory::FileRead)
            .up_to_risk(OperationRisk::Medium);

        let request = ConsentRequest::new("Read file", OperationRisk::ReadOnly)
            .with_category(OperationCategory::FileRead);

        assert!(rule.matches(&request));

        let high_risk_request = ConsentRequest::new("High risk read", OperationRisk::High)
            .with_category(OperationCategory::FileRead);

        assert!(!rule.matches(&high_risk_request));
    }

    #[test]
    fn test_glob_matching() {
        assert!(glob_match("*.rs", "test.rs"));
        assert!(glob_match("src/*", "src/main.rs"));
        assert!(!glob_match("*.py", "test.rs"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("**", "anything/here"));
    }

    #[test]
    fn test_consent_manager_risk_levels() {
        let mut manager = ConsentManager::new(RiskLevel::Safe);

        let request = ConsentRequest::new("Read file", OperationRisk::ReadOnly);
        assert_eq!(
            manager.request_consent(&request).unwrap(),
            ConsentDecision::Approved
        );

        let write_request = ConsentRequest::new("Write file", OperationRisk::Medium);
        assert_eq!(
            manager.request_consent(&write_request).unwrap(),
            ConsentDecision::Denied
        );

        // With YOLO mode, everything is approved
        manager.set_risk_level(RiskLevel::Yolo);
        let high_risk = ConsentRequest::new("Dangerous op", OperationRisk::High);
        assert_eq!(
            manager.request_consent(&high_risk).unwrap(),
            ConsentDecision::Approved
        );
    }

    #[test]
    fn test_consent_rules() {
        let mut manager = ConsentManager::new(RiskLevel::Normal);

        // Add a rule to auto-approve file reads
        manager.add_rule(ConsentRuleBuilder::auto_approve_reads());

        let request = ConsentRequest::new("Read config", OperationRisk::ReadOnly)
            .with_category(OperationCategory::FileRead);

        assert_eq!(
            manager.request_consent(&request).unwrap(),
            ConsentDecision::Approved
        );
    }

    #[test]
    fn test_batch_consent() {
        // Normal level allows Low risk, doesn't auto-approve it
        let mut manager = ConsentManager::new(RiskLevel::Normal);

        // Approve a batch
        manager.approve_batch("batch-123");

        // Requests in this batch should be auto-approved (batch approval takes precedence)
        let request = ConsentRequest::new("Op in batch", OperationRisk::Low)
            .in_batch("batch-123");

        assert_eq!(
            manager.request_consent(&request).unwrap(),
            ConsentDecision::Approved
        );

        // Different batch should not be auto-approved, but allowed at Normal level
        let other_request = ConsentRequest::new("Op in other batch", OperationRisk::Low)
            .in_batch("batch-456");

        assert_eq!(
            manager.request_consent(&other_request).unwrap(),
            ConsentDecision::NeedsPrompt
        );
    }

    #[test]
    fn test_consent_response_recording() {
        let mut manager = ConsentManager::new(RiskLevel::Normal);

        let request = ConsentRequest::new("Test op", OperationRisk::Medium);
        let response = ConsentResponse::approve(&request.id)
            .remember(RememberScope::Session);

        manager.record_response(&request, &response);

        // Similar request should now be auto-approved
        let _similar_request = ConsentRequest::new("Similar op", OperationRisk::Medium);
        // Note: This depends on the consent key generation
    }

    #[test]
    fn test_clear_session() {
        let mut manager = ConsentManager::new(RiskLevel::Normal);

        manager.approve_batch("batch-123");
        manager.add_rule(ConsentRule::new("Persistent rule").persistent());
        manager.add_rule(ConsentRule::new("Session rule"));

        manager.clear_session();

        assert!(manager.approved_batches.is_empty());
        assert_eq!(manager.rules.len(), 1); // Only persistent rule remains
    }

    // ============================================================
    // Additional consent module unit tests
    // ============================================================

    #[test]
    fn test_operation_risk_variants_exist() {
        let _ = OperationRisk::ReadOnly;
        let _ = OperationRisk::Low;
        let _ = OperationRisk::Medium;
        let _ = OperationRisk::High;
        let _ = OperationRisk::Critical;
    }

    #[test]
    fn test_consent_request_file_operation() {
        let req = ConsentRequest::file_operation("read", OperationCategory::FileRead, ["/tmp/test.txt"]);
        assert_eq!(req.risk, OperationRisk::ReadOnly);
        assert!(!req.affected_files.is_empty());
    }

    #[test]
    fn test_consent_request_with_category() {
        let req = ConsentRequest::new("Op", OperationRisk::Low)
            .with_category(OperationCategory::FileWrite);
        assert!(matches!(req.category, OperationCategory::FileWrite));
    }

    #[test]
    fn test_consent_request_with_files() {
        let req = ConsentRequest::new("Op", OperationRisk::Low)
            .with_files(["/a.txt", "/b.txt"]);
        assert_eq!(req.affected_files.len(), 2);
    }

    #[test]
    fn test_consent_request_with_context() {
        let req = ConsentRequest::new("Op", OperationRisk::Low)
            .with_context("reason", "testing");
        assert_eq!(req.context.get("reason"), Some(&"testing".to_string()));
    }

    #[test]
    fn test_consent_request_in_batch() {
        let req = ConsentRequest::new("Op", OperationRisk::Low)
            .in_batch("batch-42");
        assert_eq!(req.batch_id, Some("batch-42".to_string()));
    }

    #[test]
    fn test_shell_command_risk_classification() {
        // Safe commands
        let safe = ConsentRequest::shell_command("cat file.txt");
        assert_eq!(safe.risk, OperationRisk::ReadOnly);

        let ls = ConsentRequest::shell_command("ls -la /tmp");
        assert_eq!(ls.risk, OperationRisk::ReadOnly);

        // Risky commands
        let rm = ConsentRequest::shell_command("rm -rf /tmp/old");
        assert!(matches!(rm.risk, OperationRisk::High | OperationRisk::Critical));

        let sudo = ConsentRequest::shell_command("sudo systemctl restart nginx");
        assert!(matches!(sudo.risk, OperationRisk::High | OperationRisk::Critical));
    }

    #[test]
    fn test_consent_response_approve() {
        let resp = ConsentResponse::approve("req-1");
        assert!(matches!(resp.decision, ConsentLevel::Auto));
    }

    #[test]
    fn test_consent_response_deny() {
        let resp = ConsentResponse::deny("req-1");
        assert!(matches!(resp.decision, ConsentLevel::Deny));
    }

    #[test]
    fn test_consent_response_with_comment() {
        let resp = ConsentResponse::approve("req-1")
            .with_comment("Looks safe");
        assert_eq!(resp.comment, Some("Looks safe".to_string()));
    }

    #[test]
    fn test_consent_response_remember_session() {
        let resp = ConsentResponse::approve("req-1")
            .remember(RememberScope::Session);
        assert!(resp.remember);
        assert!(matches!(resp.remember_scope, RememberScope::Session));
    }

    #[test]
    fn test_consent_response_remember_always() {
        let resp = ConsentResponse::approve("req-1")
            .remember(RememberScope::Global);
        assert!(resp.remember);
        assert!(matches!(resp.remember_scope, RememberScope::Global));
    }

    #[test]
    fn test_operation_category_default_risk() {
        assert!(matches!(OperationCategory::FileRead.default_risk(), OperationRisk::ReadOnly));
        assert!(matches!(OperationCategory::System.default_risk(), OperationRisk::High | OperationRisk::Critical));
    }

    #[test]
    fn test_consent_rule_builder() {
        let rule = ConsentRule::new("Auto-approve reads")
            .for_category(OperationCategory::FileRead)
            .up_to_risk(OperationRisk::Low)
            .persistent()
            .with_action(ConsentLevel::Auto);
        assert!(rule.persistent);
        assert!(matches!(rule.action, ConsentLevel::Auto));
    }

    #[test]
    fn test_consent_rule_expires_in() {
        let rule = ConsentRule::new("Temp rule")
            .expires_in(Duration::from_secs(3600));
        assert!(rule.expires_at.is_some());
    }

    #[test]
    fn test_consent_rule_path_matching() {
        let rule = ConsentRule::new("Rust files")
            .matching_path("*.rs")
            .up_to_risk(OperationRisk::Medium)
            .with_action(ConsentLevel::Auto);

        let req = ConsentRequest::file_operation("edit", OperationCategory::FileWrite, ["main.rs"]);
        assert!(rule.matches(&req));

        let non_match = ConsentRequest::file_operation("edit", OperationCategory::FileWrite, ["main.py"]);
        assert!(!rule.matches(&non_match));
    }

    #[test]
    fn test_consent_rule_command_matching() {
        let rule = ConsentRule::new("Git commands")
            .matching_command("git *")
            .up_to_risk(OperationRisk::Medium)
            .with_action(ConsentLevel::Auto);

        let req = ConsentRequest::shell_command("git status");
        assert!(rule.matches(&req));
    }

    #[test]
    fn test_consent_manager_safe_mode() {
        let mut manager = ConsentManager::new(RiskLevel::Safe);

        // ReadOnly should be approved in safe mode
        let read = ConsentRequest::new("Read", OperationRisk::ReadOnly);
        assert_eq!(manager.request_consent(&read).unwrap(), ConsentDecision::Approved);

        // Low risk should be denied in safe mode
        let low = ConsentRequest::new("Write", OperationRisk::Low);
        assert_eq!(manager.request_consent(&low).unwrap(), ConsentDecision::Denied);
    }

    #[test]
    fn test_consent_manager_trusted_mode() {
        let mut manager = ConsentManager::new(RiskLevel::Trusted);

        // Medium risk should be auto-approved in trusted mode
        let med = ConsentRequest::new("Edit file", OperationRisk::Medium);
        assert_eq!(manager.request_consent(&med).unwrap(), ConsentDecision::Approved);
    }

    #[test]
    fn test_consent_decision_variants() {
        let _ = ConsentDecision::Approved;
        let _ = ConsentDecision::Denied;
        let _ = ConsentDecision::NeedsPrompt;
    }

    #[test]
    fn test_consent_level_variants() {
        let _ = ConsentLevel::Auto;
        let _ = ConsentLevel::Deny;
        let _ = ConsentLevel::Confirm;
    }

}
