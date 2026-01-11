//! Session Rollback System
//!
//! Tracks file changes and enables undoing session modifications.
//! Works by storing file snapshots before modifications.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;

/// A snapshot of a file before modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: String,
    pub existed: bool,
    pub content_hash: String,
    pub size: u64,
    pub modified_at: DateTime<Utc>,
    /// Compressed content (gzip)
    #[serde(skip)]
    pub content: Option<Vec<u8>>,
}

/// A complete session rollback record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub session_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub description: String,
    pub snapshots: Vec<FileSnapshot>,
    pub commands: Vec<CommandRecord>,
    pub applied: bool,
}

/// Record of a command that was executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    pub command: String,
    pub cwd: String,
    pub executed_at: DateTime<Utc>,
    pub success: bool,
    /// Inverse command if applicable (e.g., "rm file" -> restore from snapshot)
    pub inverse: Option<String>,
}

/// The rollback manager
pub struct RollbackManager {
    base_dir: PathBuf,
    current_session: Option<Uuid>,
    snapshots: HashMap<String, FileSnapshot>,
    commands: Vec<CommandRecord>,
}

impl RollbackManager {
    pub fn new() -> Self {
        let base_dir = Self::get_base_dir();
        fs::create_dir_all(&base_dir).ok();

        Self {
            base_dir,
            current_session: None,
            snapshots: HashMap::new(),
            commands: vec![],
        }
    }

