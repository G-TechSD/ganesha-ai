//! Rollback system for undoing changes
//!
//! Provides checkpointing and rollback capabilities to safely
//! undo changes made by Ganesha.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Rollback-specific errors
#[derive(Error, Debug)]
pub enum RollbackError {
    #[error("Checkpoint creation failed: {0}")]
    CheckpointFailed(String),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("Checkpoint not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, RollbackError>;

/// A checkpoint representing a recoverable state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique checkpoint ID
    pub id: String,
    /// Human-readable name/description
    pub name: String,
    /// When the checkpoint was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Files that were backed up
    pub files: Vec<FileBackup>,
    /// Git commit hash at checkpoint time (if in git repo)
    pub git_commit: Option<String>,
    /// Working directory
    pub working_dir: PathBuf,
    /// Parent checkpoint ID (for checkpoint chains)
    pub parent_id: Option<String>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Backup of a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBackup {
    /// Relative path from working directory
    pub path: PathBuf,
    /// Original content (None if file didn't exist)
    pub original_content: Option<String>,
    /// File existed before changes
    pub existed: bool,
    /// SHA256 hash of original content
    pub content_hash: Option<String>,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(name: &str, working_dir: PathBuf) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            created_at: chrono::Utc::now(),
            files: Vec::new(),
            git_commit: None,
            working_dir,
            parent_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a file backup to this checkpoint
    pub async fn backup_file(&mut self, relative_path: &Path) -> Result<()> {
        let full_path = self.working_dir.join(relative_path);
        let existed = full_path.exists();

        let original_content = if existed {
            match tokio::fs::read_to_string(&full_path).await {
                Ok(content) => Some(content),
                Err(_) => None, // Binary file or read error
            }
        } else {
            None
        };

        let content_hash = original_content.as_ref().map(|c| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            c.hash(&mut hasher);
            format!("{:x}", hasher.finish())
        });

        self.files.push(FileBackup {
            path: relative_path.to_path_buf(),
            original_content,
            existed,
            content_hash,
        });

        Ok(())
    }

    /// Record the current git commit
    pub async fn record_git_state(&mut self) -> Result<()> {
        use tokio::process::Command;

        let output = Command::new("git")
            .current_dir(&self.working_dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                self.git_commit = Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string()
                );
            }
        }

        Ok(())
    }

    /// Get number of files backed up
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }
}

/// Manages checkpoints and rollbacks
pub struct RollbackManager {
    /// Storage directory for checkpoints
    storage_dir: PathBuf,
    /// In-memory checkpoint cache
    checkpoints: HashMap<String, Checkpoint>,
    /// Maximum number of checkpoints to keep
    max_checkpoints: usize,
    /// Current working directory
    working_dir: PathBuf,
}

impl RollbackManager {
    /// Create a new rollback manager
    pub fn new(working_dir: PathBuf) -> Self {
        let storage_dir = working_dir.join(".ganesha").join("checkpoints");
        Self {
            storage_dir,
            checkpoints: HashMap::new(),
            max_checkpoints: 50,
            working_dir,
        }
    }

    /// Create with custom storage directory
    pub fn with_storage(working_dir: PathBuf, storage_dir: PathBuf) -> Self {
        Self {
            storage_dir,
            checkpoints: HashMap::new(),
            max_checkpoints: 50,
            working_dir,
        }
    }

    /// Initialize the rollback manager
    pub async fn initialize(&mut self) -> Result<()> {
        // Create storage directory
        tokio::fs::create_dir_all(&self.storage_dir).await?;

        // Load existing checkpoints
        self.load_checkpoints().await?;

        tracing::info!("Rollback manager initialized with {} checkpoints", self.checkpoints.len());
        Ok(())
    }

