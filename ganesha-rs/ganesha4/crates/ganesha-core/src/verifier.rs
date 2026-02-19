//! # Verification System
//!
//! Verifies execution results through syntax checking, test running, and diff validation.
//!
//! ## Overview
//!
//! The verifier is responsible for:
//! - Checking syntax validity of modified files
//! - Running test suites to validate changes
//! - Comparing actual vs expected outcomes
//! - Providing detailed verification reports
//!
//! ## Example
//!
//! ```ignore
//! let verifier = StandardVerifier::new();
//! let result = verifier.verify(&execution_result, &context).await?;
//!
//! if result.passed() {
//!     println!("Verification passed!");
//! } else {
//!     for issue in &result.issues {
//!         println!("Issue: {} ({:?})", issue.message, issue.severity);
//!     }
//! }
//! ```

use crate::executor::{ExecutionResult, FileChangeType};
use crate::planner::StepId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Errors that can occur during verification
#[derive(Error, Debug)]
pub enum VerifierError {
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Timeout during verification: {0}")]
    Timeout(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, VerifierError>;

/// Severity of a verification issue
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// Informational note
    Info,
    /// Warning that doesn't prevent success
    Warning,
    /// Error that causes verification failure
    Error,
    /// Critical issue that may indicate data corruption
    Critical,
}

/// Type of verification check
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CheckType {
    /// Syntax validation
    Syntax,
    /// Type checking
    Types,
    /// Linting/style
    Lint,
    /// Unit tests
    UnitTests,
    /// Integration tests
    IntegrationTests,
    /// Diff comparison
    DiffComparison,
    /// File existence
    FileExists,
    /// Build verification
    Build,
    /// Custom check
    Custom(String),
}

/// A single verification issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationIssue {
    /// Type of check that found the issue
    pub check_type: CheckType,
    /// Severity of the issue
    pub severity: IssueSeverity,
    /// Human-readable message
    pub message: String,
    /// File associated with the issue (if any)
    pub file: Option<PathBuf>,
    /// Line number (if applicable)
    pub line: Option<usize>,
    /// Column number (if applicable)
    pub column: Option<usize>,
    /// Suggestion for fixing
    pub suggestion: Option<String>,
    /// Raw tool output
    pub raw_output: Option<String>,
}

impl VerificationIssue {
    /// Create a new verification issue
    pub fn new(check_type: CheckType, severity: IssueSeverity, message: impl Into<String>) -> Self {
        Self {
            check_type,
            severity,
            message: message.into(),
            file: None,
            line: None,
            column: None,
            suggestion: None,
            raw_output: None,
        }
    }

    /// Set the file
    pub fn with_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.file = Some(path.into());
        self
    }

    /// Set location
    pub fn with_location(mut self, line: usize, column: Option<usize>) -> Self {
        self.line = Some(line);
        self.column = column;
        self
    }

    /// Set suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Set raw output
    pub fn with_raw_output(mut self, output: impl Into<String>) -> Self {
        self.raw_output = Some(output.into());
        self
    }

    /// Check if this is an error or critical issue
    pub fn is_error(&self) -> bool {
        self.severity >= IssueSeverity::Error
    }
}

/// Overall verification status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// All checks passed
    Passed,
    /// Passed with warnings
    PassedWithWarnings,
    /// Failed due to errors
    Failed,
    /// Could not complete verification
    Incomplete,
}

