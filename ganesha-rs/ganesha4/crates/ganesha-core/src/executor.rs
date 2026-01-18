//! # Execution Engine
//!
//! Executes planned steps with proper error handling, rollback support, and timeout management.
//!
//! ## Overview
//!
//! The executor is responsible for:
//! - Executing individual plan steps
//! - Managing file operations (read, write, edit, delete)
//! - Running shell commands with configurable timeouts
//! - Creating rollback points before destructive operations
//! - Tracking execution state and changes
//!
//! ## Example
//!
//! ```ignore
//! let executor = StandardExecutor::new();
//! let result = executor.execute_step(&step, &context).await?;
//!
//! if result.success {
//!     println!("Step completed: {:?}", result.output);
//! } else {
//!     println!("Step failed: {:?}", result.error);
//! }
//! ```

use crate::planner::{ActionType, PlanStep, RollbackStrategy, StepId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, error, info};

/// Errors that can occur during execution
#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Step execution failed: {0}")]
    ExecutionFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Command failed with exit code {code}: {message}")]
    CommandFailed { code: i32, message: String },

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Rollback creation failed: {0}")]
    RollbackFailed(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Cancelled by user")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, ExecutorError>;

/// Record of a change made during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Path to the file
    pub path: PathBuf,
    /// Type of change
    pub change_type: FileChangeType,
    /// Original content (for rollback)
    pub original_content: Option<String>,
    /// New content
    pub new_content: Option<String>,
    /// Timestamp of change
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl FileChange {
    /// Create a new file change record
    pub fn new(path: impl Into<PathBuf>, change_type: FileChangeType) -> Self {
        Self {
            path: path.into(),
            change_type,
            original_content: None,
            new_content: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Set original content
    pub fn with_original(mut self, content: impl Into<String>) -> Self {
        self.original_content = Some(content.into());
        self
    }

    /// Set new content
    pub fn with_new_content(mut self, content: impl Into<String>) -> Self {
        self.new_content = Some(content.into());
        self
    }
}

/// Type of file change
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChangeType {
    /// File was created
    Created,
    /// File was modified
    Modified,
    /// File was deleted
    Deleted,
    /// File was moved/renamed
    Moved { from: PathBuf },
}

/// Result of executing a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// ID of the step that was executed
    pub step_id: StepId,
    /// Whether execution succeeded
    pub success: bool,
    /// Output from the execution
    pub output: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Changes made during execution
    pub changes: Vec<FileChange>,
    /// Duration of execution
    pub duration: Duration,
    /// Exit code for command executions
    pub exit_code: Option<i32>,
    /// Rollback point ID (if created)
    pub rollback_point: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionResult {
    /// Create a successful result
    pub fn success(step_id: StepId, duration: Duration) -> Self {
        Self {
            step_id,
            success: true,
            output: None,
            error: None,
            changes: Vec::new(),
            duration,
            exit_code: None,
            rollback_point: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed result
    pub fn failure(step_id: StepId, error: impl Into<String>, duration: Duration) -> Self {
        Self {
            step_id,
            success: false,
            output: None,
            error: Some(error.into()),
            changes: Vec::new(),
            duration,
            exit_code: None,
            rollback_point: None,
            metadata: HashMap::new(),
        }
    }

    /// Add output
    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    /// Add a file change
    pub fn with_change(mut self, change: FileChange) -> Self {
        self.changes.push(change);
        self
    }

    /// Add multiple changes
    pub fn with_changes(mut self, changes: impl IntoIterator<Item = FileChange>) -> Self {
        self.changes.extend(changes);
        self
    }

    /// Set exit code
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Set rollback point
    pub fn with_rollback_point(mut self, point_id: impl Into<String>) -> Self {
        self.rollback_point = Some(point_id.into());
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

/// Context for execution
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Working directory for commands
    pub working_directory: PathBuf,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Default timeout for commands
    pub default_timeout: Duration,
    /// Whether to create rollback points
    pub enable_rollback: bool,
    /// Path to store rollback data
    pub rollback_dir: Option<PathBuf>,
    /// Dry run mode (don't actually make changes)
    pub dry_run: bool,
    /// Maximum file size to read (bytes)
    pub max_file_size: usize,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            environment: std::env::vars().collect(),
            default_timeout: Duration::from_secs(120),
            enable_rollback: true,
            rollback_dir: None,
            dry_run: false,
            max_file_size: 10 * 1024 * 1024, // 10 MB
        }
    }
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        Self {
            working_directory: working_directory.into(),
            ..Default::default()
        }
    }

    /// Set environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Enable dry run mode
    pub fn dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }

    /// Disable rollback
    pub fn no_rollback(mut self) -> Self {
        self.enable_rollback = false;
        self
    }

    /// Set rollback directory
    pub fn with_rollback_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.rollback_dir = Some(path.into());
        self
    }
}

/// Trait for step executors
#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute a single plan step
    async fn execute_step(
        &self,
        step: &PlanStep,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult>;

    /// Check if a step can be executed
    fn can_execute(&self, step: &PlanStep) -> bool {
        // Default: can execute all step types
        matches!(
            step.action_type,
            ActionType::ReadFile
                | ActionType::WriteFile
                | ActionType::EditFile
                | ActionType::DeleteFile
                | ActionType::CreateDirectory
                | ActionType::ShellCommand
        )
    }

    /// Rollback a step using its execution result
    async fn rollback(
        &self,
        result: &ExecutionResult,
        context: &ExecutionContext,
    ) -> Result<()>;
}

/// Standard executor implementation
pub struct StandardExecutor {
    /// Maximum number of retries for failed operations
    max_retries: u32,
}

impl StandardExecutor {
    /// Create a new standard executor
    pub fn new() -> Self {
        Self { max_retries: 3 }
    }

    /// Set maximum retries
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Read a file
    async fn read_file(&self, path: &Path, context: &ExecutionContext) -> Result<String> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            context.working_directory.join(path)
        };

        if !full_path.exists() {
            return Err(ExecutorError::FileNotFound(full_path));
        }

        let metadata = tokio::fs::metadata(&full_path).await?;
        if metadata.len() > context.max_file_size as u64 {
            return Err(ExecutorError::InvalidOperation(format!(
                "File too large: {} bytes (max {})",
                metadata.len(),
                context.max_file_size
            )));
        }

        let content = tokio::fs::read_to_string(&full_path).await?;
        Ok(content)
    }

    /// Write a file
    async fn write_file(
        &self,
        path: &Path,
        content: &str,
        context: &ExecutionContext,
    ) -> Result<FileChange> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            context.working_directory.join(path)
        };

        // Check if file exists for rollback
        let original_content = if full_path.exists() {
            Some(tokio::fs::read_to_string(&full_path).await?)
        } else {
            None
        };

        let change_type = if original_content.is_some() {
            FileChangeType::Modified
        } else {
            FileChangeType::Created
        };

        if !context.dry_run {
            // Ensure parent directory exists
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&full_path, content).await?;
        }

        let mut change = FileChange::new(&full_path, change_type).with_new_content(content);
        if let Some(original) = original_content {
            change = change.with_original(original);
        }

        Ok(change)
    }

    /// Delete a file
    async fn delete_file(&self, path: &Path, context: &ExecutionContext) -> Result<FileChange> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            context.working_directory.join(path)
        };

        if !full_path.exists() {
            return Err(ExecutorError::FileNotFound(full_path));
        }

        // Read content for rollback
        let original_content = tokio::fs::read_to_string(&full_path).await.ok();

        if !context.dry_run {
            tokio::fs::remove_file(&full_path).await?;
        }

        let mut change = FileChange::new(&full_path, FileChangeType::Deleted);
        if let Some(original) = original_content {
            change = change.with_original(original);
        }

        Ok(change)
    }

    /// Create a directory
    async fn create_directory(&self, path: &Path, context: &ExecutionContext) -> Result<FileChange> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            context.working_directory.join(path)
        };

        if !context.dry_run {
            tokio::fs::create_dir_all(&full_path).await?;
        }

        Ok(FileChange::new(&full_path, FileChangeType::Created))
    }

    /// Execute a shell command
    async fn execute_command(
        &self,
        command: &str,
        context: &ExecutionContext,
        timeout: Option<Duration>,
    ) -> Result<(String, i32)> {
        let timeout = timeout.unwrap_or(context.default_timeout);

        debug!("Executing command: {} in {:?}", command, context.working_directory);

        if context.dry_run {
            return Ok((format!("[DRY RUN] Would execute: {}", command), 0));
        }

        // Use platform-appropriate shell
        #[cfg(windows)]
        let child = Command::new("cmd")
            .arg("/C")
            .arg(command)
            .current_dir(&context.working_directory)
            .envs(&context.environment)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        #[cfg(not(windows))]
        let child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&context.working_directory)
            .envs(&context.environment)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // Apply timeout - child will be killed on drop if timeout occurs
        let result = tokio::time::timeout(timeout, child.wait_with_output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = if stderr.is_empty() {
                    stdout.to_string()
                } else {
                    format!("{}\n{}", stdout, stderr)
                };

                let exit_code = output.status.code().unwrap_or(-1);
                Ok((combined, exit_code))
            }
            Ok(Err(e)) => Err(ExecutorError::IoError(e)),
            Err(_) => {
                // Process is killed automatically via kill_on_drop
                Err(ExecutorError::Timeout(timeout))
            }
        }
    }

    /// Create a rollback point for a step
    async fn create_rollback_point(
        &self,
        step: &PlanStep,
        context: &ExecutionContext,
    ) -> Result<Option<String>> {
        if !context.enable_rollback {
            return Ok(None);
        }

        match step.rollback_strategy {
            RollbackStrategy::None => Ok(None),
            RollbackStrategy::Auto | RollbackStrategy::Snapshot => {
                // For file operations, the rollback data is stored in FileChange
                // This is a placeholder for more sophisticated rollback mechanisms
                let rollback_id = format!("rollback-{}-{}", step.id, chrono::Utc::now().timestamp());
                info!("Created rollback point: {}", rollback_id);
                Ok(Some(rollback_id))
            }
            RollbackStrategy::Custom(ref _commands) => {
                // Custom rollback commands will be executed during rollback
                let rollback_id = format!("custom-rollback-{}", step.id);
                Ok(Some(rollback_id))
            }
        }
    }
}