    /// Create a new checkpoint
    pub async fn create_checkpoint(&mut self, name: &str) -> Result<String> {
        let mut checkpoint = Checkpoint::new(name, self.working_dir.clone());

        // Record git state if available
        checkpoint.record_git_state().await?;

        // Set parent to most recent checkpoint
        if let Some(recent) = self.most_recent_checkpoint() {
            checkpoint.parent_id = Some(recent.id.clone());
        }

        let id = checkpoint.id.clone();

        // Save checkpoint
        self.save_checkpoint(&checkpoint).await?;
        self.checkpoints.insert(id.clone(), checkpoint);

        // Cleanup old checkpoints if needed
        self.cleanup_old_checkpoints().await?;

        tracing::info!("Created checkpoint: {} ({})", name, id);
        Ok(id)
    }

    /// Create a checkpoint with specific files
    pub async fn create_checkpoint_for_files(
        &mut self,
        name: &str,
        files: &[PathBuf],
    ) -> Result<String> {
        let mut checkpoint = Checkpoint::new(name, self.working_dir.clone());

        // Backup specified files
        for file in files {
            checkpoint.backup_file(file).await?;
        }

        // Record git state
        checkpoint.record_git_state().await?;

        // Set parent
        if let Some(recent) = self.most_recent_checkpoint() {
            checkpoint.parent_id = Some(recent.id.clone());
        }

        let id = checkpoint.id.clone();

        // Save and store
        self.save_checkpoint(&checkpoint).await?;
        self.checkpoints.insert(id.clone(), checkpoint);

        self.cleanup_old_checkpoints().await?;

        tracing::info!("Created checkpoint for {} files: {} ({})", files.len(), name, id);
        Ok(id)
    }

    /// Rollback to a specific checkpoint
    pub async fn rollback(&mut self, checkpoint_id: &str) -> Result<RollbackResult> {
        let checkpoint = self.checkpoints.get(checkpoint_id)
            .ok_or_else(|| RollbackError::NotFound(checkpoint_id.to_string()))?
            .clone();

        let mut result = RollbackResult {
            checkpoint_id: checkpoint_id.to_string(),
            files_restored: Vec::new(),
            files_deleted: Vec::new(),
            git_reset: false,
            success: true,
        };

        // Restore files
        for backup in &checkpoint.files {
            let full_path = self.working_dir.join(&backup.path);

            if let Some(content) = &backup.original_content {
                // Restore original content
                if let Some(parent) = full_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(&full_path, content).await?;
                result.files_restored.push(backup.path.clone());
            } else if !backup.existed {
                // File was created after checkpoint - delete it
                if full_path.exists() {
                    tokio::fs::remove_file(&full_path).await?;
                    result.files_deleted.push(backup.path.clone());
                }
            }
        }

        // Optionally reset git state
        if let Some(commit) = &checkpoint.git_commit {
            // Note: We don't automatically reset git, just log it
            tracing::info!("Checkpoint was at git commit: {}", commit);
            // User can manually: git reset --hard {commit}
        }

        tracing::info!(
            "Rolled back to checkpoint {}: {} files restored, {} files deleted",
            checkpoint_id,
            result.files_restored.len(),
            result.files_deleted.len()
        );

        Ok(result)
    }

    /// Rollback to the most recent checkpoint
    pub async fn rollback_latest(&mut self) -> Result<RollbackResult> {
        let checkpoint_id = self.most_recent_checkpoint()
            .map(|c| c.id.clone())
            .ok_or_else(|| RollbackError::NotFound("No checkpoints available".to_string()))?;

        self.rollback(&checkpoint_id).await
    }

