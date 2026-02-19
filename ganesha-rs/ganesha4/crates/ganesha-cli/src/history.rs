//! # History Management
//!
//! Conversation history and session persistence.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use chrono::{DateTime, Utc};

/// A conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u32>,
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub working_dir: PathBuf,
    pub messages: Vec<HistoryMessage>,
    #[serde(default)]
    pub files_in_context: Vec<PathBuf>,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl Session {
    /// Create a new session
    pub fn new(working_dir: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: None,
            created_at: now,
            updated_at: now,
            working_dir,
            messages: Vec::new(),
            files_in_context: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, role: &str, content: &str, model: Option<String>) {
        self.messages.push(HistoryMessage {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
            model,
            tokens: None,
        });
        self.updated_at = Utc::now();
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Export to markdown
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Session: {}\n\n", self.name.as_deref().unwrap_or(&self.id)));
        md.push_str(&format!("Created: {}\n", self.created_at.format("%Y-%m-%d %H:%M:%S")));
        md.push_str(&format!("Working Directory: {}\n\n", self.working_dir.display()));
        md.push_str("---\n\n");

        for msg in &self.messages {
            let role_prefix = match msg.role.as_str() {
                "user" => "**You:**",
                "assistant" => "**Assistant:**",
                "system" => "*System:*",
                _ => &msg.role,
            };

            md.push_str(&format!("{}\n\n{}\n\n", role_prefix, msg.content));
        }

        md
    }
}

