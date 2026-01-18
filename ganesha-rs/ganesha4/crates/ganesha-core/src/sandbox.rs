//! Sandbox environment for safe code execution
//!
//! Provides isolated execution environments to safely run
//! potentially dangerous operations before applying them to
//! the live system.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Sandbox-specific errors
#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("Sandbox creation failed: {0}")]
    CreationFailed(String),

    #[error("Sandbox execution failed: {0}")]
    ExecutionFailed(String),

    #[error("File operation failed: {0}")]
    FileError(String),

    #[error("Command execution failed: {0}")]
    CommandError(String),

    #[error("Sandbox not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SandboxError>;

/// Sandbox execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxMode {
    /// Full isolation - copy entire project to temp directory
    FullIsolation,
    /// Git worktree - use git worktree for isolation
    GitWorktree,
    /// Virtual filesystem overlay (Linux only)
    Overlay,
    /// No isolation - dry run only (show what would happen)
    DryRun,
}

impl Default for SandboxMode {
    fn default() -> Self {
        Self::GitWorktree
    }
}

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Sandbox mode to use
    pub mode: SandboxMode,
    /// Base directory for sandboxes
    pub base_dir: Option<PathBuf>,
    /// Maximum sandbox lifetime in seconds
    pub max_lifetime_secs: u64,
    /// Maximum disk usage in MB
    pub max_disk_mb: u64,
    /// Allow network access
    pub allow_network: bool,
    /// Allow executing commands
    pub allow_commands: bool,
    /// Command whitelist (if allow_commands is true)
    pub command_whitelist: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            mode: SandboxMode::GitWorktree,
            base_dir: None,
            max_lifetime_secs: 3600, // 1 hour
            max_disk_mb: 1024,       // 1 GB
            allow_network: false,
            allow_commands: true,
            command_whitelist: vec![
                "cargo".to_string(),
                "npm".to_string(),
                "yarn".to_string(),
                "pnpm".to_string(),
                "python".to_string(),
                "node".to_string(),
                "go".to_string(),
                "make".to_string(),
                "cmake".to_string(),
            ],
        }
    }
}

/// A sandbox environment
#[derive(Debug)]
pub struct Sandbox {
    /// Unique sandbox ID
    pub id: String,
    /// Original project root
    pub original_root: PathBuf,
    /// Sandbox root directory
    pub sandbox_root: PathBuf,
    /// Sandbox mode
    pub mode: SandboxMode,
    /// Configuration
    pub config: SandboxConfig,
    /// Created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Files modified in sandbox
    pub modified_files: Vec<PathBuf>,
    /// Commands executed in sandbox
    pub executed_commands: Vec<ExecutedCommand>,
    /// Whether sandbox is active
    pub active: bool,
}

/// Record of an executed command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutedCommand {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

impl Sandbox {
    /// Create a new sandbox
    pub async fn create(
        original_root: PathBuf,
        config: SandboxConfig,
    ) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        let base_dir = config.base_dir.clone()
            .unwrap_or_else(std::env::temp_dir);

        let sandbox_root = base_dir.join(format!("ganesha-sandbox-{}", &id[..8]));

        let sandbox = Self {
            id,
            original_root: original_root.clone(),
            sandbox_root: sandbox_root.clone(),
            mode: config.mode,
            config,
            created_at: chrono::Utc::now(),
            modified_files: Vec::new(),
            executed_commands: Vec::new(),
            active: true,
        };

        // Create sandbox based on mode
        match sandbox.mode {
            SandboxMode::FullIsolation => {
                sandbox.create_full_copy().await?;
            }
            SandboxMode::GitWorktree => {
                sandbox.create_git_worktree().await?;
            }
            SandboxMode::Overlay => {
                sandbox.create_overlay().await?;
            }
            SandboxMode::DryRun => {
                // No actual sandbox created
                tracing::info!("Dry-run mode: no sandbox created");
            }
        }

