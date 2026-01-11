//! Ganesha Scalable Memory System - SQLite Backend
//!
//! Replaces the single JSON file with SQLite for:
//! - Fast queries across thousands of sessions
//! - Efficient storage and retrieval
//! - Full-text search on session content
//! - No file size limits

use rusqlite::{Connection, Result as SqliteResult, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// SQLite-backed memory store
pub struct MemoryDb {
    conn: Connection,
    base_dir: PathBuf,
}

impl MemoryDb {
    /// Initialize or open the memory database
    pub fn open() -> SqliteResult<Self> {
        let base_dir = Self::get_base_dir();
        let db_path = base_dir.join("ganesha_memory.db");

        let conn = Connection::open(&db_path)?;

        let mut db = Self { conn, base_dir };
        db.init_schema()?;

        Ok(db)
    }

    /// Get the base directory for memory storage
    fn get_base_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let base = home.join(".ganesha").join("memory");
        std::fs::create_dir_all(&base).ok();
        base
    }

    /// Initialize database schema
    fn init_schema(&mut self) -> SqliteResult<()> {
        self.conn.execute_batch(r#"
            -- Sessions table
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                started_at TEXT NOT NULL,
                ended_at TEXT NOT NULL,
                primary_task TEXT NOT NULL,
                outcome TEXT NOT NULL,
                rollback_available INTEGER DEFAULT 0,
                summary TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_task ON sessions(primary_task);

            -- Session files (many-to-one with sessions)
            CREATE TABLE IF NOT EXISTS session_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                operation TEXT NOT NULL, -- 'created', 'modified', 'deleted'
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            CREATE INDEX IF NOT EXISTS idx_session_files_session ON session_files(session_id);

            -- Session commands (many-to-one with sessions)
            CREATE TABLE IF NOT EXISTS session_commands (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                command TEXT NOT NULL,
                output TEXT,
                exit_code INTEGER,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            -- Session learnings
            CREATE TABLE IF NOT EXISTS session_learnings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                learning TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            -- Goals table
            CREATE TABLE IF NOT EXISTS goals (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                description TEXT NOT NULL,
                status TEXT NOT NULL,
                progress REAL DEFAULT 0.0,
                notes TEXT
            );

            -- Goal milestones
            CREATE TABLE IF NOT EXISTS milestones (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                goal_id TEXT NOT NULL,
                description TEXT NOT NULL,
                completed INTEGER DEFAULT 0,
                completed_at TEXT,
                FOREIGN KEY (goal_id) REFERENCES goals(id)
            );

            -- Learned patterns
            CREATE TABLE IF NOT EXISTS patterns (
                id TEXT PRIMARY KEY,
                learned_at TEXT NOT NULL,
                category TEXT NOT NULL,
                description TEXT NOT NULL,
                confidence INTEGER DEFAULT 1,
                context TEXT NOT NULL,
                action TEXT NOT NULL,
                anti_pattern TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_patterns_category ON patterns(category);
            CREATE INDEX IF NOT EXISTS idx_patterns_confidence ON patterns(confidence DESC);

            -- Knowledge base
            CREATE TABLE IF NOT EXISTS knowledge (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                topic TEXT NOT NULL,
                content TEXT NOT NULL,
                source TEXT,
                relevance REAL DEFAULT 1.0,
                last_accessed TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_knowledge_topic ON knowledge(topic);
            CREATE INDEX IF NOT EXISTS idx_knowledge_relevance ON knowledge(relevance DESC);

            -- Knowledge tags (many-to-many)
            CREATE TABLE IF NOT EXISTS knowledge_tags (
                knowledge_id TEXT NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (knowledge_id, tag),
                FOREIGN KEY (knowledge_id) REFERENCES knowledge(id)
            );
            CREATE INDEX IF NOT EXISTS idx_knowledge_tags_tag ON knowledge_tags(tag);

            -- User preferences (key-value store)
            CREATE TABLE IF NOT EXISTS preferences (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            -- Full-text search index for sessions
            CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
                primary_task,
                summary,
                content='sessions',
                content_rowid='rowid'
            );

            -- Trigger to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS sessions_ai AFTER INSERT ON sessions BEGIN
                INSERT INTO sessions_fts(rowid, primary_task, summary)
                VALUES (NEW.rowid, NEW.primary_task, NEW.summary);
            END;
        "#)?;

        Ok(())
    }

    // ========== Sessions ==========

    /// Add a new session
    pub fn add_session(&self, session: &SessionRecord) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, started_at, ended_at, primary_task, outcome, rollback_available, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session.id.to_string(),
                session.started_at.to_rfc3339(),
                session.ended_at.to_rfc3339(),
                session.primary_task,
                format!("{:?}", session.outcome),
                session.rollback_available as i32,
                session.summary
            ],
        )?;

        // Add files
        for (path, op) in &session.files_modified {
            self.conn.execute(
                "INSERT INTO session_files (session_id, file_path, operation)
                 VALUES (?1, ?2, ?3)",
                params![session.id.to_string(), path, op],
            )?;
        }

        // Add commands
        for cmd in &session.commands_executed {
            self.conn.execute(
                "INSERT INTO session_commands (session_id, command)
                 VALUES (?1, ?2)",
                params![session.id.to_string(), cmd],
            )?;
        }

        // Add learnings
        for learning in &session.key_learnings {
            self.conn.execute(
                "INSERT INTO session_learnings (session_id, learning)
                 VALUES (?1, ?2)",
                params![session.id.to_string(), learning],
            )?;
        }

        Ok(())
    }

    /// Get recent sessions
    pub fn recent_sessions(&self, limit: usize) -> SqliteResult<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, started_at, ended_at, primary_task, outcome, rollback_available, summary
             FROM sessions
             ORDER BY started_at DESC
             LIMIT ?1"
        )?;

        let mut sessions = Vec::new();
        let rows = stmt.query_map([limit], |row| {
            let id: String = row.get(0)?;
            let started: String = row.get(1)?;
            let ended: String = row.get(2)?;
            let task: String = row.get(3)?;
            let outcome_str: String = row.get(4)?;
            let rollback: i32 = row.get(5)?;
            let summary: Option<String> = row.get(6)?;

            Ok(SessionRecord {
                id: Uuid::parse_str(&id).unwrap_or_default(),
                started_at: DateTime::parse_from_rfc3339(&started)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                ended_at: DateTime::parse_from_rfc3339(&ended)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                primary_task: task,
                outcome: parse_outcome(&outcome_str),
                rollback_available: rollback != 0,
                summary,
                files_modified: vec![],
                commands_executed: vec![],
                key_learnings: vec![],
            })
        })?;

        for row in rows {
            if let Ok(session) = row {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    /// Search sessions by text
    pub fn search_sessions(&self, query: &str, limit: usize) -> SqliteResult<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.started_at, s.ended_at, s.primary_task, s.outcome, s.rollback_available, s.summary
             FROM sessions s
             JOIN sessions_fts fts ON s.rowid = fts.rowid
             WHERE sessions_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;

        let mut sessions = Vec::new();
        let rows = stmt.query_map([query, &limit.to_string()], |row| {
            let id: String = row.get(0)?;
            let started: String = row.get(1)?;
            let ended: String = row.get(2)?;
            let task: String = row.get(3)?;
            let outcome_str: String = row.get(4)?;
            let rollback: i32 = row.get(5)?;
            let summary: Option<String> = row.get(6)?;

            Ok(SessionRecord {
                id: Uuid::parse_str(&id).unwrap_or_default(),
                started_at: DateTime::parse_from_rfc3339(&started)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                ended_at: DateTime::parse_from_rfc3339(&ended)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                primary_task: task,
                outcome: parse_outcome(&outcome_str),
                rollback_available: rollback != 0,
                summary,
                files_modified: vec![],
                commands_executed: vec![],
                key_learnings: vec![],
            })
        })?;

        for row in rows {
            if let Ok(session) = row {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    /// Get session count
    pub fn session_count(&self) -> SqliteResult<usize> {
        self.conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| {
            row.get(0)
        })
    }

    // ========== Goals ==========

    /// Add or update a goal
    pub fn upsert_goal(&self, goal: &Goal) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO goals (id, created_at, description, status, progress, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                goal.id.to_string(),
                goal.created_at.to_rfc3339(),
                goal.description,
                format!("{:?}", goal.status),
                goal.progress,
                goal.notes.as_deref()
            ],
        )?;

        // Update milestones
        self.conn.execute(
            "DELETE FROM milestones WHERE goal_id = ?1",
            [goal.id.to_string()],
        )?;

        for milestone in &goal.milestones {
            self.conn.execute(
                "INSERT INTO milestones (goal_id, description, completed, completed_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    goal.id.to_string(),
                    milestone.description,
                    milestone.completed as i32,
                    milestone.completed_at.map(|d| d.to_rfc3339())
                ],
            )?;
        }

        Ok(())
    }

    /// Get active goals
    pub fn active_goals(&self) -> SqliteResult<Vec<Goal>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, description, status, progress, notes
             FROM goals WHERE status = 'Active'"
        )?;

        let mut goals = Vec::new();
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let created: String = row.get(1)?;
            let description: String = row.get(2)?;
            let status_str: String = row.get(3)?;
            let progress: f32 = row.get(4)?;
            let notes: Option<String> = row.get(5)?;

            Ok(Goal {
                id: Uuid::parse_str(&id).unwrap_or_default(),
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                description,
                status: parse_goal_status(&status_str),
                progress,
                milestones: vec![],
                notes,
            })
        })?;

        for row in rows {
            if let Ok(goal) = row {
                goals.push(goal);
            }
        }

        Ok(goals)
    }

    // ========== Patterns ==========

    /// Learn or reinforce a pattern
    pub fn learn_pattern(&self, pattern: &LearnedPattern) -> SqliteResult<()> {
        // Try to find existing pattern
        let existing: Option<i32> = self.conn.query_row(
            "SELECT confidence FROM patterns WHERE category = ?1 AND context = ?2",
            params![format!("{:?}", pattern.category), pattern.context],
            |row| row.get(0),
        ).ok();

        if let Some(confidence) = existing {
            // Reinforce existing pattern
            self.conn.execute(
                "UPDATE patterns SET confidence = ?1, action = ?2 WHERE category = ?3 AND context = ?4",
                params![confidence + 1, pattern.action, format!("{:?}", pattern.category), pattern.context],
            )?;
        } else {
            // Insert new pattern
            self.conn.execute(
                "INSERT INTO patterns (id, learned_at, category, description, confidence, context, action, anti_pattern)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    pattern.id.to_string(),
                    pattern.learned_at.to_rfc3339(),
                    format!("{:?}", pattern.category),
                    pattern.description,
                    pattern.confidence,
                    pattern.context,
                    pattern.action,
                    pattern.anti_pattern.as_deref()
                ],
            )?;
        }

        Ok(())
    }

    /// Get top patterns by confidence
    pub fn top_patterns(&self, limit: usize) -> SqliteResult<Vec<LearnedPattern>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, learned_at, category, description, confidence, context, action, anti_pattern
             FROM patterns
             ORDER BY confidence DESC
             LIMIT ?1"
        )?;

        let mut patterns = Vec::new();
        let rows = stmt.query_map([limit], |row| {
            let id: String = row.get(0)?;
            let learned: String = row.get(1)?;
            let category_str: String = row.get(2)?;
            let description: String = row.get(3)?;
            let confidence: u32 = row.get(4)?;
            let context: String = row.get(5)?;
            let action: String = row.get(6)?;
            let anti_pattern: Option<String> = row.get(7)?;

            Ok(LearnedPattern {
                id: Uuid::parse_str(&id).unwrap_or_default(),
                learned_at: DateTime::parse_from_rfc3339(&learned)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                category: parse_pattern_category(&category_str),
                description,
                confidence,
                context,
                action,
                anti_pattern,
            })
        })?;

        for row in rows {
            if let Ok(pattern) = row {
                patterns.push(pattern);
            }
        }

        Ok(patterns)
    }

    // ========== Knowledge ==========

    /// Add knowledge entry
    pub fn add_knowledge(&self, entry: &KnowledgeEntry) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO knowledge (id, created_at, topic, content, source, relevance, last_accessed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.id.to_string(),
                entry.created_at.to_rfc3339(),
                entry.topic,
                entry.content,
                entry.source.as_deref(),
                entry.relevance,
                entry.last_accessed.to_rfc3339()
            ],
        )?;

        // Add tags
        self.conn.execute(
            "DELETE FROM knowledge_tags WHERE knowledge_id = ?1",
            [entry.id.to_string()],
        )?;

        for tag in &entry.tags {
            self.conn.execute(
                "INSERT INTO knowledge_tags (knowledge_id, tag) VALUES (?1, ?2)",
                params![entry.id.to_string(), tag],
            )?;
        }

        Ok(())
    }

    /// Query knowledge by tags
    pub fn query_knowledge(&self, tags: &[&str]) -> SqliteResult<Vec<KnowledgeEntry>> {
        if tags.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<_> = tags.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT DISTINCT k.id, k.created_at, k.topic, k.content, k.source, k.relevance, k.last_accessed
             FROM knowledge k
             JOIN knowledge_tags kt ON k.id = kt.knowledge_id
             WHERE kt.tag IN ({})
             ORDER BY k.relevance DESC",
            placeholders.join(", ")
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = tags.iter().map(|t| t as &dyn rusqlite::ToSql).collect();

        let mut entries = Vec::new();
        let rows = stmt.query_map(params.as_slice(), |row| {
            let id: String = row.get(0)?;
            let created: String = row.get(1)?;
            let topic: String = row.get(2)?;
            let content: String = row.get(3)?;
            let source: Option<String> = row.get(4)?;
            let relevance: f32 = row.get(5)?;
            let last_accessed: String = row.get(6)?;

            Ok(KnowledgeEntry {
                id: Uuid::parse_str(&id).unwrap_or_default(),
                created_at: DateTime::parse_from_rfc3339(&created)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                topic,
                content,
                source,
                relevance,
                last_accessed: DateTime::parse_from_rfc3339(&last_accessed)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                tags: vec![],
            })
        })?;

        for row in rows {
            if let Ok(entry) = row {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    // ========== Preferences ==========

    /// Set a preference
    pub fn set_preference(&self, key: &str, value: &str) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get a preference
    pub fn get_preference(&self, key: &str) -> SqliteResult<Option<String>> {
        match self.conn.query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            [key],
            |row| row.get(0),
        ) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ========== Stats ==========

    /// Get memory statistics
    pub fn stats(&self) -> SqliteResult<MemoryStats> {
        let sessions = self.session_count()?;
        let goals: usize = self.conn.query_row("SELECT COUNT(*) FROM goals", [], |r| r.get(0))?;
        let patterns: usize = self.conn.query_row("SELECT COUNT(*) FROM patterns", [], |r| r.get(0))?;
        let knowledge: usize = self.conn.query_row("SELECT COUNT(*) FROM knowledge", [], |r| r.get(0))?;

        // Database file size
        let db_path = self.base_dir.join("ganesha_memory.db");
        let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

        Ok(MemoryStats {
            total_sessions: sessions,
            total_goals: goals,
            total_patterns: patterns,
            total_knowledge: knowledge,
            database_size_bytes: db_size,
        })
    }
}

// ========== Data Structures ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub primary_task: String,
    pub outcome: SessionOutcome,
    pub files_modified: Vec<(String, String)>, // (path, operation)
    pub commands_executed: Vec<String>,
    pub rollback_available: bool,
    pub key_learnings: Vec<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionOutcome {
    Success,
    PartialSuccess,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub description: String,
    pub status: GoalStatus,
    pub progress: f32,
    pub milestones: Vec<Milestone>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalStatus {
    Active,
    OnHold,
    Completed,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub description: String,
    pub completed: bool,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub id: Uuid,
    pub learned_at: DateTime<Utc>,
    pub category: PatternCategory,
    pub description: String,
    pub confidence: u32,
    pub context: String,
    pub action: String,
    pub anti_pattern: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternCategory {
    Coding,
    SystemAdmin,
    UserPreference,
    ToolUsage,
    ErrorRecovery,
    ProjectStructure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub topic: String,
    pub content: String,
    pub source: Option<String>,
    pub relevance: f32,
    pub last_accessed: DateTime<Utc>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_sessions: usize,
    pub total_goals: usize,
    pub total_patterns: usize,
    pub total_knowledge: usize,
    pub database_size_bytes: u64,
}

// ========== Helper Functions ==========

fn parse_outcome(s: &str) -> SessionOutcome {
    match s {
        "Success" => SessionOutcome::Success,
        "PartialSuccess" => SessionOutcome::PartialSuccess,
        "Failed" => SessionOutcome::Failed,
        _ => SessionOutcome::Aborted,
    }
}

fn parse_goal_status(s: &str) -> GoalStatus {
    match s {
        "Active" => GoalStatus::Active,
        "OnHold" => GoalStatus::OnHold,
        "Completed" => GoalStatus::Completed,
        _ => GoalStatus::Abandoned,
    }
}

fn parse_pattern_category(s: &str) -> PatternCategory {
    match s {
        "Coding" => PatternCategory::Coding,
        "SystemAdmin" => PatternCategory::SystemAdmin,
        "UserPreference" => PatternCategory::UserPreference,
        "ToolUsage" => PatternCategory::ToolUsage,
        "ErrorRecovery" => PatternCategory::ErrorRecovery,
        _ => PatternCategory::ProjectStructure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_db_init() {
        let db = MemoryDb::open().unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.total_sessions, 0);
    }

    #[test]
    fn test_add_session() {
        let db = MemoryDb::open().unwrap();
        let session = SessionRecord {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            ended_at: Utc::now(),
            primary_task: "Test task".into(),
            outcome: SessionOutcome::Success,
            files_modified: vec![("test.txt".into(), "created".into())],
            commands_executed: vec!["ls -la".into()],
            rollback_available: false,
            key_learnings: vec!["Learned something".into()],
            summary: Some("Test session".into()),
        };

        db.add_session(&session).unwrap();
        let count = db.session_count().unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_preferences() {
        let db = MemoryDb::open().unwrap();
        db.set_preference("test_key", "test_value").unwrap();
        let value = db.get_preference("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }
}
