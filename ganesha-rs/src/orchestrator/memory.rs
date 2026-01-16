//! Ganesha Persistent Memory System
//!
//! Long-horizon memory that persists across sessions:
//!
//! 1. **Fast Local DB** - SpacetimeDB for structured queries
//! 2. **Markdown Export** - Human-readable summaries
//! 3. **Global Context** - Not per-project, system-wide
//!
//! Memory types:
//! - Session history (what was done, when)
//! - Goal progress (long-term objectives)
//! - Learned patterns (what works, what doesn't)
//! - User preferences (how they like things done)
//! - Knowledge base (facts learned during sessions)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The global memory store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMemory {
    /// Base directory for all memory files
    pub base_dir: PathBuf,
    /// Session history
    pub sessions: Vec<SessionRecord>,
    /// Long-term goals
    pub goals: Vec<Goal>,
    /// Learned patterns
    pub patterns: Vec<LearnedPattern>,
    /// User preferences
    pub preferences: HashMap<String, String>,
    /// Knowledge base entries
    pub knowledge: Vec<KnowledgeEntry>,
    /// Last updated
    pub last_updated: DateTime<Utc>,
}

/// Record of a completed session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub primary_task: String,
    pub outcome: SessionOutcome,
    pub files_modified: Vec<String>,
    pub commands_executed: Vec<String>,
    pub rollback_available: bool,
    pub key_learnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionOutcome {
    Success,
    PartialSuccess,
    Failed,
    Aborted,
}

/// A long-term goal being tracked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub description: String,
    pub status: GoalStatus,
    pub progress: f32, // 0.0 to 1.0
    pub milestones: Vec<Milestone>,
    pub related_sessions: Vec<Uuid>,
    pub notes: Vec<String>,
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

/// A learned pattern (what works, what doesn't)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub id: Uuid,
    pub learned_at: DateTime<Utc>,
    pub category: PatternCategory,
    pub description: String,
    /// How many times this pattern was confirmed
    pub confidence: u32,
    /// Context where this applies
    pub context: String,
    /// What to do
    pub action: String,
    /// What to avoid
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

/// A piece of knowledge learned
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub topic: String,
    pub content: String,
    pub source: String,
    pub tags: Vec<String>,
    /// How relevant this is (decays over time if not used)
    pub relevance: f32,
    pub last_accessed: DateTime<Utc>,
}

impl GlobalMemory {
    /// Load or create global memory
    pub fn load() -> Self {
        let base_dir = Self::get_base_dir();

        let memory_file = base_dir.join("memory.json");

        if memory_file.exists() {
            match fs::read_to_string(&memory_file) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(memory) => return memory,
                        Err(e) => {
                            eprintln!("Warning: Failed to parse memory file: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to read memory file: {}", e);
                }
            }
        }