/// Session manager
pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        let sessions_dir = dirs::data_dir()
            .map(|d| d.join("ganesha").join("sessions"))
            .unwrap_or_else(|| PathBuf::from(".ganesha/sessions"));

        Self { sessions_dir }
    }

    /// Create with a custom directory
    pub fn with_dir(dir: PathBuf) -> Self {
        Self { sessions_dir: dir }
    }

    /// Ensure sessions directory exists
    async fn ensure_dir(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.sessions_dir).await?;
        Ok(())
    }

    /// Save a session
    pub async fn save(&self, session: &Session) -> anyhow::Result<()> {
        self.ensure_dir().await?;

        let filename = format!("{}.json", session.id);
        let path = self.sessions_dir.join(filename);
        let content = serde_json::to_string_pretty(session)?;
        fs::write(path, content).await?;

        Ok(())
    }

    /// Load a session by ID
    pub async fn load(&self, id: &str) -> anyhow::Result<Session> {
        let filename = format!("{}.json", id);
        let path = self.sessions_dir.join(filename);
        let content = fs::read_to_string(path).await?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// List all sessions
    pub async fn list(&self) -> anyhow::Result<Vec<SessionSummary>> {
        self.ensure_dir().await?;

        let mut summaries = Vec::new();
        let mut entries = fs::read_dir(&self.sessions_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path).await {
                    if let Ok(session) = serde_json::from_str::<Session>(&content) {
                        summaries.push(SessionSummary {
                            id: session.id,
                            name: session.name,
                            created_at: session.created_at,
                            updated_at: session.updated_at,
                            message_count: session.messages.len(),
                        });
                    }
                }
            }
        }

        // Sort by updated_at descending
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(summaries)
    }

    /// Delete a session
    pub async fn delete(&self, id: &str) -> anyhow::Result<()> {
        let filename = format!("{}.json", id);
        let path = self.sessions_dir.join(filename);
        fs::remove_file(path).await?;
        Ok(())
    }

    /// Find sessions by working directory
    pub async fn find_by_dir(&self, dir: &Path) -> anyhow::Result<Vec<SessionSummary>> {
        let all = self.list().await?;
        let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());

        let mut matching = Vec::new();
        for summary in all {
            if let Ok(session) = self.load(&summary.id).await {
                if session.working_dir == canonical_dir {
                    matching.push(summary);
                }
            }
        }

        Ok(matching)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a session for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_session_creation() {
        let session = Session::new(PathBuf::from("/tmp/test"));
        assert!(!session.id.is_empty());
        assert!(session.name.is_none());
        assert_eq!(session.messages.len(), 0);
        assert_eq!(session.working_dir, PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_add_messages() {
        let mut session = Session::new(PathBuf::from("."));
        session.add_message("user", "Hello", None);
        session.add_message("assistant", "Hi!", Some("gpt-4".to_string()));

        assert_eq!(session.message_count(), 2);
        assert_eq!(session.messages[0].role, "user");
        assert_eq!(session.messages[0].content, "Hello");
        assert!(session.messages[0].model.is_none());
        assert_eq!(session.messages[1].model, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_updated_at_changes() {
        let mut session = Session::new(PathBuf::from("."));
        let created = session.created_at;
        // Add slight delay to ensure timestamp differs
        session.add_message("user", "test", None);
        // updated_at should be >= created_at
        assert!(session.updated_at >= created);
    }

    #[test]
    fn test_to_markdown() {
        let mut session = Session::new(PathBuf::from("/home/user/project"));
        session.name = Some("Test Session".to_string());
        session.add_message("user", "What is Rust?", None);
        session.add_message("assistant", "Rust is a systems programming language.", None);

        let md = session.to_markdown();
        assert!(md.contains("# Session: Test Session"));
        assert!(md.contains("**You:**"));
        assert!(md.contains("What is Rust?"));
        assert!(md.contains("**Assistant:**"));
        assert!(md.contains("Rust is a systems programming language."));
    }

    #[test]
    fn test_session_serialization() {
        let mut session = Session::new(PathBuf::from("."));
        session.add_message("user", "Hello", None);
        session.metadata.insert("key".to_string(), "value".to_string());

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, session.id);
        assert_eq!(deserialized.message_count(), 1);
        assert_eq!(deserialized.metadata.get("key").unwrap(), "value");
    }

    #[test]
    fn test_session_files_in_context() {
        let mut session = Session::new(PathBuf::from("."));
        session.files_in_context.push(PathBuf::from("src/main.rs"));
        session.files_in_context.push(PathBuf::from("Cargo.toml"));

        assert_eq!(session.files_in_context.len(), 2);

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.files_in_context.len(), 2);
    }

    #[test]
    fn test_history_message_optional_fields() {
        let msg = HistoryMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: Utc::now(),
            model: None,
            tokens: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Optional fields should be skipped
        assert!(!json.contains("model"));
        assert!(!json.contains("tokens"));
    }

    #[test]
    fn test_session_summary() {
        let summary = SessionSummary {
            id: "test-id".to_string(),
            name: Some("My Session".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            message_count: 42,
        };
        assert_eq!(summary.message_count, 42);
        assert_eq!(summary.name, Some("My Session".to_string()));
    }

    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::with_dir(PathBuf::from("/tmp/test-sessions"));
        // Just verifies construction doesn't panic
        let _ = manager;
    }

    #[test]
    fn test_markdown_with_system_messages() {
        let mut session = Session::new(PathBuf::from("."));
        session.add_message("system", "You are a helpful assistant.", None);
        session.add_message("user", "Hi", None);

        let md = session.to_markdown();
        assert!(md.contains("*System:*"));
        assert!(md.contains("You are a helpful assistant."));
    }

    #[test]
    fn test_markdown_without_name_uses_id() {
        let session = Session::new(PathBuf::from("/tmp"));
        let md = session.to_markdown();
        assert!(md.contains(&format!("# Session: {}", session.id)));
    }

    #[test]
    fn test_empty_session_markdown() {
        let session = Session::new(PathBuf::from("."));
        let md = session.to_markdown();
        assert!(md.contains("---"));
        assert!(!md.contains("**You:**"));
    }

    #[test]
    fn test_message_with_tokens() {
        let msg = HistoryMessage {
            role: "assistant".to_string(),
            content: "Response".to_string(),
            timestamp: Utc::now(),
            model: Some("gpt-4".to_string()),
            tokens: Some(150),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tokens"));
        assert!(json.contains("150"));
        assert!(json.contains("model"));
        assert!(json.contains("gpt-4"));
    }

    #[test]
    fn test_session_metadata_operations() {
        let mut session = Session::new(PathBuf::from("."));
        session.metadata.insert("provider".to_string(), "openai".to_string());
        session.metadata.insert("mode".to_string(), "chat".to_string());
        
        assert_eq!(session.metadata.len(), 2);
        assert_eq!(session.metadata.get("provider").unwrap(), "openai");
        
        session.metadata.remove("mode");
        assert_eq!(session.metadata.len(), 1);
    }

    #[test]
    fn test_session_id_is_uuid() {
        let session = Session::new(PathBuf::from("."));
        // UUID v4 format: 8-4-4-4-12 hex chars
        assert_eq!(session.id.len(), 36);
        assert_eq!(session.id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn test_multiple_messages_ordering() {
        let mut session = Session::new(PathBuf::from("."));
        session.add_message("user", "First", None);
        session.add_message("assistant", "Second", None);
        session.add_message("user", "Third", None);
        
        assert_eq!(session.messages[0].content, "First");
        assert_eq!(session.messages[1].content, "Second");
        assert_eq!(session.messages[2].content, "Third");
    }

    #[test]
    fn test_session_summary_fields() {
        let summary = SessionSummary {
            id: "abc-123".to_string(),
            name: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            message_count: 0,
        };
        assert!(summary.name.is_none());
        assert_eq!(summary.id, "abc-123");
    }

    #[test]
    fn test_markdown_multiple_roles() {
        let mut session = Session::new(PathBuf::from("."));
        session.add_message("system", "System prompt", None);
        session.add_message("user", "User message", None);
        session.add_message("assistant", "Assistant reply", None);
        session.add_message("tool", "Tool output", None);
        
        let md = session.to_markdown();
        assert!(md.contains("*System:*"));
        assert!(md.contains("**You:**"));
        assert!(md.contains("**Assistant:**"));
        assert!(md.contains("tool"));  // Unknown roles just use the role name
    }

}
