//! # Session Management
//!
//! Manages conversation sessions with checkpointing and recovery support.
//!
//! ## Overview
//!
//! The session system is responsible for:
//! - Creating and managing conversation sessions
//! - Tracking conversation history
//! - Creating checkpoints for recovery
//! - Managing working directory context
//! - Persisting session state
//!
//! ## Example
//!
//! ```ignore
//! let mut manager = SessionManager::new("/path/to/sessions")?;
//!
//! // Create a new session
//! let session = manager.create_session("/path/to/project")?;
//!
//! // Add messages to history
//! session.add_message(Message::user("What files are in this project?"));
//! session.add_message(Message::assistant("I found the following files..."));
//!
//! // Create a checkpoint
//! session.checkpoint("After initial exploration")?;
//!
//! // Save session
//! manager.save_session(&session)?;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Errors that can occur in session management
#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Invalid session state: {0}")]
    InvalidState(String),

    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    #[error("Session storage error: {0}")]
    StorageError(String),
}

pub type Result<T> = std::result::Result<T, SessionError>;

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// User message
    User,
    /// Assistant (AI) message
    Assistant,
    /// System message
    System,
    /// Tool call/result
    Tool,
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier
    pub id: String,
    /// Role of the sender
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Associated tool calls (if any)
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID this message is responding to
    pub tool_call_id: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: content.into(),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content: content.into(),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a tool result message
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Tool,
            content: content.into(),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            metadata: HashMap::new(),
        }
    }

    /// Add tool calls to an assistant message
    pub fn with_tool_calls(mut self, calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(calls);
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

/// A tool call made by the assistant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this call
    pub id: String,
    /// Name of the tool
    pub name: String,
    /// Arguments passed to the tool
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Create a new tool call
    pub fn new(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            arguments,
        }
    }
}