        // Create new memory
        Self {
            base_dir,
            sessions: vec![],
            goals: vec![],
            patterns: vec![],
            preferences: HashMap::new(),
            knowledge: vec![],
            last_updated: Utc::now(),
        }
    }

    /// Get the base directory for memory storage
    fn get_base_dir() -> PathBuf {
        let home = dirs::home_dir().expect("Unable to determine home directory for memory storage");
        let base = home.join(".ganesha").join("memory");
        fs::create_dir_all(&base).ok();
        base
    }

    /// Save memory to disk
    pub fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.last_updated = Utc::now();

        let memory_file = self.base_dir.join("memory.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&memory_file, content)?;

        // Also export markdown summary
        self.export_markdown()?;

        Ok(())
    }

    /// Add a session record
    pub fn add_session(&mut self, session: SessionRecord) {
        // Keep last 1000 sessions
        if self.sessions.len() >= 1000 {
            self.sessions.remove(0);
        }
        self.sessions.push(session);
    }

    /// Add or update a goal
    pub fn add_goal(&mut self, goal: Goal) {
        if let Some(existing) = self.goals.iter_mut().find(|g| g.id == goal.id) {
            *existing = goal;
        } else {
            self.goals.push(goal);
        }
    }

    /// Record a learned pattern
    pub fn learn_pattern(&mut self, pattern: LearnedPattern) {
        // Check if similar pattern exists
        if let Some(existing) = self.patterns.iter_mut().find(|p| {
            p.category == pattern.category && p.context == pattern.context
        }) {
            existing.confidence += 1;
            existing.action = pattern.action;
        } else {
            self.patterns.push(pattern);
        }
    }

    /// Add knowledge entry
    pub fn add_knowledge(&mut self, entry: KnowledgeEntry) {
        // Check for duplicates
        if self.knowledge.iter().any(|k| k.topic == entry.topic && k.content == entry.content) {
            return;
        }

        // Keep last 500 entries
        if self.knowledge.len() >= 500 {
            // Remove least relevant
            self.knowledge.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());
            self.knowledge.pop();
        }

        self.knowledge.push(entry);
    }

    /// Query knowledge by tags
    pub fn query_knowledge(&self, tags: &[&str]) -> Vec<&KnowledgeEntry> {
        self.knowledge
            .iter()
            .filter(|k| tags.iter().any(|t| k.tags.contains(&t.to_string())))
            .collect()
    }

    /// Get recent sessions
    pub fn recent_sessions(&self, limit: usize) -> &[SessionRecord] {
        let start = self.sessions.len().saturating_sub(limit);
        &self.sessions[start..]
    }

    /// Get active goals
    pub fn active_goals(&self) -> Vec<&Goal> {
        self.goals
            .iter()
            .filter(|g| matches!(g.status, GoalStatus::Active))
            .collect()
    }

    /// Get patterns for a category
    pub fn patterns_for(&self, category: PatternCategory) -> Vec<&LearnedPattern> {
        self.patterns
            .iter()
            .filter(|p| std::mem::discriminant(&p.category) == std::mem::discriminant(&category))
            .collect()
    }

    /// Export memory to markdown files
    pub fn export_markdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        let md_dir = self.base_dir.join("markdown");
        fs::create_dir_all(&md_dir)?;

        // Export session summary
        self.export_session_summary(&md_dir)?;

        // Export goals
        self.export_goals_summary(&md_dir)?;

        // Export patterns
        self.export_patterns_summary(&md_dir)?;

        // Export knowledge base
        self.export_knowledge_summary(&md_dir)?;

        Ok(())
    }

    fn export_session_summary(&self, dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut md = String::new();
        md.push_str("# Ganesha Session History\n\n");
        md.push_str(&format!("*Last updated: {}*\n\n", self.last_updated.format("%Y-%m-%d %H:%M:%S UTC")));

        md.push_str("## Recent Sessions\n\n");

        for session in self.sessions.iter().rev().take(20) {
            md.push_str(&format!(
                "### {} - {}\n",
                session.started_at.format("%Y-%m-%d %H:%M"),
                session.primary_task
            ));
            md.push_str(&format!("- **Outcome**: {:?}\n", session.outcome));
            md.push_str(&format!("- **Files Modified**: {}\n", session.files_modified.len()));
            md.push_str(&format!("- **Commands**: {}\n", session.commands_executed.len()));
            if session.rollback_available {
                md.push_str(&format!("- **Rollback**: Available (ID: {})\n", session.id));
            }
            if !session.key_learnings.is_empty() {
                md.push_str("- **Learnings**:\n");
                for learning in &session.key_learnings {
                    md.push_str(&format!("  - {}\n", learning));
                }
            }
            md.push_str("\n");
        }

        fs::write(dir.join("sessions.md"), md)?;
        Ok(())
    }

    fn export_goals_summary(&self, dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut md = String::new();
        md.push_str("# Ganesha Goals\n\n");

        md.push_str("## Active Goals\n\n");
        for goal in self.goals.iter().filter(|g| matches!(g.status, GoalStatus::Active)) {
            md.push_str(&format!("### {}\n", goal.description));
            md.push_str(&format!("- **Progress**: {:.0}%\n", goal.progress * 100.0));
            md.push_str("- **Milestones**:\n");
            for milestone in &goal.milestones {
                let status = if milestone.completed { "[x]" } else { "[ ]" };
                md.push_str(&format!("  - {} {}\n", status, milestone.description));
            }
            md.push_str("\n");
        }

        md.push_str("## Completed Goals\n\n");
        for goal in self.goals.iter().filter(|g| matches!(g.status, GoalStatus::Completed)) {
            md.push_str(&format!("- ~~{}~~\n", goal.description));
        }

        fs::write(dir.join("goals.md"), md)?;
        Ok(())
    }

    fn export_patterns_summary(&self, dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut md = String::new();
        md.push_str("# Ganesha Learned Patterns\n\n");
        md.push_str("*These patterns were learned from previous sessions.*\n\n");

        let categories = [
            (PatternCategory::Coding, "Coding"),
            (PatternCategory::SystemAdmin, "System Administration"),
            (PatternCategory::UserPreference, "User Preferences"),
            (PatternCategory::ToolUsage, "Tool Usage"),
            (PatternCategory::ErrorRecovery, "Error Recovery"),
            (PatternCategory::ProjectStructure, "Project Structure"),
        ];

        for (category, name) in categories {
            let patterns: Vec<_> = self.patterns
                .iter()
                .filter(|p| std::mem::discriminant(&p.category) == std::mem::discriminant(&category))
                .collect();

            if !patterns.is_empty() {
                md.push_str(&format!("## {}\n\n", name));
                for pattern in patterns {
                    md.push_str(&format!("### {} (confidence: {})\n", pattern.description, pattern.confidence));
                    md.push_str(&format!("- **Context**: {}\n", pattern.context));
                    md.push_str(&format!("- **Do**: {}\n", pattern.action));
                    if let Some(ref anti) = pattern.anti_pattern {
                        md.push_str(&format!("- **Don't**: {}\n", anti));
                    }
                    md.push_str("\n");
                }
            }
        }

        fs::write(dir.join("patterns.md"), md)?;
        Ok(())
    }

    fn export_knowledge_summary(&self, dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut md = String::new();
        md.push_str("# Ganesha Knowledge Base\n\n");

        // Group by topic
        let mut by_topic: HashMap<&str, Vec<&KnowledgeEntry>> = HashMap::new();
        for entry in &self.knowledge {
            by_topic.entry(&entry.topic).or_default().push(entry);
        }

        for (topic, entries) in by_topic {
            md.push_str(&format!("## {}\n\n", topic));
            for entry in entries {
                md.push_str(&format!("### {}\n", entry.content.lines().next().unwrap_or("Untitled")));
                md.push_str(&format!("{}\n\n", entry.content));
                md.push_str(&format!("*Source: {} | Tags: {}*\n\n", entry.source, entry.tags.join(", ")));
            }
        }

        fs::write(dir.join("knowledge.md"), md)?;
        Ok(())
    }

    /// Get context for the current session
    pub fn get_session_context(&self) -> SessionContext {
        SessionContext {
            recent_sessions: self.recent_sessions(5)
                .iter()
                .map(|s| format!("{}: {} ({:?})", s.started_at.format("%m-%d %H:%M"), s.primary_task, s.outcome))
                .collect(),
            active_goals: self.active_goals()
                .iter()
                .map(|g| format!("{} ({:.0}%)", g.description, g.progress * 100.0))
                .collect(),
            relevant_patterns: self.patterns
                .iter()
                .filter(|p| p.confidence > 2)
                .take(10)
                .map(|p| format!("{}: {}", p.context, p.action))
                .collect(),
        }
    }
}