    /// Undo the last N operations (rollback through checkpoint chain)
    pub async fn undo(&mut self, steps: usize) -> Result<RollbackResult> {
        let mut current_id = self.most_recent_checkpoint()
            .map(|c| c.id.clone());

        // Walk back through checkpoint chain
        for _ in 0..steps {
            if let Some(id) = &current_id {
                if let Some(checkpoint) = self.checkpoints.get(id) {
                    current_id = checkpoint.parent_id.clone();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let target_id = current_id
            .ok_or_else(|| RollbackError::NotFound(
                format!("Cannot undo {} steps - not enough checkpoints", steps)
            ))?;

        self.rollback(&target_id).await
    }

    /// Get a checkpoint by ID
    pub fn get_checkpoint(&self, id: &str) -> Option<&Checkpoint> {
        self.checkpoints.get(id)
    }

    /// Get the most recent checkpoint
    pub fn most_recent_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.values()
            .max_by_key(|c| c.created_at)
    }

    /// List all checkpoints
    pub fn list_checkpoints(&self) -> Vec<&Checkpoint> {
        let mut checkpoints: Vec<_> = self.checkpoints.values().collect();
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        checkpoints
    }

    /// Delete a checkpoint
    pub async fn delete_checkpoint(&mut self, id: &str) -> Result<()> {
        // Remove from disk
        let path = self.checkpoint_path(id);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }

        // Remove from memory
        self.checkpoints.remove(id);

        tracing::info!("Deleted checkpoint: {}", id);
        Ok(())
    }

    /// Get the path for a checkpoint file
    fn checkpoint_path(&self, id: &str) -> PathBuf {
        self.storage_dir.join(format!("{}.json", id))
    }

    /// Save a checkpoint to disk
    async fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let path = self.checkpoint_path(&checkpoint.id);
        let json = serde_json::to_string_pretty(checkpoint)?;
        tokio::fs::write(&path, json).await?;
        Ok(())
    }

    /// Load all checkpoints from disk
    async fn load_checkpoints(&mut self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.storage_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        if let Ok(checkpoint) = serde_json::from_str::<Checkpoint>(&content) {
                            self.checkpoints.insert(checkpoint.id.clone(), checkpoint);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load checkpoint {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up old checkpoints beyond max limit
    async fn cleanup_old_checkpoints(&mut self) -> Result<()> {
        if self.checkpoints.len() <= self.max_checkpoints {
            return Ok(());
        }

        // Sort by creation time, oldest first
        let mut sorted: Vec<_> = self.checkpoints.iter()
            .map(|(id, c)| (id.clone(), c.created_at))
            .collect();
        sorted.sort_by_key(|(_, time)| *time);

        // Collect IDs to remove (oldest first)
        let to_remove = self.checkpoints.len() - self.max_checkpoints;
        let ids_to_remove: Vec<String> = sorted.into_iter()
            .take(to_remove)
            .map(|(id, _)| id)
            .collect();

        // Remove oldest checkpoints
        for id in ids_to_remove {
            self.delete_checkpoint(&id).await?;
        }

        Ok(())
    }

    /// Set maximum checkpoints to keep
    pub fn set_max_checkpoints(&mut self, max: usize) {
        self.max_checkpoints = max;
    }
}

/// Result of a rollback operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub checkpoint_id: String,
    pub files_restored: Vec<PathBuf>,
    pub files_deleted: Vec<PathBuf>,
    pub git_reset: bool,
    pub success: bool,
}

impl RollbackResult {
    /// Total number of files affected
    pub fn files_affected(&self) -> usize {
        self.files_restored.len() + self.files_deleted.len()
    }
}

/// Auto-checkpoint wrapper for operations
pub struct AutoCheckpoint<'a> {
    manager: &'a mut RollbackManager,
    checkpoint_id: String,
    committed: bool,
}

impl<'a> AutoCheckpoint<'a> {
    /// Create an auto-checkpoint that will rollback on drop unless committed
    pub async fn new(
        manager: &'a mut RollbackManager,
        name: &str,
        files: &[PathBuf],
    ) -> Result<Self> {
        let checkpoint_id = manager.create_checkpoint_for_files(name, files).await?;
        Ok(Self {
            manager,
            checkpoint_id,
            committed: false,
        })
    }

    /// Commit the changes (don't rollback on drop)
    pub fn commit(mut self) {
        self.committed = true;
    }

    /// Get the checkpoint ID
    pub fn checkpoint_id(&self) -> &str {
        &self.checkpoint_id
    }
}

// Note: Can't implement async Drop, so manual cleanup is needed
// Users should call commit() on success