    fn get_base_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".ganesha").join("rollback")
    }

    /// Start tracking a new session
    pub fn start_session(&mut self, session_id: Uuid) {
        self.current_session = Some(session_id);
        self.snapshots.clear();
        self.commands.clear();

        // Create session directory
        let session_dir = self.base_dir.join(session_id.to_string());
        fs::create_dir_all(&session_dir).ok();
    }

    /// Snapshot a file before modification
    pub fn snapshot_file(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let session_id = self.current_session
            .ok_or("No active session")?;

        // Don't snapshot the same file twice
        if self.snapshots.contains_key(path) {
            return Ok(());
        }

        let full_path = PathBuf::from(path);
        let existed = full_path.exists();

        let (content, size, hash) = if existed {
            let content = fs::read(&full_path)?;
            let size = content.len() as u64;
            let hash = format!("{:x}", md5::compute(&content));

            // Compress content
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&content)?;
            let compressed = encoder.finish()?;

            (Some(compressed), size, hash)
        } else {
            (None, 0, String::new())
        };

        let snapshot = FileSnapshot {
            path: path.to_string(),
            existed,
            content_hash: hash,
            size,
            modified_at: Utc::now(),
            content,
        };

        // Save snapshot to disk
        self.save_snapshot(&session_id, &snapshot)?;

        self.snapshots.insert(path.to_string(), snapshot);
        Ok(())
    }

    /// Save a snapshot to disk
    fn save_snapshot(&self, session_id: &Uuid, snapshot: &FileSnapshot) -> Result<(), Box<dyn std::error::Error>> {
        let session_dir = self.base_dir.join(session_id.to_string());
        fs::create_dir_all(&session_dir)?;

        // Create a safe filename from path
        let safe_name = snapshot.path
            .replace("/", "_")
            .replace("\\", "_")
            .replace(":", "_");

        // Save metadata
        let meta_path = session_dir.join(format!("{}.meta.json", safe_name));
        let meta = serde_json::to_string_pretty(snapshot)?;
        fs::write(&meta_path, meta)?;

        // Save content if it exists
        if let Some(ref content) = snapshot.content {
            let content_path = session_dir.join(format!("{}.content.gz", safe_name));
            fs::write(&content_path, content)?;
        }

        Ok(())
    }

    /// Record a command execution
    pub fn record_command(&mut self, command: &str, cwd: &str, success: bool) {
        let inverse = self.compute_inverse(command);

        self.commands.push(CommandRecord {
            command: command.to_string(),
            cwd: cwd.to_string(),
            executed_at: Utc::now(),
            success,
            inverse,
        });
    }

    /// Try to compute an inverse command
    fn compute_inverse(&self, command: &str) -> Option<String> {
        // Some commands have obvious inverses
        let parts: Vec<&str> = command.split_whitespace().collect();

        match parts.get(0).map(|s| *s) {
            Some("mkdir") => {
                if let Some(dir) = parts.get(1) {
                    return Some(format!("rmdir {}", dir));
                }
            }
            Some("touch") => {
                if let Some(file) = parts.get(1) {
                    return Some(format!("rm {}", file));
                }
            }
            Some("ln") => {
                if parts.contains(&"-s") {
                    if let Some(link) = parts.last() {
                        return Some(format!("rm {}", link));
                    }
                }
            }
            _ => {}
        }

        // Most commands need file restoration, not inverse commands
        None
    }

    /// End the current session and save the rollback record
    pub fn end_session(&mut self, description: &str) -> Result<Option<RollbackRecord>, Box<dyn std::error::Error>> {
        let session_id = match self.current_session.take() {
            Some(id) => id,
            None => return Ok(None),
        };

        if self.snapshots.is_empty() && self.commands.is_empty() {
            // Nothing to rollback
            return Ok(None);
        }

        let record = RollbackRecord {
            session_id,
            created_at: Utc::now(),
            description: description.to_string(),
            snapshots: self.snapshots.values().cloned().collect(),
            commands: self.commands.clone(),
            applied: false,
        };

        // Save the record
        let record_path = self.base_dir.join(format!("{}.record.json", session_id));
        let json = serde_json::to_string_pretty(&record)?;
        fs::write(&record_path, json)?;

        self.snapshots.clear();
        self.commands.clear();

        Ok(Some(record))
    }

    /// List available rollback sessions
    pub fn list_sessions(&self) -> Result<Vec<RollbackRecord>, Box<dyn std::error::Error>> {
        let mut records = vec![];

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false)
                && path.file_name().map(|n| n.to_string_lossy().ends_with(".record.json")).unwrap_or(false)
            {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(record) = serde_json::from_str::<RollbackRecord>(&content) {
                        if !record.applied {
                            records.push(record);
                        }
                    }
                }
            }
        }

        // Sort by date, newest first
        records.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(records)
    }

    /// Rollback a session
    pub fn rollback_session(&mut self, session_id: Uuid) -> Result<RollbackResult, Box<dyn std::error::Error>> {
        let record_path = self.base_dir.join(format!("{}.record.json", session_id));
        let content = fs::read_to_string(&record_path)?;
        let mut record: RollbackRecord = serde_json::from_str(&content)?;

        if record.applied {
            return Err("Session already rolled back".into());
        }

        let mut result = RollbackResult {
            session_id,
            files_restored: vec![],
            files_deleted: vec![],
            errors: vec![],
        };

        let session_dir = self.base_dir.join(session_id.to_string());

        // Restore each file
        for snapshot in &record.snapshots {
            let safe_name = snapshot.path
                .replace("/", "_")
                .replace("\\", "_")
                .replace(":", "_");

            if snapshot.existed {
                // Restore from snapshot
                let content_path = session_dir.join(format!("{}.content.gz", safe_name));

                if content_path.exists() {
                    match self.restore_file(&snapshot.path, &content_path) {
                        Ok(_) => result.files_restored.push(snapshot.path.clone()),
                        Err(e) => result.errors.push(format!("{}: {}", snapshot.path, e)),
                    }
                } else {
                    result.errors.push(format!("{}: Snapshot content not found", snapshot.path));
                }
            } else {
                // File didn't exist before, delete it
                let path = PathBuf::from(&snapshot.path);
                if path.exists() {
                    match fs::remove_file(&path) {
                        Ok(_) => result.files_deleted.push(snapshot.path.clone()),
                        Err(e) => result.errors.push(format!("{}: {}", snapshot.path, e)),
                    }
                }
            }
        }

        // Mark as applied
        record.applied = true;
        let json = serde_json::to_string_pretty(&record)?;
        fs::write(&record_path, json)?;

        Ok(result)
    }

    /// Restore a file from compressed snapshot
    fn restore_file(&self, target: &str, content_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let compressed = fs::read(content_path)?;
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut content = Vec::new();
        decoder.read_to_end(&mut content)?;

        // Create parent directories if needed
        let target_path = PathBuf::from(target);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&target_path, content)?;
        Ok(())
    }

    /// Clean up old rollback data (older than days)
    pub fn cleanup(&self, days: i64) -> Result<usize, Box<dyn std::error::Error>> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let mut removed = 0;

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Check if session directory is old
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(uuid) = Uuid::parse_str(name) {
                        let record_path = self.base_dir.join(format!("{}.record.json", uuid));
                        if let Ok(content) = fs::read_to_string(&record_path) {
                            if let Ok(record) = serde_json::from_str::<RollbackRecord>(&content) {
                                if record.created_at < cutoff || record.applied {
                                    fs::remove_dir_all(&path).ok();
                                    fs::remove_file(&record_path).ok();
                                    removed += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a rollback operation
#[derive(Debug, Clone)]
pub struct RollbackResult {
    pub session_id: Uuid,
    pub files_restored: Vec<String>,
    pub files_deleted: Vec<String>,
    pub errors: Vec<String>,
}

impl RollbackResult {
    pub fn success(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn print_summary(&self) {
        println!("\n\x1b[1;36mRollback Summary:\x1b[0m");
        println!("  Session: {}", self.session_id);

        if !self.files_restored.is_empty() {
            println!("\n  \x1b[32mRestored:\x1b[0m");
            for file in &self.files_restored {
                println!("    ✓ {}", file);
            }
        }

        if !self.files_deleted.is_empty() {
            println!("\n  \x1b[33mDeleted (didn't exist before):\x1b[0m");
            for file in &self.files_deleted {
                println!("    ✗ {}", file);
            }
        }

        if !self.errors.is_empty() {
            println!("\n  \x1b[31mErrors:\x1b[0m");
            for error in &self.errors {
                println!("    ! {}", error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_manager_creation() {
        let manager = RollbackManager::new();
        assert!(manager.current_session.is_none());
    }

    #[test]
    fn test_session_lifecycle() {
        let mut manager = RollbackManager::new();
        let session_id = Uuid::new_v4();

        manager.start_session(session_id);
        assert_eq!(manager.current_session, Some(session_id));

        manager.record_command("echo test", "/tmp", true);
        assert_eq!(manager.commands.len(), 1);
    }

    #[test]
    fn test_compute_inverse() {
        let manager = RollbackManager::new();

        assert_eq!(
            manager.compute_inverse("mkdir test"),
            Some("rmdir test".to_string())
        );

        assert_eq!(
            manager.compute_inverse("touch file.txt"),
            Some("rm file.txt".to_string())
        );

        assert_eq!(
            manager.compute_inverse("echo hello"),
            None
        );
    }
}
