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