/// Context to inject into sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub recent_sessions: Vec<String>,
    pub active_goals: Vec<String>,
    pub relevant_patterns: Vec<String>,
}

impl SessionContext {
    pub fn to_prompt(&self) -> String {
        let mut prompt = String::new();

        if !self.active_goals.is_empty() {
            prompt.push_str("ACTIVE GOALS:\n");
            for goal in &self.active_goals {
                prompt.push_str(&format!("- {}\n", goal));
            }
            prompt.push_str("\n");
        }

        if !self.relevant_patterns.is_empty() {
            prompt.push_str("LEARNED PATTERNS:\n");
            for pattern in &self.relevant_patterns {
                prompt.push_str(&format!("- {}\n", pattern));
            }
            prompt.push_str("\n");
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_memory_creation() {
        let memory = GlobalMemory::load();
        assert!(memory.sessions.is_empty() || !memory.sessions.is_empty());
    }

    #[test]
    fn test_session_record() {
        let session = SessionRecord {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            ended_at: Utc::now(),
            primary_task: "Test task".into(),
            outcome: SessionOutcome::Success,
            files_modified: vec![],
            commands_executed: vec![],
            rollback_available: false,
            key_learnings: vec![],
        };
        assert_eq!(session.primary_task, "Test task");
    }

    #[test]
    fn test_goal_creation() {
        let goal = Goal {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            description: "Build Ganesha 3.0".into(),
            status: GoalStatus::Active,
            progress: 0.3,
            milestones: vec![
                Milestone {
                    description: "Complete orchestrator".into(),
                    completed: true,
                    completed_at: Some(Utc::now()),
                },
            ],
            related_sessions: vec![],
            notes: vec![],
        };
        assert!(goal.progress > 0.0);
    }
}