/// A checkpoint in the session for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique identifier
    pub id: String,
    /// Human-readable name/description
    pub name: String,
    /// Message index at this checkpoint
    pub message_index: usize,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Working directory at checkpoint
    pub working_directory: PathBuf,
    /// Additional state to restore
    pub state: HashMap<String, serde_json::Value>,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(name: impl Into<String>, message_index: usize, working_directory: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            message_index,
            timestamp: Utc::now(),
            working_directory,
            state: HashMap::new(),
        }
    }

    /// Add state to the checkpoint
    pub fn with_state(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.state.insert(key.into(), json_value);
        }
        self
    }
}

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Session is active
    Active,
    /// Session is paused
    Paused,
    /// Session is completed
    Completed,
    /// Session errored
    Errored,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: Option<String>,
    /// Session status
    pub status: SessionStatus,
    /// Working directory
    pub working_directory: PathBuf,
    /// Project name (if detected)
    pub project_name: Option<String>,
    /// Conversation history
    messages: Vec<Message>,
    /// Checkpoints
    checkpoints: Vec<Checkpoint>,
    /// Session start time
    pub started_at: DateTime<Utc>,
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    /// Total token count (if tracked)
    pub token_count: Option<u64>,
    /// Session metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Session {
    /// Create a new session
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        let working_directory = working_directory.into();
        let project_name = working_directory
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from);

        Self {
            id: Uuid::new_v4().to_string(),
            name: None,
            status: SessionStatus::Active,
            working_directory,
            project_name,
            messages: Vec::new(),
            checkpoints: Vec::new(),
            started_at: Utc::now(),
            last_activity: Utc::now(),
            token_count: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a session with a specific ID (for loading)
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Set the session name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, message: Message) {
        self.last_activity = Utc::now();
        self.messages.push(message);
    }

    /// Get all messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get messages since a specific index
    pub fn messages_since(&self, index: usize) -> &[Message] {
        if index < self.messages.len() {
            &self.messages[index..]
        } else {
            &[]
        }
    }

    /// Get the last N messages
    pub fn last_messages(&self, n: usize) -> &[Message] {
        if n >= self.messages.len() {
            &self.messages
        } else {
            &self.messages[self.messages.len() - n..]
        }
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Create a checkpoint at current state
    pub fn checkpoint(&mut self, name: impl Into<String>) -> &Checkpoint {
        let checkpoint = Checkpoint::new(
            name,
            self.messages.len(),
            self.working_directory.clone(),
        );
        self.checkpoints.push(checkpoint);
        self.checkpoints.last().unwrap()
    }

    /// Get all checkpoints
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// Get a checkpoint by ID
    pub fn get_checkpoint(&self, id: &str) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.id == id)
    }

    /// Restore to a checkpoint (truncates messages after checkpoint)
    pub fn restore_to_checkpoint(&mut self, checkpoint_id: &str) -> Result<()> {
        let checkpoint = self
            .checkpoints
            .iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or_else(|| SessionError::CheckpointNotFound(checkpoint_id.to_string()))?;

        let message_index = checkpoint.message_index;
        self.messages.truncate(message_index);
        self.working_directory = checkpoint.working_directory.clone();
        self.last_activity = Utc::now();

        // Remove checkpoints after this one
        let checkpoint_timestamp = checkpoint.timestamp;
        self.checkpoints
            .retain(|c| c.timestamp <= checkpoint_timestamp);

        info!(
            "Restored session to checkpoint: {} (message index: {})",
            checkpoint_id, message_index
        );

        Ok(())
    }

    /// Clear conversation history but keep session metadata
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.checkpoints.clear();
        self.last_activity = Utc::now();
    }

    /// Set session status
    pub fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
        self.last_activity = Utc::now();
    }

    /// Update working directory
    pub fn set_working_directory(&mut self, path: impl Into<PathBuf>) {
        self.working_directory = path.into();
        self.last_activity = Utc::now();
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), json_value);
        }
        self
    }

    /// Get session duration
    pub fn duration(&self) -> chrono::Duration {
        self.last_activity - self.started_at
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.status == SessionStatus::Active
    }

    /// Generate a summary of the session
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            status: self.status,
            working_directory: self.working_directory.clone(),
            project_name: self.project_name.clone(),
            message_count: self.messages.len(),
            checkpoint_count: self.checkpoints.len(),
            started_at: self.started_at,
            last_activity: self.last_activity,
            duration_seconds: self.duration().num_seconds() as u64,
        }
    }
}

/// Summary of a session (lightweight)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Session ID
    pub id: String,
    /// Session name
    pub name: Option<String>,
    /// Status
    pub status: SessionStatus,
    /// Working directory
    pub working_directory: PathBuf,
    /// Project name
    pub project_name: Option<String>,
    /// Number of messages
    pub message_count: usize,
    /// Number of checkpoints
    pub checkpoint_count: usize,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// Last activity
    pub last_activity: DateTime<Utc>,
    /// Duration in seconds
    pub duration_seconds: u64,
}

/// Manages multiple sessions with persistence
pub struct SessionManager {
    /// Directory for session storage
    storage_dir: PathBuf,
    /// Currently loaded sessions (cache)
    sessions: HashMap<String, Session>,
    /// Active session ID
    active_session_id: Option<String>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(storage_dir: impl Into<PathBuf>) -> Result<Self> {
        let storage_dir = storage_dir.into();

        // Create storage directory if it doesn't exist
        if !storage_dir.exists() {
            std::fs::create_dir_all(&storage_dir)?;
        }

        Ok(Self {
            storage_dir,
            sessions: HashMap::new(),
            active_session_id: None,
        })
    }

    /// Create a new session
    pub fn create_session(&mut self, working_directory: impl Into<PathBuf>) -> Result<&mut Session> {
        let session = Session::new(working_directory);
        let session_id = session.id.clone();

        info!("Created new session: {}", session_id);
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());