        tracing::info!("Sandbox created: {} at {:?}", sandbox.id, sandbox.sandbox_root);
        Ok(sandbox)
    }

    /// Create full copy sandbox
    async fn create_full_copy(&self) -> Result<()> {
        use tokio::fs;

        // Create sandbox directory
        fs::create_dir_all(&self.sandbox_root).await?;

        // Copy project recursively (excluding .git, node_modules, target, etc.)
        copy_dir_recursive(&self.original_root, &self.sandbox_root, &[
            ".git",
            "node_modules",
            "target",
            ".cargo",
            "__pycache__",
            ".venv",
            "venv",
        ]).await?;

        Ok(())
    }

    /// Create git worktree sandbox
    async fn create_git_worktree(&self) -> Result<()> {
        use tokio::process::Command;

        // Create a new git worktree
        let output = Command::new("git")
            .current_dir(&self.original_root)
            .args(["worktree", "add", "-d"])
            .arg(&self.sandbox_root)
            .arg("HEAD")
            .output()
            .await?;

        if !output.status.success() {
            return Err(SandboxError::CreationFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        Ok(())
    }

    /// Create overlay filesystem sandbox (Linux only)
    async fn create_overlay(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            // This would require root privileges or user namespaces
            // For now, fall back to git worktree
            tracing::warn!("Overlay mode not fully implemented, falling back to git worktree");
            self.create_git_worktree().await
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SandboxError::CreationFailed(
                "Overlay mode only supported on Linux".to_string()
            ))
        }
    }

    /// Write a file in the sandbox
    pub async fn write_file(&mut self, relative_path: &Path, content: &str) -> Result<()> {
        if self.mode == SandboxMode::DryRun {
            tracing::info!("Dry-run: would write to {:?}", relative_path);
            self.modified_files.push(relative_path.to_path_buf());
            return Ok(());
        }

        let full_path = self.sandbox_root.join(relative_path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&full_path, content).await?;
        self.modified_files.push(relative_path.to_path_buf());

        tracing::debug!("Sandbox: wrote file {:?}", relative_path);
        Ok(())
    }

    /// Read a file from the sandbox
    pub async fn read_file(&self, relative_path: &Path) -> Result<String> {
        let full_path = if self.mode == SandboxMode::DryRun {
            self.original_root.join(relative_path)
        } else {
            self.sandbox_root.join(relative_path)
        };

        tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| SandboxError::FileError(e.to_string()))
    }

    /// Execute a command in the sandbox
    pub async fn execute_command(
        &mut self,
        command: &str,
        args: &[&str],
        working_dir: Option<&Path>,
    ) -> Result<ExecutedCommand> {
        // Check if command is allowed
        if !self.config.allow_commands {
            return Err(SandboxError::CommandError(
                "Command execution is disabled".to_string()
            ));
        }

        if !self.config.command_whitelist.is_empty() {
            let cmd_name = Path::new(command)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(command);

            if !self.config.command_whitelist.iter().any(|w| w == cmd_name) {
                return Err(SandboxError::CommandError(
                    format!("Command '{}' not in whitelist", cmd_name)
                ));
            }
        }

        let work_dir = if self.mode == SandboxMode::DryRun {
            self.original_root.clone()
        } else {
            working_dir
                .map(|p| self.sandbox_root.join(p))
                .unwrap_or_else(|| self.sandbox_root.clone())
        };

        if self.mode == SandboxMode::DryRun {
            tracing::info!("Dry-run: would execute {} {:?} in {:?}", command, args, work_dir);
            let record = ExecutedCommand {
                command: command.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                working_dir: work_dir,
                exit_code: Some(0),
                stdout: "[dry-run]".to_string(),
                stderr: String::new(),
                duration_ms: 0,
            };
            self.executed_commands.push(record.clone());
            return Ok(record);
        }

        use tokio::process::Command;
        let start = std::time::Instant::now();

        let output = Command::new(command)
            .args(args)
            .current_dir(&work_dir)
            .output()
            .await?;

        let duration = start.elapsed();

        let record = ExecutedCommand {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: work_dir,
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms: duration.as_millis() as u64,
        };

        self.executed_commands.push(record.clone());
        Ok(record)
    }

    /// Get diff of all changes made in sandbox
    pub async fn get_diff(&self) -> Result<String> {
        if self.mode == SandboxMode::DryRun {
            return Ok("[dry-run mode - no actual changes]".to_string());
        }

        if self.mode == SandboxMode::GitWorktree {
            use tokio::process::Command;

            let output = Command::new("git")
                .current_dir(&self.sandbox_root)
                .args(["diff", "--no-color"])
                .output()
                .await?;

            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        // For other modes, generate diff manually
        let mut diffs = Vec::new();
        for path in &self.modified_files {
            let sandbox_path = self.sandbox_root.join(path);
            let original_path = self.original_root.join(path);

            let sandbox_content = tokio::fs::read_to_string(&sandbox_path)
                .await
                .unwrap_or_default();
            let original_content = tokio::fs::read_to_string(&original_path)
                .await
                .unwrap_or_default();

            diffs.push(format!(
                "--- a/{}\n+++ b/{}\n{}",
                path.display(),
                path.display(),
                simple_diff(&original_content, &sandbox_content)
            ));
        }

        Ok(diffs.join("\n"))
    }

    /// Apply sandbox changes to the original project
    pub async fn apply_changes(&self) -> Result<ApplyResult> {
        if self.mode == SandboxMode::DryRun {
            return Ok(ApplyResult {
                files_modified: self.modified_files.clone(),
                files_created: Vec::new(),
                files_deleted: Vec::new(),
                success: true,
            });
        }

        let mut result = ApplyResult {
            files_modified: Vec::new(),
            files_created: Vec::new(),
            files_deleted: Vec::new(),
            success: true,
        };

        for relative_path in &self.modified_files {
            let sandbox_path = self.sandbox_root.join(relative_path);
            let original_path = self.original_root.join(relative_path);

            let content = tokio::fs::read(&sandbox_path).await?;

            // Ensure parent directory exists
            if let Some(parent) = original_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            let existed = original_path.exists();
            tokio::fs::write(&original_path, content).await?;

            if existed {
                result.files_modified.push(relative_path.clone());
            } else {
                result.files_created.push(relative_path.clone());
            }
        }

        tracing::info!("Applied {} changes from sandbox", result.files_modified.len() + result.files_created.len());
        Ok(result)
    }

    /// Discard sandbox without applying changes
    pub async fn discard(mut self) -> Result<()> {
        self.active = false;

        match self.mode {
            SandboxMode::GitWorktree => {
                use tokio::process::Command;

                // Remove the worktree
                let _ = Command::new("git")
                    .current_dir(&self.original_root)
                    .args(["worktree", "remove", "--force"])
                    .arg(&self.sandbox_root)
                    .output()
                    .await;
            }
            SandboxMode::FullIsolation | SandboxMode::Overlay => {
                // Remove sandbox directory
                let _ = tokio::fs::remove_dir_all(&self.sandbox_root).await;
            }
            SandboxMode::DryRun => {
                // Nothing to clean up
            }
        }

        tracing::info!("Sandbox discarded: {}", self.id);
        Ok(())
    }

    /// Get sandbox path
    pub fn path(&self) -> &Path {
        &self.sandbox_root
    }

    /// Check if sandbox is still active
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Result of applying sandbox changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub files_modified: Vec<PathBuf>,
    pub files_created: Vec<PathBuf>,
    pub files_deleted: Vec<PathBuf>,
    pub success: bool,
}