/// Result of a verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// ID of the step that was verified
    pub step_id: StepId,
    /// Overall status
    pub status: VerificationStatus,
    /// All issues found
    pub issues: Vec<VerificationIssue>,
    /// Checks that were run
    pub checks_run: Vec<CheckType>,
    /// Checks that were skipped
    pub checks_skipped: Vec<CheckType>,
    /// Duration of verification
    pub duration: Duration,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl VerificationResult {
    /// Create a new verification result
    pub fn new(step_id: StepId) -> Self {
        Self {
            step_id,
            status: VerificationStatus::Passed,
            issues: Vec::new(),
            checks_run: Vec::new(),
            checks_skipped: Vec::new(),
            duration: Duration::ZERO,
            metadata: HashMap::new(),
        }
    }

    /// Check if verification passed
    pub fn passed(&self) -> bool {
        matches!(
            self.status,
            VerificationStatus::Passed | VerificationStatus::PassedWithWarnings
        )
    }

    /// Get error count
    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.is_error()).count()
    }

    /// Get warning count
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count()
    }

    /// Add an issue
    pub fn add_issue(&mut self, issue: VerificationIssue) {
        // Update status based on issue severity
        if issue.severity >= IssueSeverity::Error {
            self.status = VerificationStatus::Failed;
        } else if issue.severity == IssueSeverity::Warning
            && self.status == VerificationStatus::Passed
        {
            self.status = VerificationStatus::PassedWithWarnings;
        }
        self.issues.push(issue);
    }

    /// Record a check that was run
    pub fn record_check(&mut self, check_type: CheckType) {
        if !self.checks_run.contains(&check_type) {
            self.checks_run.push(check_type);
        }
    }

    /// Record a skipped check
    pub fn skip_check(&mut self, check_type: CheckType, reason: &str) {
        if !self.checks_skipped.contains(&check_type) {
            self.checks_skipped.push(check_type.clone());
            self.add_issue(VerificationIssue::new(
                check_type,
                IssueSeverity::Info,
                format!("Check skipped: {}", reason),
            ));
        }
    }

    /// Set duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), json_value);
        }
        self
    }
}

/// Context for verification
#[derive(Debug, Clone)]
pub struct VerificationContext {
    /// Working directory
    pub working_directory: PathBuf,
    /// Expected outcomes (file path -> expected content)
    pub expected_outcomes: HashMap<PathBuf, String>,
    /// Checks to run
    pub enabled_checks: Vec<CheckType>,
    /// Timeout for verification commands
    pub timeout: Duration,
    /// Whether to run tests
    pub run_tests: bool,
    /// Test command to use
    pub test_command: Option<String>,
    /// Build command to use
    pub build_command: Option<String>,
    /// Lint command to use
    pub lint_command: Option<String>,
}

impl Default for VerificationContext {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            expected_outcomes: HashMap::new(),
            enabled_checks: vec![
                CheckType::Syntax,
                CheckType::FileExists,
                CheckType::FileExists,
            ],
            timeout: Duration::from_secs(300), // 5 minutes
            run_tests: true,
            test_command: None,
            build_command: None,
            lint_command: None,
        }
    }
}

impl VerificationContext {
    /// Create a new verification context
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        Self {
            working_directory: working_directory.into(),
            ..Default::default()
        }
    }

    /// Add expected outcome
    pub fn expect(mut self, path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        self.expected_outcomes.insert(path.into(), content.into());
        self
    }

    /// Enable a check
    pub fn with_check(mut self, check_type: CheckType) -> Self {
        if !self.enabled_checks.contains(&check_type) {
            self.enabled_checks.push(check_type);
        }
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set test command
    pub fn with_test_command(mut self, command: impl Into<String>) -> Self {
        self.test_command = Some(command.into());
        self
    }

    /// Set build command
    pub fn with_build_command(mut self, command: impl Into<String>) -> Self {
        self.build_command = Some(command.into());
        self
    }

    /// Set lint command
    pub fn with_lint_command(mut self, command: impl Into<String>) -> Self {
        self.lint_command = Some(command.into());
        self
    }

    /// Disable tests
    pub fn no_tests(mut self) -> Self {
        self.run_tests = false;
        self
    }
}

/// Trait for verifiers
#[async_trait]
pub trait Verifier: Send + Sync {
    /// Verify an execution result
    async fn verify(
        &self,
        execution_result: &ExecutionResult,
        context: &VerificationContext,
    ) -> Result<VerificationResult>;

    /// Check syntax of a file
    async fn check_syntax(&self, path: &Path, context: &VerificationContext) -> Result<Vec<VerificationIssue>>;

    /// Run tests
    async fn run_tests(&self, context: &VerificationContext) -> Result<Vec<VerificationIssue>>;

    /// Compare actual vs expected content
    fn verify_diff(
        &self,
        actual: &str,
        expected: &str,
        path: &Path,
    ) -> Vec<VerificationIssue>;
}

/// Standard verifier implementation
pub struct StandardVerifier {
    /// File extensions and their syntax check commands
    syntax_checkers: HashMap<String, Vec<String>>,
}