impl Default for StandardExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Executor for StandardExecutor {
    async fn execute_step(
        &self,
        step: &PlanStep,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let start = Instant::now();
        info!("Executing step: {} ({:?})", step.description, step.action_type);

        // Create rollback point if needed
        let rollback_point = self.create_rollback_point(step, context).await?;

        let mut result = match &step.action_type {
            ActionType::ReadFile => {
                let mut outputs = Vec::new();
                let all_changes = Vec::new();

                for path in &step.target_files {
                    match self.read_file(path, context).await {
                        Ok(content) => {
                            outputs.push(format!("=== {} ===\n{}", path.display(), content));
                        }
                        Err(e) => {
                            return Ok(ExecutionResult::failure(
                                step.id,
                                format!("Failed to read {}: {}", path.display(), e),
                                start.elapsed(),
                            ));
                        }
                    }
                }

                ExecutionResult::success(step.id, start.elapsed())
                    .with_output(outputs.join("\n\n"))
                    .with_changes(all_changes)
            }

            ActionType::WriteFile => {
                // Get content from step context
                let content = step
                    .context
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let mut changes = Vec::new();
                for path in &step.target_files {
                    match self.write_file(path, content, context).await {
                        Ok(change) => changes.push(change),
                        Err(e) => {
                            return Ok(ExecutionResult::failure(
                                step.id,
                                format!("Failed to write {}: {}", path.display(), e),
                                start.elapsed(),
                            ));
                        }
                    }
                }

                ExecutionResult::success(step.id, start.elapsed())
                    .with_output(format!("Wrote {} file(s)", changes.len()))
                    .with_changes(changes)
            }

            ActionType::EditFile => {
                // Get old and new content from step context
                let old_text = step
                    .context
                    .get("old_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new_text = step
                    .context
                    .get("new_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let mut changes = Vec::new();
                for path in &step.target_files {
                    // Read current content
                    let current_content = match self.read_file(path, context).await {
                        Ok(c) => c,
                        Err(e) => {
                            return Ok(ExecutionResult::failure(
                                step.id,
                                format!("Failed to read {}: {}", path.display(), e),
                                start.elapsed(),
                            ));
                        }
                    };

                    // Apply edit
                    let new_content = current_content.replace(old_text, new_text);

                    match self.write_file(path, &new_content, context).await {
                        Ok(mut change) => {
                            change.original_content = Some(current_content);
                            changes.push(change);
                        }
                        Err(e) => {
                            return Ok(ExecutionResult::failure(
                                step.id,
                                format!("Failed to edit {}: {}", path.display(), e),
                                start.elapsed(),
                            ));
                        }
                    }
                }

                ExecutionResult::success(step.id, start.elapsed())
                    .with_output(format!("Edited {} file(s)", changes.len()))
                    .with_changes(changes)
            }

            ActionType::DeleteFile => {
                let mut changes = Vec::new();
                for path in &step.target_files {
                    match self.delete_file(path, context).await {
                        Ok(change) => changes.push(change),
                        Err(e) => {
                            return Ok(ExecutionResult::failure(
                                step.id,
                                format!("Failed to delete {}: {}", path.display(), e),
                                start.elapsed(),
                            ));
                        }
                    }
                }

                ExecutionResult::success(step.id, start.elapsed())
                    .with_output(format!("Deleted {} file(s)", changes.len()))
                    .with_changes(changes)
            }

            ActionType::CreateDirectory => {
                let mut changes = Vec::new();
                for path in &step.target_files {
                    match self.create_directory(path, context).await {
                        Ok(change) => changes.push(change),
                        Err(e) => {
                            return Ok(ExecutionResult::failure(
                                step.id,
                                format!("Failed to create directory {}: {}", path.display(), e),
                                start.elapsed(),
                            ));
                        }
                    }
                }

                ExecutionResult::success(step.id, start.elapsed())
                    .with_output(format!("Created {} director(y/ies)", changes.len()))
                    .with_changes(changes)
            }

            ActionType::ShellCommand => {
                let command = step
                    .context
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let timeout = step
                    .context
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .map(Duration::from_secs);

                match self.execute_command(command, context, timeout).await {
                    Ok((output, exit_code)) => {
                        if exit_code == 0 {
                            ExecutionResult::success(step.id, start.elapsed())
                                .with_output(output)
                                .with_exit_code(exit_code)
                        } else {
                            ExecutionResult::failure(
                                step.id,
                                format!("Command exited with code {}", exit_code),
                                start.elapsed(),
                            )
                            .with_output(output)
                            .with_exit_code(exit_code)
                        }
                    }
                    Err(e) => ExecutionResult::failure(step.id, e.to_string(), start.elapsed()),
                }
            }

            ActionType::RunTests => {
                // Default test command
                let test_command = step
                    .context
                    .get("test_command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cargo test");

                match self.execute_command(test_command, context, None).await {
                    Ok((output, exit_code)) => {
                        if exit_code == 0 {
                            ExecutionResult::success(step.id, start.elapsed())
                                .with_output(output)
                                .with_exit_code(exit_code)
                        } else {
                            ExecutionResult::failure(
                                step.id,
                                "Tests failed",
                                start.elapsed(),
                            )
                            .with_output(output)
                            .with_exit_code(exit_code)
                        }
                    }
                    Err(e) => ExecutionResult::failure(step.id, e.to_string(), start.elapsed()),
                }
            }

            ActionType::Build => {
                // Default build command
                let build_command = step
                    .context
                    .get("build_command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cargo build");

                match self.execute_command(build_command, context, None).await {
                    Ok((output, exit_code)) => {
                        if exit_code == 0 {
                            ExecutionResult::success(step.id, start.elapsed())
                                .with_output(output)
                                .with_exit_code(exit_code)
                        } else {
                            ExecutionResult::failure(
                                step.id,
                                "Build failed",
                                start.elapsed(),
                            )
                            .with_output(output)
                            .with_exit_code(exit_code)
                        }
                    }
                    Err(e) => ExecutionResult::failure(step.id, e.to_string(), start.elapsed()),
                }
            }

            ActionType::GitOperation => {
                let git_command = step
                    .context
                    .get("git_command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("git status");

                match self.execute_command(git_command, context, None).await {
                    Ok((output, exit_code)) => {
                        ExecutionResult::success(step.id, start.elapsed())
                            .with_output(output)
                            .with_exit_code(exit_code)
                    }
                    Err(e) => ExecutionResult::failure(step.id, e.to_string(), start.elapsed()),
                }
            }

            // These action types are not directly executable by StandardExecutor
            ActionType::Search
            | ActionType::Analyze
            | ActionType::Generate
            | ActionType::UserInput
            | ActionType::Custom(_) => {
                ExecutionResult::success(step.id, start.elapsed())
                    .with_output(format!(
                        "Action type {:?} requires external handling",
                        step.action_type
                    ))
            }
        };

        // Add rollback point if created
        if let Some(ref point) = rollback_point {
            result = result.with_rollback_point(point);
        }

        Ok(result)
    }

    async fn rollback(
        &self,
        result: &ExecutionResult,
        context: &ExecutionContext,
    ) -> Result<()> {
        if context.dry_run {
            info!("[DRY RUN] Would rollback step {}", result.step_id);
            return Ok(());
        }

        info!("Rolling back step {}", result.step_id);

        for change in &result.changes {
            match &change.change_type {
                FileChangeType::Created => {
                    // Delete the created file
                    if change.path.exists() {
                        tokio::fs::remove_file(&change.path).await?;
                        debug!("Rollback: deleted {}", change.path.display());
                    }
                }
                FileChangeType::Modified => {
                    // Restore original content
                    if let Some(ref original) = change.original_content {
                        tokio::fs::write(&change.path, original).await?;
                        debug!("Rollback: restored {}", change.path.display());
                    }
                }
                FileChangeType::Deleted => {
                    // Recreate the deleted file
                    if let Some(ref original) = change.original_content {
                        tokio::fs::write(&change.path, original).await?;
                        debug!("Rollback: recreated {}", change.path.display());
                    }
                }
                FileChangeType::Moved { from } => {
                    // Move the file back
                    if change.path.exists() {
                        tokio::fs::rename(&change.path, from).await?;
                        debug!("Rollback: moved {} back to {}", change.path.display(), from.display());
                    }
                }
            }
        }

        Ok(())
    }
}

/// Execute multiple steps in sequence
pub async fn execute_plan_steps<E: Executor>(
    executor: &E,
    steps: &[PlanStep],
    context: &ExecutionContext,
) -> Vec<ExecutionResult> {
    let mut results = Vec::new();

    for step in steps {
        let result = executor.execute_step(step, context).await;
        match result {
            Ok(r) => {
                let success = r.success;
                results.push(r);
                if !success {
                    break; // Stop on first failure
                }
            }
            Err(e) => {
                error!("Execution error for step {}: {}", step.id, e);
                results.push(ExecutionResult::failure(
                    step.id,
                    e.to_string(),
                    Duration::ZERO,
                ));
                break;
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();

        let executor = StandardExecutor::new();
        let context = ExecutionContext::new(temp_dir.path());

        let step = PlanStep::new("Read test file", ActionType::ReadFile)
            .with_target(&file_path);

        let result = executor.execute_step(&step, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.unwrap().contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");

        let executor = StandardExecutor::new();
        let context = ExecutionContext::new(temp_dir.path());

        let step = PlanStep::new("Write test file", ActionType::WriteFile)
            .with_target(&file_path)
            .with_context("content", "New content!");

        let result = executor.execute_step(&step, &context).await.unwrap();
        assert!(result.success);
        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "New content!");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("to_delete.txt");
        std::fs::write(&file_path, "Delete me!").unwrap();

        let executor = StandardExecutor::new();
        let context = ExecutionContext::new(temp_dir.path());

        let step = PlanStep::new("Delete test file", ActionType::DeleteFile)
            .with_target(&file_path);

        let result = executor.execute_step(&step, &context).await.unwrap();
        assert!(result.success);
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_shell_command() {
        let temp_dir = TempDir::new().unwrap();
        let executor = StandardExecutor::new();
        let context = ExecutionContext::new(temp_dir.path());

        let step = PlanStep::new("Run echo", ActionType::ShellCommand)
            .with_context("command", "echo 'Hello from shell'");

        let result = executor.execute_step(&step, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.unwrap().contains("Hello from shell"));
    }

    #[tokio::test]
    async fn test_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("dry_run.txt");

        let executor = StandardExecutor::new();
        let context = ExecutionContext::new(temp_dir.path()).dry_run();

        let step = PlanStep::new("Write in dry run", ActionType::WriteFile)
            .with_target(&file_path)
            .with_context("content", "Should not be written");

        let result = executor.execute_step(&step, &context).await.unwrap();
        assert!(result.success);
        assert!(!file_path.exists()); // File should not be created in dry run
    }

    #[tokio::test]
    async fn test_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("rollback_test.txt");
        std::fs::write(&file_path, "Original content").unwrap();

        let executor = StandardExecutor::new();
        let context = ExecutionContext::new(temp_dir.path());

        // First, modify the file
        let step = PlanStep::new("Edit file", ActionType::WriteFile)
            .with_target(&file_path)
            .with_context("content", "Modified content");

        let result = executor.execute_step(&step, &context).await.unwrap();
        assert!(result.success);
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "Modified content");

        // Now rollback
        executor.rollback(&result, &context).await.unwrap();
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "Original content");
    }

    #[tokio::test]
    async fn test_command_timeout() {
        let temp_dir = TempDir::new().unwrap();
        let executor = StandardExecutor::new();
        let context = ExecutionContext::new(temp_dir.path());

        let step = PlanStep::new("Slow command", ActionType::ShellCommand)
            .with_context("command", "sleep 5")
            .with_context("timeout", 1u64); // 1 second timeout

        let result = executor.execute_step(&step, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Timeout"));
    }
}