/// Manages multiple sandboxes
pub struct SandboxManager {
    config: SandboxConfig,
    sandboxes: HashMap<String, Sandbox>,
}

impl SandboxManager {
    /// Create a new sandbox manager
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            sandboxes: HashMap::new(),
        }
    }

    /// Create a new sandbox
    pub async fn create_sandbox(&mut self, project_root: PathBuf) -> Result<String> {
        let sandbox = Sandbox::create(project_root, self.config.clone()).await?;
        let id = sandbox.id.clone();
        self.sandboxes.insert(id.clone(), sandbox);
        Ok(id)
    }

    /// Get a sandbox by ID
    pub fn get(&self, id: &str) -> Option<&Sandbox> {
        self.sandboxes.get(id)
    }

    /// Get a mutable sandbox by ID
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Sandbox> {
        self.sandboxes.get_mut(id)
    }

    /// Apply sandbox changes and remove it
    pub async fn apply_and_remove(&mut self, id: &str) -> Result<ApplyResult> {
        let sandbox = self.sandboxes.remove(id)
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;

        sandbox.apply_changes().await
    }

    /// Discard a sandbox
    pub async fn discard(&mut self, id: &str) -> Result<()> {
        let sandbox = self.sandboxes.remove(id)
            .ok_or_else(|| SandboxError::NotFound(id.to_string()))?;

        sandbox.discard().await
    }

    /// List all active sandboxes
    pub fn list(&self) -> Vec<&str> {
        self.sandboxes.keys().map(|s| s.as_str()).collect()
    }

    /// Clean up expired sandboxes
    pub async fn cleanup_expired(&mut self) -> Result<Vec<String>> {
        let now = chrono::Utc::now();
        let max_age = chrono::Duration::seconds(self.config.max_lifetime_secs as i64);

        let expired: Vec<String> = self.sandboxes
            .iter()
            .filter(|(_, s)| now - s.created_at > max_age)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &expired {
            self.discard(id).await?;
        }

        Ok(expired)
    }
}

/// Helper function to copy directory recursively
async fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    exclude: &[&str],
) -> Result<()> {
    use tokio::fs;

    fs::create_dir_all(dst).await?;

    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Check if should be excluded
        if exclude.iter().any(|e| file_name_str == *e) {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&file_name);

        if entry.file_type().await?.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path, exclude)).await?;
        } else {
            fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

/// Simple line-by-line diff (placeholder for proper diff)
fn simple_diff(original: &str, modified: &str) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();

    let mut diff = String::new();

    for (i, (orig, modif)) in orig_lines.iter().zip(mod_lines.iter()).enumerate() {
        if orig != modif {
            diff.push_str(&format!("-{}: {}\n", i + 1, orig));
            diff.push_str(&format!("+{}: {}\n", i + 1, modif));
        }
    }

    // Handle different lengths
    if mod_lines.len() > orig_lines.len() {
        for (i, line) in mod_lines[orig_lines.len()..].iter().enumerate() {
            diff.push_str(&format!("+{}: {}\n", orig_lines.len() + i + 1, line));
        }
    } else if orig_lines.len() > mod_lines.len() {
        for (i, line) in orig_lines[mod_lines.len()..].iter().enumerate() {
            diff.push_str(&format!("-{}: {}\n", mod_lines.len() + i + 1, line));
        }
    }

    diff
}