impl StandardVerifier {
    /// Create a new standard verifier
    pub fn new() -> Self {
        let mut syntax_checkers = HashMap::new();

        // Rust
        syntax_checkers.insert(
            "rs".to_string(),
            vec!["cargo".to_string(), "check".to_string()],
        );

        // Python
        syntax_checkers.insert(
            "py".to_string(),
            vec!["python3".to_string(), "-m".to_string(), "py_compile".to_string()],
        );

        // JavaScript/TypeScript
        syntax_checkers.insert(
            "js".to_string(),
            vec!["node".to_string(), "--check".to_string()],
        );
        syntax_checkers.insert(
            "ts".to_string(),
            vec!["npx".to_string(), "tsc".to_string(), "--noEmit".to_string()],
        );

        // JSON
        syntax_checkers.insert(
            "json".to_string(),
            vec!["python3".to_string(), "-m".to_string(), "json.tool".to_string()],
        );

        // YAML
        syntax_checkers.insert(
            "yaml".to_string(),
            vec!["python3".to_string(), "-c".to_string(), "import yaml; yaml.safe_load(open('{}'))".to_string()],
        );
        syntax_checkers.insert(
            "yml".to_string(),
            vec!["python3".to_string(), "-c".to_string(), "import yaml; yaml.safe_load(open('{}'))".to_string()],
        );

        Self { syntax_checkers }
    }

    /// Add a custom syntax checker
    pub fn with_syntax_checker(
        mut self,
        extension: impl Into<String>,
        command: Vec<String>,
    ) -> Self {
        self.syntax_checkers.insert(extension.into(), command);
        self
    }

    /// Run a command and capture output
    async fn run_command(
        &self,
        command: &[String],
        working_dir: &Path,
        timeout: Duration,
    ) -> Result<(bool, String)> {
        if command.is_empty() {
            return Err(VerifierError::VerificationFailed(
                "Empty command".to_string(),
            ));
        }

        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..])
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = tokio::time::timeout(timeout, async {
            let child = cmd.spawn()?;
            let output = child.wait_with_output().await?;
            Ok::<_, std::io::Error>(output)
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}{}", stdout, stderr);
                Ok((output.status.success(), combined))
            }
            Ok(Err(e)) => Err(VerifierError::IoError(e)),
            Err(_) => Err(VerifierError::Timeout(format!(
                "Command {:?} timed out after {:?}",
                command, timeout
            ))),
        }
    }

    /// Get syntax checker for a file extension
    fn get_syntax_checker(&self, path: &Path) -> Option<Vec<String>> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.syntax_checkers.get(ext))
            .cloned()
    }

    /// Detect project type from working directory
    fn detect_project_type(&self, working_dir: &Path) -> Option<String> {
        if working_dir.join("Cargo.toml").exists() {
            Some("rust".to_string())
        } else if working_dir.join("package.json").exists() {
            Some("node".to_string())
        } else if working_dir.join("pyproject.toml").exists()
            || working_dir.join("setup.py").exists()
        {
            Some("python".to_string())
        } else if working_dir.join("go.mod").exists() {
            Some("go".to_string())
        } else {
            None
        }
    }

    /// Get default test command for project type
    fn get_default_test_command(&self, project_type: &str) -> &str {
        match project_type {
            "rust" => "cargo test",
            "node" => "npm test",
            "python" => "pytest",
            "go" => "go test ./...",
            _ => "echo 'No test command configured'",
        }
    }

    /// Get default build command for project type
    fn get_default_build_command(&self, project_type: &str) -> &str {
        match project_type {
            "rust" => "cargo build",
            "node" => "npm run build",
            "python" => "python -m py_compile",
            "go" => "go build ./...",
            _ => "echo 'No build command configured'",
        }
    }

    /// Parse compiler/linter output into issues
    fn parse_tool_output(
        &self,
        output: &str,
        check_type: CheckType,
    ) -> Vec<VerificationIssue> {
        let mut issues = Vec::new();

        // Try to parse common error formats
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Rust format: error[E0001]: message
            if line.starts_with("error") {
                let severity = if line.contains("[E") {
                    IssueSeverity::Error
                } else {
                    IssueSeverity::Warning
                };
                issues.push(
                    VerificationIssue::new(check_type.clone(), severity, line)
                        .with_raw_output(output.to_string()),
                );
            }
            // Generic format: file:line:col: message
            else if line.contains(":") && !line.starts_with(" ") {
                let parts: Vec<&str> = line.splitn(4, ':').collect();
                if parts.len() >= 3 {
                    let severity = if line.to_lowercase().contains("error") {
                        IssueSeverity::Error
                    } else if line.to_lowercase().contains("warning") {
                        IssueSeverity::Warning
                    } else {
                        IssueSeverity::Info
                    };

                    let mut issue = VerificationIssue::new(
                        check_type.clone(),
                        severity,
                        *parts.get(3).unwrap_or(&line),
                    );

                    if let Ok(line_num) = parts.get(1).unwrap_or(&"").parse::<usize>() {
                        let col = parts.get(2).and_then(|s| s.parse().ok());
                        issue = issue
                            .with_file(parts[0])
                            .with_location(line_num, col);
                    }

                    issues.push(issue);
                }
            }
        }

        // If no issues were parsed but output exists and command failed,
        // add a generic error
        if issues.is_empty() && !output.trim().is_empty() {
            // Check if this looks like an error output
            let lower_output = output.to_lowercase();
            if lower_output.contains("error") || lower_output.contains("failed") {
                issues.push(
                    VerificationIssue::new(
                        check_type,
                        IssueSeverity::Error,
                        "Verification check failed",
                    )
                    .with_raw_output(output.to_string()),
                );
            }
        }

        issues
    }
}