        Ok(self.sessions.get_mut(&session_id).unwrap())
    }

    /// Get a session by ID
    pub fn get_session(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Get a mutable session by ID
    pub fn get_session_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    /// Get the active session
    pub fn active_session(&self) -> Option<&Session> {
        self.active_session_id
            .as_ref()
            .and_then(|id| self.sessions.get(id))
    }

    /// Get the active session mutably
    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        if let Some(id) = self.active_session_id.clone() {
            self.sessions.get_mut(&id)
        } else {
            None
        }
    }

    /// Set the active session
    pub fn set_active_session(&mut self, id: impl Into<String>) -> Result<()> {
        let id = id.into();
        if !self.sessions.contains_key(&id) {
            // Try to load from disk
            self.load_session(&id)?;
        }
        self.active_session_id = Some(id);
        Ok(())
    }

    /// Save a session to disk
    pub fn save_session(&self, session: &Session) -> Result<()> {
        let file_path = self.session_file_path(&session.id);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(session)?;
        std::fs::write(&file_path, json)?;

        debug!("Saved session {} to {:?}", session.id, file_path);
        Ok(())
    }

    /// Load a session from disk
    pub fn load_session(&mut self, id: &str) -> Result<&Session> {
        if self.sessions.contains_key(id) {
            return Ok(self.sessions.get(id).unwrap());
        }

        let file_path = self.session_file_path(id);
        if !file_path.exists() {
            return Err(SessionError::NotFound(id.to_string()));
        }

        let json = std::fs::read_to_string(&file_path)?;
        let session: Session = serde_json::from_str(&json)?;

        debug!("Loaded session {} from {:?}", id, file_path);
        self.sessions.insert(id.to_string(), session);
        Ok(self.sessions.get(id).unwrap())
    }

    /// Delete a session
    pub fn delete_session(&mut self, id: &str) -> Result<()> {
        self.sessions.remove(id);

        let file_path = self.session_file_path(id);
        if file_path.exists() {
            std::fs::remove_file(&file_path)?;
        }

        if self.active_session_id.as_deref() == Some(id) {
            self.active_session_id = None;
        }

        info!("Deleted session: {}", id);
        Ok(())
    }

    /// List all sessions (loads summaries from disk)
    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut summaries = Vec::new();

        // Add in-memory sessions
        for session in self.sessions.values() {
            summaries.push(session.summary());
        }

        // Load session files not already in memory
        if self.storage_dir.exists() {
            for entry in std::fs::read_dir(&self.storage_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map_or(false, |ext| ext == "json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !self.sessions.contains_key(stem) {
                            // Load just enough to create summary
                            if let Ok(json) = std::fs::read_to_string(&path) {
                                if let Ok(session) = serde_json::from_str::<Session>(&json) {
                                    summaries.push(session.summary());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by last activity (most recent first)
        summaries.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        Ok(summaries)
    }

    /// Find sessions for a project
    pub fn sessions_for_project(&self, project_path: &Path) -> Result<Vec<SessionSummary>> {
        let all_sessions = self.list_sessions()?;
        let canonical_path = project_path.canonicalize().unwrap_or_else(|_| project_path.to_path_buf());

        Ok(all_sessions
            .into_iter()
            .filter(|s| {
                s.working_directory
                    .canonicalize()
                    .map(|p| p.starts_with(&canonical_path))
                    .unwrap_or(false)
            })
            .collect())
    }

    /// Get most recent session for a project
    pub fn most_recent_for_project(&self, project_path: &Path) -> Result<Option<SessionSummary>> {
        let sessions = self.sessions_for_project(project_path)?;
        Ok(sessions.into_iter().next())
    }

    /// Clean up old sessions
    pub fn cleanup_old_sessions(&mut self, max_age_days: u32) -> Result<Vec<String>> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let mut deleted = Vec::new();

        let summaries = self.list_sessions()?;
        for summary in summaries {
            if summary.last_activity < cutoff {
                if let Err(e) = self.delete_session(&summary.id) {
                    warn!("Failed to delete old session {}: {}", summary.id, e);
                } else {
                    deleted.push(summary.id);
                }
            }
        }

        if !deleted.is_empty() {
            info!("Cleaned up {} old sessions", deleted.len());
        }

        Ok(deleted)
    }

    /// Get session file path
    fn session_file_path(&self, id: &str) -> PathBuf {
        self.storage_dir.join(format!("{}.json", id))
    }

    /// Save all in-memory sessions
    pub fn save_all(&self) -> Result<()> {
        for session in self.sessions.values() {
            self.save_session(session)?;
        }
        Ok(())
    }
}

/// Async version of session operations
impl SessionManager {
    /// Async save session
    pub async fn save_session_async(&self, session: &Session) -> Result<()> {
        let file_path = self.session_file_path(&session.id);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(session)?;
        tokio::fs::write(&file_path, json).await?;

        debug!("Saved session {} to {:?}", session.id, file_path);
        Ok(())
    }

    /// Async load session
    pub async fn load_session_async(&mut self, id: &str) -> Result<()> {
        if self.sessions.contains_key(id) {
            return Ok(());
        }

        let file_path = self.session_file_path(id);
        if !file_path.exists() {
            return Err(SessionError::NotFound(id.to_string()));
        }

        let json = tokio::fs::read_to_string(&file_path).await?;
        let session: Session = serde_json::from_str(&json)?;

        debug!("Loaded session {} from {:?}", id, file_path);
        self.sessions.insert(id.to_string(), session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello");
        assert_eq!(user_msg.role, MessageRole::User);
        assert_eq!(user_msg.content, "Hello");

        let assistant_msg = Message::assistant("Hi there!");
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
    }

    #[test]
    fn test_session_creation() {
        let session = Session::new("/path/to/project");
        assert!(!session.id.is_empty());
        assert_eq!(session.working_directory, PathBuf::from("/path/to/project"));
        assert_eq!(session.project_name, Some("project".to_string()));
        assert_eq!(session.status, SessionStatus::Active);
    }

    #[test]
    fn test_session_messages() {
        let mut session = Session::new("/project");

        session.add_message(Message::user("Hello"));
        session.add_message(Message::assistant("Hi!"));
        session.add_message(Message::user("How are you?"));

        assert_eq!(session.message_count(), 3);
        assert_eq!(session.last_messages(2).len(), 2);
        assert_eq!(session.messages_since(1).len(), 2);
    }

    #[test]
    fn test_checkpoints() {
        let mut session = Session::new("/project");

        session.add_message(Message::user("First"));
        session.add_message(Message::assistant("Response 1"));
        session.checkpoint("After first exchange");

        session.add_message(Message::user("Second"));
        session.add_message(Message::assistant("Response 2"));

        assert_eq!(session.message_count(), 4);
        assert_eq!(session.checkpoints().len(), 1);

        // Restore to checkpoint
        let checkpoint_id = session.checkpoints()[0].id.clone();
        session.restore_to_checkpoint(&checkpoint_id).unwrap();

        assert_eq!(session.message_count(), 2);
    }

    #[test]
    fn test_session_manager() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).unwrap();

        // Create a session
        let session = manager.create_session("/project").unwrap();
        let session_id = session.id.clone();
        session.add_message(Message::user("Test"));

        // Save session
        let session_clone = session.clone();
        manager.save_session(&session_clone).unwrap();

        // Create new manager and load
        let mut manager2 = SessionManager::new(temp_dir.path()).unwrap();
        manager2.load_session(&session_id).unwrap();

        let loaded = manager2.get_session(&session_id).unwrap();
        assert_eq!(loaded.message_count(), 1);
    }

    #[test]
    fn test_session_summary() {
        let session = Session::new("/project")
            .with_name("Test Session");

        let summary = session.summary();
        assert_eq!(summary.name, Some("Test Session".to_string()));
        assert_eq!(summary.message_count, 0);
        assert_eq!(summary.checkpoint_count, 0);
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).unwrap();

        // Create multiple sessions
        manager.create_session("/project1").unwrap();
        manager.create_session("/project2").unwrap();

        let summaries = manager.list_sessions().unwrap();
        assert_eq!(summaries.len(), 2);
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("hello world");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "hello world");
        assert!(!msg.id.is_empty());
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("I can help");
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(msg.content, "I can help");
    }

    #[test]
    fn test_message_system() {
        let msg = Message::system("You are a helpful assistant");
        assert_eq!(msg.role, MessageRole::System);
    }

    #[test]
    fn test_message_tool_result() {
        let msg = Message::tool_result("call-123", "result data");
        assert_eq!(msg.role, MessageRole::Tool);
        assert_eq!(msg.tool_call_id, Some("call-123".to_string()));
    }

    #[test]
    fn test_message_with_tool_calls() {
        let calls = vec![ToolCall::new("read_file", serde_json::json!({"path": "test.rs"}))];
        let msg = Message::assistant("Let me read that").with_tool_calls(calls);
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_message_with_metadata() {
        let msg = Message::user("test").with_metadata("source", "cli");
        assert!(msg.metadata.contains_key("source"));
    }

    #[test]
    fn test_message_unique_ids() {
        let m1 = Message::user("a");
        let m2 = Message::user("b");
        assert_ne!(m1.id, m2.id);
    }

    #[test]
    fn test_tool_call_new() {
        let tc = ToolCall::new("run_command", serde_json::json!({"cmd": "ls"}));
        assert_eq!(tc.name, "run_command");
        assert!(!tc.id.is_empty());
    }

    #[test]
    fn test_session_with_id() {
        let s = Session::new("/tmp").with_id("custom-id");
        assert_eq!(s.id, "custom-id");
    }

    #[test]
    fn test_session_with_name() {
        let s = Session::new("/tmp").with_name("My Project");
        assert_eq!(s.name, Some("My Project".to_string()));
    }

    #[test]
    fn test_session_message_count() {
        let mut s = Session::new("/tmp");
        assert_eq!(s.message_count(), 0);
        s.add_message(Message::user("hello"));
        s.add_message(Message::assistant("hi"));
        assert_eq!(s.message_count(), 2);
    }

    #[test]
    fn test_session_last_messages() {
        let mut s = Session::new("/tmp");
        s.add_message(Message::user("1"));
        s.add_message(Message::user("2"));
        s.add_message(Message::user("3"));
        let last2 = s.last_messages(2);
        assert_eq!(last2.len(), 2);
        assert_eq!(last2[0].content, "2");
        assert_eq!(last2[1].content, "3");
    }

    #[test]
    fn test_session_messages_since() {
        let mut s = Session::new("/tmp");
        s.add_message(Message::user("a"));
        s.add_message(Message::user("b"));
        s.add_message(Message::user("c"));
        let since1 = s.messages_since(1);
        assert_eq!(since1.len(), 2);
    }

    #[test]
    fn test_session_clear_history() {
        let mut s = Session::new("/tmp");
        s.add_message(Message::user("msg"));
        assert_eq!(s.message_count(), 1);
        s.clear_history();
        assert_eq!(s.message_count(), 0);
    }

    #[test]
    fn test_session_status_transitions() {
        let mut s = Session::new("/tmp");
        assert_eq!(s.status, SessionStatus::Active);
        s.set_status(SessionStatus::Paused);
        assert_eq!(s.status, SessionStatus::Paused);
        s.set_status(SessionStatus::Completed);
        assert_eq!(s.status, SessionStatus::Completed);
    }

    #[test]
    fn test_session_checkpoint_creation() {
        let mut s = Session::new("/tmp");
        s.add_message(Message::user("before checkpoint"));
        let cp = s.checkpoint("save point");
        assert_eq!(cp.name, "save point");
        assert!(!cp.id.is_empty());
    }

    #[test]
    fn test_session_get_checkpoint() {
        let mut s = Session::new("/tmp");
        s.add_message(Message::user("msg"));
        let cp_id = s.checkpoint("cp1").id.clone();
        assert!(s.get_checkpoint(&cp_id).is_some());
        assert!(s.get_checkpoint("nonexistent").is_none());
    }

    #[test]
    fn test_checkpoint_with_state() {
        let cp = Checkpoint::new("test", 0, PathBuf::from("/tmp"))
            .with_state("key", "value");
        assert!(cp.state.contains_key("key"));
    }

}