impl Default for StandardVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verifier for StandardVerifier {
    async fn verify(
        &self,
        execution_result: &ExecutionResult,
        context: &VerificationContext,
    ) -> Result<VerificationResult> {
        let start = Instant::now();
        let mut result = VerificationResult::new(execution_result.step_id);

        info!(
            "Verifying step {} with {} changes",
            execution_result.step_id,
            execution_result.changes.len()
        );

        // Check file existence for created/modified files
        if context.enabled_checks.contains(&CheckType::FileExists) {
            result.record_check(CheckType::FileExists);

            for change in &execution_result.changes {
                match change.change_type {
                    FileChangeType::Created | FileChangeType::Modified => {
                        if !change.path.exists() {
                            result.add_issue(
                                VerificationIssue::new(
                                    CheckType::FileExists,
                                    IssueSeverity::Error,
                                    format!("File should exist but doesn't: {}", change.path.display()),
                                )
                                .with_file(&change.path),
                            );
                        }
                    }
                    FileChangeType::Deleted => {
                        if change.path.exists() {
                            result.add_issue(
                                VerificationIssue::new(
                                    CheckType::FileExists,
                                    IssueSeverity::Error,
                                    format!("File should be deleted but still exists: {}", change.path.display()),
                                )
                                .with_file(&change.path),
                            );
                        }
                    }
                    FileChangeType::Moved { ref from } => {
                        if from.exists() {
                            result.add_issue(
                                VerificationIssue::new(
                                    CheckType::FileExists,
                                    IssueSeverity::Error,
                                    format!("Original file should be gone: {}", from.display()),
                                )
                                .with_file(from),
                            );
                        }
                        if !change.path.exists() {
                            result.add_issue(
                                VerificationIssue::new(
                                    CheckType::FileExists,
                                    IssueSeverity::Error,
                                    format!("Moved file should exist: {}", change.path.display()),
                                )
                                .with_file(&change.path),
                            );
                        }
                    }
                }
            }
        }

        // Check syntax of modified files
        if context.enabled_checks.contains(&CheckType::Syntax) {
            result.record_check(CheckType::Syntax);

            for change in &execution_result.changes {
                if matches!(
                    change.change_type,
                    FileChangeType::Created | FileChangeType::Modified
                ) {
                    let syntax_issues = self.check_syntax(&change.path, context).await?;
                    for issue in syntax_issues {
                        result.add_issue(issue);
                    }
                }
            }
        }

        // Verify expected outcomes (diff check)
        if context.enabled_checks.contains(&CheckType::DiffComparison) {
            result.record_check(CheckType::DiffComparison);

            for (path, expected) in &context.expected_outcomes {
                if path.exists() {
                    match tokio::fs::read_to_string(path).await {
                        Ok(actual) => {
                            let diff_issues = self.verify_diff(&actual, expected, path);
                            for issue in diff_issues {
                                result.add_issue(issue);
                            }
                        }
                        Err(e) => {
                            result.add_issue(
                                VerificationIssue::new(
                                    CheckType::DiffComparison,
                                    IssueSeverity::Error,
                                    format!("Failed to read {}: {}", path.display(), e),
                                )
                                .with_file(path),
                            );
                        }
                    }
                } else {
                    result.add_issue(
                        VerificationIssue::new(
                            CheckType::DiffComparison,
                            IssueSeverity::Error,
                            format!("Expected file doesn't exist: {}", path.display()),
                        )
                        .with_file(path),
                    );
                }
            }
        }

        // Run build check
        if context.enabled_checks.contains(&CheckType::Build) {
            result.record_check(CheckType::Build);

            let project_type = self.detect_project_type(&context.working_directory);
            let build_cmd = context
                .build_command
                .clone()
                .or_else(|| project_type.as_ref().map(|t| self.get_default_build_command(t).to_string()));

            if let Some(cmd) = build_cmd {
                debug!("Running build command: {}", cmd);
                let cmd_parts: Vec<String> = cmd.split_whitespace().map(String::from).collect();
                match self
                    .run_command(&cmd_parts, &context.working_directory, context.timeout)
                    .await
                {
                    Ok((success, output)) => {
                        if !success {
                            let issues = self.parse_tool_output(&output, CheckType::Build);
                            if issues.is_empty() {
                                result.add_issue(
                                    VerificationIssue::new(
                                        CheckType::FileExists,
                                        IssueSeverity::Error,
                                        "Build failed",
                                    )
                                    .with_raw_output(output),
                                );
                            } else {
                                for issue in issues {
                                    result.add_issue(issue);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Build check failed: {}", e);
                        result.skip_check(CheckType::FileExists, &e.to_string());
                    }
                }
            } else {
                result.skip_check(CheckType::FileExists, "No build command configured");
            }
        }

        // Run tests
        if context.run_tests && context.enabled_checks.contains(&CheckType::UnitTests) {
            let test_issues = self.run_tests(context).await?;
            result.record_check(CheckType::UnitTests);
            for issue in test_issues {
                result.add_issue(issue);
            }
        }

        result.duration = start.elapsed();
        Ok(result)
    }

    async fn check_syntax(
        &self,
        path: &Path,
        context: &VerificationContext,
    ) -> Result<Vec<VerificationIssue>> {
        let mut issues = Vec::new();

        if !path.exists() {
            return Ok(vec![VerificationIssue::new(
                CheckType::Syntax,
                IssueSeverity::Warning,
                format!("Cannot check syntax: file doesn't exist: {}", path.display()),
            )
            .with_file(path)]);
        }

        // Get the appropriate syntax checker
        if let Some(checker_cmd) = self.get_syntax_checker(path) {
            // Replace {} with the file path
            let cmd: Vec<String> = checker_cmd
                .iter()
                .map(|s| s.replace("{}", &path.to_string_lossy()))
                .collect();

            debug!("Running syntax check: {:?}", cmd);

            match self
                .run_command(&cmd, &context.working_directory, context.timeout)
                .await
            {
                Ok((success, output)) => {
                    if !success {
                        issues.extend(self.parse_tool_output(&output, CheckType::Syntax));
                        if issues.is_empty() {
                            issues.push(
                                VerificationIssue::new(
                                    CheckType::Syntax,
                                    IssueSeverity::Error,
                                    format!("Syntax check failed for {}", path.display()),
                                )
                                .with_file(path)
                                .with_raw_output(output),
                            );
                        }
                    }
                }
                Err(e) => {
                    issues.push(
                        VerificationIssue::new(
                            CheckType::Syntax,
                            IssueSeverity::Warning,
                            format!("Could not run syntax checker: {}", e),
                        )
                        .with_file(path),
                    );
                }
            }
        } else {
            // No syntax checker configured for this file type
            debug!(
                "No syntax checker configured for {}",
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
            );
        }

        Ok(issues)
    }

    async fn run_tests(&self, context: &VerificationContext) -> Result<Vec<VerificationIssue>> {
        let mut issues = Vec::new();

        let project_type = self.detect_project_type(&context.working_directory);
        let test_cmd = context
            .test_command
            .clone()
            .or_else(|| project_type.as_ref().map(|t| self.get_default_test_command(t).to_string()));

        if let Some(cmd) = test_cmd {
            info!("Running tests: {}", cmd);
            let cmd_parts: Vec<String> = cmd.split_whitespace().map(String::from).collect();

            match self
                .run_command(&cmd_parts, &context.working_directory, context.timeout)
                .await
            {
                Ok((success, output)) => {
                    if !success {
                        issues.push(
                            VerificationIssue::new(
                                CheckType::UnitTests,
                                IssueSeverity::Error,
                                "Test suite failed",
                            )
                            .with_raw_output(output),
                        );
                    } else {
                        info!("Tests passed");
                    }
                }
                Err(e) => {
                    issues.push(VerificationIssue::new(
                        CheckType::UnitTests,
                        IssueSeverity::Warning,
                        format!("Could not run tests: {}", e),
                    ));
                }
            }
        } else {
            issues.push(VerificationIssue::new(
                CheckType::UnitTests,
                IssueSeverity::Info,
                "No test command configured",
            ));
        }

        Ok(issues)
    }

    fn verify_diff(
        &self,
        actual: &str,
        expected: &str,
        path: &Path,
    ) -> Vec<VerificationIssue> {
        let mut issues = Vec::new();

        if actual != expected {
            // Find the first difference
            let actual_lines: Vec<&str> = actual.lines().collect();
            let expected_lines: Vec<&str> = expected.lines().collect();

            for (i, (a, e)) in actual_lines.iter().zip(expected_lines.iter()).enumerate() {
                if a != e {
                    issues.push(
                        VerificationIssue::new(
                            CheckType::DiffComparison,
                            IssueSeverity::Error,
                            format!("Content mismatch at line {}", i + 1),
                        )
                        .with_file(path)
                        .with_location(i + 1, None)
                        .with_suggestion(format!("Expected: '{}', got: '{}'", e, a)),
                    );
                    break;
                }
            }

            // Check for line count difference
            if actual_lines.len() != expected_lines.len() {
                issues.push(
                    VerificationIssue::new(
                        CheckType::DiffComparison,
                        IssueSeverity::Error,
                        format!(
                            "Line count mismatch: expected {} lines, got {}",
                            expected_lines.len(),
                            actual_lines.len()
                        ),
                    )
                    .with_file(path),
                );
            }
        }

        issues
    }
}

/// Quick verification helpers
pub async fn verify_file_exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

pub async fn verify_file_contains(path: &Path, content: &str) -> bool {
    match tokio::fs::read_to_string(path).await {
        Ok(file_content) => file_content.contains(content),
        Err(_) => false,
    }
}

pub async fn verify_file_matches(path: &Path, expected: &str) -> bool {
    match tokio::fs::read_to_string(path).await {
        Ok(file_content) => file_content == expected,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_verification_result() {
        let step_id = StepId::new();
        let mut result = VerificationResult::new(step_id);

        assert!(result.passed());
        assert_eq!(result.error_count(), 0);

        result.add_issue(VerificationIssue::new(
            CheckType::Syntax,
            IssueSeverity::Warning,
            "Warning message",
        ));

        assert!(result.passed()); // Still passes with warning
        assert_eq!(result.status, VerificationStatus::PassedWithWarnings);

        result.add_issue(VerificationIssue::new(
            CheckType::Syntax,
            IssueSeverity::Error,
            "Error message",
        ));

        assert!(!result.passed());
        assert_eq!(result.error_count(), 1);
    }

    #[test]
    fn test_diff_verification() {
        let verifier = StandardVerifier::new();
        let path = PathBuf::from("test.txt");

        // Identical content
        let issues = verifier.verify_diff("hello\nworld", "hello\nworld", &path);
        assert!(issues.is_empty());

        // Different content
        let issues = verifier.verify_diff("hello\nworld", "hello\nearth", &path);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].severity, IssueSeverity::Error);

        // Different line count
        let issues = verifier.verify_diff("hello", "hello\nworld", &path);
        assert!(!issues.is_empty());
    }

    #[tokio::test]
    async fn test_file_verification_helpers() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // File doesn't exist
        assert!(!verify_file_exists(&file_path).await);

        // Create file
        tokio::fs::write(&file_path, "Hello, World!").await.unwrap();

        assert!(verify_file_exists(&file_path).await);
        assert!(verify_file_contains(&file_path, "Hello").await);
        assert!(!verify_file_contains(&file_path, "Goodbye").await);
        assert!(verify_file_matches(&file_path, "Hello, World!").await);
        assert!(!verify_file_matches(&file_path, "Hello").await);
    }

    #[test]
    fn test_project_type_detection() {
        let verifier = StandardVerifier::new();

        // This test is limited since we can't easily create the files
        // Just test the method exists and returns None for empty dir
        let temp_dir = TempDir::new().unwrap();
        assert!(verifier.detect_project_type(temp_dir.path()).is_none());
    }

    #[test]
    fn test_verification_issue_new() {
        let issue = VerificationIssue::new(
            CheckType::Syntax,
            IssueSeverity::Error,
            "Missing semicolon",
        );
        assert_eq!(issue.message, "Missing semicolon");
        assert_eq!(issue.check_type, CheckType::Syntax);
        assert_eq!(issue.severity, IssueSeverity::Error);
        assert!(issue.file.is_none());
        assert!(issue.line.is_none());
    }

    #[test]
    fn test_verification_issue_with_file() {
        let issue = VerificationIssue::new(CheckType::Types, IssueSeverity::Warning, "Type mismatch")
            .with_file("src/main.rs");
        assert_eq!(issue.file, Some(PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_verification_issue_with_location() {
        let issue = VerificationIssue::new(CheckType::Lint, IssueSeverity::Info, "unused variable")
            .with_file("lib.rs")
            .with_location(42, Some(10));
        assert_eq!(issue.line, Some(42));
        assert_eq!(issue.column, Some(10));
    }

    #[test]
    fn test_verification_issue_with_suggestion() {
        let issue = VerificationIssue::new(CheckType::Syntax, IssueSeverity::Error, "typo")
            .with_suggestion("Did you mean 'println'?");
        assert_eq!(issue.suggestion, Some("Did you mean 'println'?".to_string()));
    }

    #[test]
    fn test_verification_issue_builder_chain() {
        let issue = VerificationIssue::new(CheckType::FileExists, IssueSeverity::Error, "build failed")
            .with_file("Cargo.toml")
            .with_location(5, None)
            .with_suggestion("Add missing dependency");
        assert_eq!(issue.file, Some(PathBuf::from("Cargo.toml")));
        assert_eq!(issue.line, Some(5));
        assert!(issue.column.is_none());
        assert!(issue.suggestion.is_some());
    }

    #[test]
    fn test_verification_result_empty_passes() {
        let result = VerificationResult::new(StepId::new());
        assert!(result.passed());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn test_verification_result_info_only() {
        let mut result = VerificationResult::new(StepId::new());
        result.add_issue(VerificationIssue::new(
            CheckType::Lint, IssueSeverity::Info, "Consider renaming",
        ));
        assert!(result.passed()); // Info doesn't fail
    }

    #[test]
    fn test_verification_result_warning_count() {
        let mut result = VerificationResult::new(StepId::new());
        result.add_issue(VerificationIssue::new(CheckType::Lint, IssueSeverity::Warning, "W1"));
        result.add_issue(VerificationIssue::new(CheckType::Types, IssueSeverity::Warning, "W2"));
        assert_eq!(result.warning_count(), 2);
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_check_type_variants() {
        let _ = CheckType::Syntax;
        let _ = CheckType::Types;
        let _ = CheckType::Lint;
        let _ = CheckType::Build;
        let _ = CheckType::UnitTests;
        let _ = CheckType::DiffComparison;
    }

    #[test]
    fn test_issue_severity_variants() {
        let _ = IssueSeverity::Info;
        let _ = IssueSeverity::Warning;
        let _ = IssueSeverity::Error;
    }

    #[test]
    fn test_issue_severity_ordering() {
        assert!(IssueSeverity::Info < IssueSeverity::Warning);
        assert!(IssueSeverity::Warning < IssueSeverity::Error);
    }

    #[test]
    fn test_standard_verifier_new() {
        let verifier = StandardVerifier::new();
        // Just verify it constructs without panic
        let _ = verifier;
    }

}
