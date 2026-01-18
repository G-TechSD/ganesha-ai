//! # Memory System for Ganesha 4.0
//!
//! A comprehensive memory system that provides:
//! - Conversation memory with token tracking
//! - Knowledge graph for entity/relationship storage
//! - File context memory for tracking code structure
//! - Session persistence with SQLite backend
//! - Semantic search preparation for future embeddings
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        MemorySystem                              │
//! │                                                                  │
//! │  ┌──────────────────┐  ┌──────────────────┐                     │
//! │  │ ConversationMem  │  │  KnowledgeGraph  │                     │
//! │  │  - Messages      │  │  - Entities      │                     │
//! │  │  - Summaries     │  │  - Relationships │                     │
//! │  │  - Token counts  │  │  - SQLite store  │                     │
//! │  └──────────────────┘  └──────────────────┘                     │
//! │                                                                  │
//! │  ┌──────────────────┐  ┌──────────────────┐                     │
//! │  │ FileContextMem   │  │ SessionManager   │                     │
//! │  │  - File tracking │  │  - Persistence   │                     │
//! │  │  - Code structs  │  │  - Auto-save     │                     │
//! │  │  - AST cache     │  │  - Recovery      │                     │
//! │  └──────────────────┘  └──────────────────┘                     │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

/// Memory system errors
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Relationship not found: {0}")]
    RelationshipNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Conversation not found: {0}")]
    ConversationNotFound(String),

    #[error("Context window exceeded: {current} > {max}")]
    ContextWindowExceeded { current: usize, max: usize },

    #[error("Lock poisoned")]
    LockPoisoned,
}

pub type Result<T> = std::result::Result<T, MemoryError>;

// ============================================================================
// Conversation Memory
// ============================================================================

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "system" => Ok(Role::System),
            "user" | "human" => Ok(Role::User),
            "assistant" | "ai" | "bot" => Ok(Role::Assistant),
            "tool" | "function" => Ok(Role::Tool),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// A single message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: Uuid,
    /// Role of the sender
    pub role: Role,
    /// Message content
    pub content: String,
    /// When the message was created
    pub timestamp: DateTime<Utc>,
    /// Model that generated this message (if assistant)
    pub model: Option<String>,
    /// Estimated token count
    pub tokens: usize,
    /// Tool call ID (if this is a tool response)
    pub tool_call_id: Option<String>,
    /// Tool name (if this is a tool call or response)
    pub tool_name: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// Create a new message
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let content = content.into();
        let tokens = Self::estimate_tokens(&content);
        Self {
            id: Uuid::new_v4(),
            role,
            content,
            timestamp: Utc::now(),
            model: None,
            tokens,
            tool_call_id: None,
            tool_name: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// Create a tool response message
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        let mut msg = Self::new(Role::Tool, content);
        msg.tool_call_id = Some(tool_call_id.into());
        msg
    }

    /// Set the model that generated this message
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Add metadata to the message
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Estimate token count for content
    /// Uses a simple heuristic: ~4 characters per token for English text
    pub fn estimate_tokens(content: &str) -> usize {
        // More accurate estimation considering:
        // - Whitespace and punctuation often are separate tokens
        // - Code has different tokenization patterns
        let word_count = content.split_whitespace().count();
        let char_count = content.len();

        // Blend of character-based and word-based estimation
        let char_estimate = char_count / 4;
        let word_estimate = (word_count as f64 * 1.3) as usize;

        // Use the larger estimate for safety
        char_estimate.max(word_estimate).max(1)
    }

    /// Recalculate token count
    pub fn recalculate_tokens(&mut self) {
        self.tokens = Self::estimate_tokens(&self.content);
    }
}

/// Summary of a conversation segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    /// Summary ID
    pub id: Uuid,
    /// The summarized text
    pub summary: String,
    /// Messages that were summarized (by ID)
    pub message_ids: Vec<Uuid>,
    /// Token count of the summary
    pub tokens: usize,
    /// When the summary was created
    pub created_at: DateTime<Utc>,
    /// Original token count of summarized messages
    pub original_tokens: usize,
}

impl ConversationSummary {
    /// Create a new summary
    pub fn new(summary: impl Into<String>, message_ids: Vec<Uuid>, original_tokens: usize) -> Self {
        let summary = summary.into();
        let tokens = Message::estimate_tokens(&summary);
        Self {
            id: Uuid::new_v4(),
            summary,
            message_ids,
            tokens,
            created_at: Utc::now(),
            original_tokens,
        }
    }

    /// Compression ratio (original / summary tokens)
    pub fn compression_ratio(&self) -> f64 {
        if self.tokens == 0 {
            return 0.0;
        }
        self.original_tokens as f64 / self.tokens as f64
    }
}

/// Metadata about a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    /// Conversation title (auto-generated or user-provided)
    pub title: Option<String>,
    /// When the conversation started
    pub started_at: DateTime<Utc>,
    /// When the conversation was last updated
    pub updated_at: DateTime<Utc>,
    /// Total messages in the conversation
    pub message_count: usize,
    /// Total tokens in the conversation
    pub total_tokens: usize,
    /// Project/workspace this conversation is associated with
    pub project_path: Option<PathBuf>,
    /// Tags for organization
    pub tags: Vec<String>,
}

impl Default for ConversationMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            title: None,
            started_at: now,
            updated_at: now,
            message_count: 0,
            total_tokens: 0,
            project_path: None,
            tags: Vec::new(),
        }
    }
}

/// A conversation with messages and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique conversation ID
    pub id: Uuid,
    /// Conversation metadata
    pub metadata: ConversationMetadata,
    /// Messages in chronological order
    pub messages: Vec<Message>,
    /// Summaries of older conversation segments
    pub summaries: Vec<ConversationSummary>,
    /// System prompt (kept separate for easy updates)
    pub system_prompt: Option<String>,
}

impl Conversation {
    /// Create a new conversation
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            metadata: ConversationMetadata::default(),
            messages: Vec::new(),
            summaries: Vec::new(),
            system_prompt: None,
        }
    }

    /// Create a conversation with a specific ID
    pub fn with_id(id: Uuid) -> Self {
        let mut conv = Self::new();
        conv.id = id;
        conv
    }

    /// Set the system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set the project path
    pub fn with_project(mut self, path: impl Into<PathBuf>) -> Self {
        self.metadata.project_path = Some(path.into());
        self
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, message: Message) {
        self.metadata.total_tokens += message.tokens;
        self.metadata.message_count += 1;
        self.metadata.updated_at = Utc::now();
        self.messages.push(message);
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: impl Into<String>) -> &Message {
        self.add_message(Message::user(content));
        self.messages.last().unwrap()
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, content: impl Into<String>, model: Option<String>) -> &Message {
        let mut msg = Message::assistant(content);
        msg.model = model;
        self.add_message(msg);
        self.messages.last().unwrap()
    }

    /// Get total token count (messages + summaries)
    pub fn total_tokens(&self) -> usize {
        let message_tokens: usize = self.messages.iter().map(|m| m.tokens).sum();
        let summary_tokens: usize = self.summaries.iter().map(|s| s.tokens).sum();
        let system_tokens = self.system_prompt
            .as_ref()
            .map(|s| Message::estimate_tokens(s))
            .unwrap_or(0);

        message_tokens + summary_tokens + system_tokens
    }

    /// Get messages within a token budget
    pub fn messages_within_budget(&self, max_tokens: usize) -> Vec<&Message> {
        let mut tokens = 0;
        let mut result = Vec::new();

        // Include system prompt in budget
        if let Some(ref prompt) = self.system_prompt {
            tokens += Message::estimate_tokens(prompt);
        }

        // Add messages from newest to oldest, then reverse
        for message in self.messages.iter().rev() {
            if tokens + message.tokens <= max_tokens {
                tokens += message.tokens;
                result.push(message);
            } else {
                break;
            }
        }

        result.reverse();
        result
    }

    /// Summarize old messages to reduce context size
    /// Returns a callback that should be called with the summary text
    pub fn prepare_summarization(&self, keep_recent: usize) -> Option<SummarizationRequest> {
        if self.messages.len() <= keep_recent {
            return None;
        }

        let to_summarize = &self.messages[..self.messages.len() - keep_recent];
        let message_ids: Vec<Uuid> = to_summarize.iter().map(|m| m.id).collect();
        let original_tokens: usize = to_summarize.iter().map(|m| m.tokens).sum();

        // Build content to summarize
        let content: String = to_summarize
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        Some(SummarizationRequest {
            conversation_id: self.id,
            content,
            message_ids,
            original_tokens,
            keep_recent,
        })
    }

    /// Apply a summarization result
    pub fn apply_summarization(&mut self, summary_text: String, request: SummarizationRequest) {
        let summary = ConversationSummary::new(
            summary_text,
            request.message_ids.clone(),
            request.original_tokens,
        );

        // Remove summarized messages
        self.messages.retain(|m| !request.message_ids.contains(&m.id));

        // Add the summary
        self.summaries.push(summary);

        // Update metadata
        self.metadata.total_tokens = self.total_tokens();
    }

    /// Get recent messages (for display or processing)
    pub fn recent_messages(&self, count: usize) -> &[Message] {
        let start = self.messages.len().saturating_sub(count);
        &self.messages[start..]
    }

    /// Search messages by content
    pub fn search_messages(&self, query: &str) -> Vec<&Message> {
        let query_lower = query.to_lowercase();
        self.messages
            .iter()
            .filter(|m| m.content.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Export conversation to markdown
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        // Header
        md.push_str(&format!("# {}\n\n",
            self.metadata.title.as_deref().unwrap_or("Conversation")));

        md.push_str(&format!("**Started:** {}\n", self.metadata.started_at));
        md.push_str(&format!("**Messages:** {}\n", self.metadata.message_count));
        if let Some(ref path) = self.metadata.project_path {
            md.push_str(&format!("**Project:** {}\n", path.display()));
        }
        md.push_str("\n---\n\n");

        // System prompt
        if let Some(ref prompt) = self.system_prompt {
            md.push_str("## System Prompt\n\n");
            md.push_str(prompt);
            md.push_str("\n\n---\n\n");
        }

        // Summaries (if any)
        if !self.summaries.is_empty() {
            md.push_str("## Previous Context (Summarized)\n\n");
            for summary in &self.summaries {
                md.push_str(&format!("*Summarized {} messages ({} -> {} tokens)*\n\n",
                    summary.message_ids.len(),
                    summary.original_tokens,
                    summary.tokens));
                md.push_str(&summary.summary);
                md.push_str("\n\n");
            }
            md.push_str("---\n\n");
        }

        // Messages
        md.push_str("## Conversation\n\n");
        for message in &self.messages {
            let role_header = match message.role {
                Role::System => "**System**",
                Role::User => "**User**",
                Role::Assistant => "**Assistant**",
                Role::Tool => "**Tool**",
            };

            md.push_str(&format!("### {} ({:?})\n\n", role_header, message.timestamp));

            if let Some(ref model) = message.model {
                md.push_str(&format!("*Model: {}*\n\n", model));
            }

            md.push_str(&message.content);
            md.push_str("\n\n");
        }

        md
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to summarize part of a conversation
#[derive(Debug, Clone)]
pub struct SummarizationRequest {
    pub conversation_id: Uuid,
    pub content: String,
    pub message_ids: Vec<Uuid>,
    pub original_tokens: usize,
    pub keep_recent: usize,
}

// ============================================================================
// Knowledge Graph
// ============================================================================

/// Type of entity in the knowledge graph
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    /// A file in the project
    File,
    /// A function/method
    Function,
    /// A class/struct/type
    Type,
    /// A module/package
    Module,
    /// A variable/constant
    Variable,
    /// A concept or topic
    Concept,
    /// A person (author, maintainer)
    Person,
    /// A task or issue
    Task,
    /// Custom entity type
    Custom(String),
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::File => write!(f, "file"),
            EntityType::Function => write!(f, "function"),
            EntityType::Type => write!(f, "type"),
            EntityType::Module => write!(f, "module"),
            EntityType::Variable => write!(f, "variable"),
            EntityType::Concept => write!(f, "concept"),
            EntityType::Person => write!(f, "person"),
            EntityType::Task => write!(f, "task"),
            EntityType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// An entity in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique entity ID
    pub id: Uuid,
    /// Entity name
    pub name: String,
    /// Entity type
    pub entity_type: EntityType,
    /// Entity properties (key-value pairs)
    pub properties: HashMap<String, serde_json::Value>,
    /// When the entity was created
    pub created_at: DateTime<Utc>,
    /// When the entity was last updated
    pub updated_at: DateTime<Utc>,
}

impl Entity {
    /// Create a new entity
    pub fn new(name: impl Into<String>, entity_type: EntityType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            entity_type,
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create an entity with a specific ID
    pub fn with_id(id: Uuid, name: impl Into<String>, entity_type: EntityType) -> Self {
        let mut entity = Self::new(name, entity_type);
        entity.id = id;
        entity
    }

    /// Add a property
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.properties.insert(key.into(), value.into());
        self.updated_at = Utc::now();
        self
    }

    /// Set a property
    pub fn set_property(&mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) {
        self.properties.insert(key.into(), value.into());
        self.updated_at = Utc::now();
    }

    /// Get a property
    pub fn get_property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }

    /// Get a property as a specific type
    pub fn get_property_as<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.properties.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

/// Type of relationship between entities
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// File contains entity
    Contains,
    /// Entity imports/uses another
    Imports,
    /// Entity calls another
    Calls,
    /// Entity inherits from another
    Inherits,
    /// Entity implements another
    Implements,
    /// Entity depends on another
    DependsOn,
    /// Entity is related to another
    RelatedTo,
    /// Entity references another
    References,
    /// Entity is defined in another
    DefinedIn,
    /// Custom relationship type
    Custom(String),
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelationType::Contains => write!(f, "contains"),
            RelationType::Imports => write!(f, "imports"),
            RelationType::Calls => write!(f, "calls"),
            RelationType::Inherits => write!(f, "inherits"),
            RelationType::Implements => write!(f, "implements"),
            RelationType::DependsOn => write!(f, "depends_on"),
            RelationType::RelatedTo => write!(f, "related_to"),
            RelationType::References => write!(f, "references"),
            RelationType::DefinedIn => write!(f, "defined_in"),
            RelationType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// A relationship between two entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// Unique relationship ID
    pub id: Uuid,
    /// Source entity ID
    pub from_id: Uuid,
    /// Target entity ID
    pub to_id: Uuid,
    /// Relationship type
    pub relation_type: RelationType,
    /// Relationship properties
    pub properties: HashMap<String, serde_json::Value>,
    /// When the relationship was created
    pub created_at: DateTime<Utc>,
}

impl Relationship {
    /// Create a new relationship
    pub fn new(from_id: Uuid, to_id: Uuid, relation_type: RelationType) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_id,
            to_id,
            relation_type,
            properties: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a property
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

/// Query for finding entities in the knowledge graph
#[derive(Debug, Clone, Default)]
pub struct EntityQuery {
    /// Filter by entity type
    pub entity_type: Option<EntityType>,
    /// Filter by name pattern (substring match)
    pub name_pattern: Option<String>,
    /// Filter by property existence
    pub has_property: Option<String>,
    /// Filter by property value
    pub property_value: Option<(String, serde_json::Value)>,
    /// Maximum results
    pub limit: Option<usize>,
}

impl EntityQuery {
    /// Create a new query
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by entity type
    pub fn with_type(mut self, entity_type: EntityType) -> Self {
        self.entity_type = Some(entity_type);
        self
    }

    /// Filter by name pattern
    pub fn with_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.name_pattern = Some(pattern.into());
        self
    }

    /// Filter by property existence
    pub fn with_property(mut self, property: impl Into<String>) -> Self {
        self.has_property = Some(property.into());
        self
    }

    /// Limit results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Query for finding relationships
#[derive(Debug, Clone, Default)]
pub struct RelationshipQuery {
    /// Filter by source entity
    pub from_id: Option<Uuid>,
    /// Filter by target entity
    pub to_id: Option<Uuid>,
    /// Filter by relationship type
    pub relation_type: Option<RelationType>,
    /// Maximum results
    pub limit: Option<usize>,
}

impl RelationshipQuery {
    /// Create a new query
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by source entity
    pub fn from_entity(mut self, id: Uuid) -> Self {
        self.from_id = Some(id);
        self
    }

    /// Filter by target entity
    pub fn to_entity(mut self, id: Uuid) -> Self {
        self.to_id = Some(id);
        self
    }

    /// Filter by relationship type
    pub fn with_type(mut self, relation_type: RelationType) -> Self {
        self.relation_type = Some(relation_type);
        self
    }

    /// Limit results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Knowledge graph for storing entities and relationships
#[derive(Debug)]
pub struct KnowledgeGraph {
    /// Database connection
    conn: Connection,
    /// In-memory entity cache
    entity_cache: HashMap<Uuid, Entity>,
    /// In-memory relationship cache
    relationship_cache: HashMap<Uuid, Relationship>,
}

impl KnowledgeGraph {
    /// Create a new knowledge graph with SQLite storage
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let mut kg = Self {
            conn,
            entity_cache: HashMap::new(),
            relationship_cache: HashMap::new(),
        };
        kg.init_schema()?;
        Ok(kg)
    }

    /// Create an in-memory knowledge graph
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let mut kg = Self {
            conn,
            entity_cache: HashMap::new(),
            relationship_cache: HashMap::new(),
        };
        kg.init_schema()?;
        Ok(kg)
    }

    /// Initialize database schema
    fn init_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS entities (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                properties TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);
            CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);

            CREATE TABLE IF NOT EXISTS relationships (
                id TEXT PRIMARY KEY,
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                properties TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (from_id) REFERENCES entities(id),
                FOREIGN KEY (to_id) REFERENCES entities(id)
            );

            CREATE INDEX IF NOT EXISTS idx_relationships_from ON relationships(from_id);
            CREATE INDEX IF NOT EXISTS idx_relationships_to ON relationships(to_id);
            CREATE INDEX IF NOT EXISTS idx_relationships_type ON relationships(relation_type);
            "#,
        )?;
        Ok(())
    }

    /// Add an entity to the graph
    pub fn add_entity(&mut self, entity: Entity) -> Result<Uuid> {
        let id = entity.id;
        let entity_type_str = entity.entity_type.to_string();
        let properties_json = serde_json::to_string(&entity.properties)?;
        let created_at = entity.created_at.to_rfc3339();
        let updated_at = entity.updated_at.to_rfc3339();

        self.conn.execute(
            "INSERT OR REPLACE INTO entities (id, name, entity_type, properties, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.to_string(),
                entity.name,
                entity_type_str,
                properties_json,
                created_at,
                updated_at,
            ],
        )?;

        self.entity_cache.insert(id, entity);
        Ok(id)
    }

    /// Add a relationship to the graph
    pub fn add_relationship(&mut self, relationship: Relationship) -> Result<Uuid> {
        // Verify entities exist
        if self.get_entity(relationship.from_id)?.is_none() {
            return Err(MemoryError::EntityNotFound(relationship.from_id.to_string()));
        }
        if self.get_entity(relationship.to_id)?.is_none() {
            return Err(MemoryError::EntityNotFound(relationship.to_id.to_string()));
        }

        let id = relationship.id;
        let relation_type_str = relationship.relation_type.to_string();
        let properties_json = serde_json::to_string(&relationship.properties)?;
        let created_at = relationship.created_at.to_rfc3339();

        self.conn.execute(
            "INSERT OR REPLACE INTO relationships (id, from_id, to_id, relation_type, properties, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.to_string(),
                relationship.from_id.to_string(),
                relationship.to_id.to_string(),
                relation_type_str,
                properties_json,
                created_at,
            ],
        )?;

        self.relationship_cache.insert(id, relationship);
        Ok(id)
    }

    /// Get an entity by ID
    pub fn get_entity(&self, id: Uuid) -> Result<Option<Entity>> {
        // Check cache first
        if let Some(entity) = self.entity_cache.get(&id) {
            return Ok(Some(entity.clone()));
        }

        // Query database
        let mut stmt = self.conn.prepare(
            "SELECT id, name, entity_type, properties, created_at, updated_at FROM entities WHERE id = ?1"
        )?;

        let entity = stmt.query_row(params![id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let name: String = row.get(1)?;
            let entity_type_str: String = row.get(2)?;
            let properties_json: String = row.get(3)?;
            let created_at_str: String = row.get(4)?;
            let updated_at_str: String = row.get(5)?;

            Ok((id_str, name, entity_type_str, properties_json, created_at_str, updated_at_str))
        }).optional()?;

        match entity {
            Some((id_str, name, entity_type_str, properties_json, created_at_str, updated_at_str)) => {
                let entity_type = Self::parse_entity_type(&entity_type_str);
                let properties: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&properties_json).unwrap_or_default();
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(Entity {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    name,
                    entity_type,
                    properties,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Get a relationship by ID
    pub fn get_relationship(&self, id: Uuid) -> Result<Option<Relationship>> {
        // Check cache first
        if let Some(rel) = self.relationship_cache.get(&id) {
            return Ok(Some(rel.clone()));
        }

        // Query database
        let mut stmt = self.conn.prepare(
            "SELECT id, from_id, to_id, relation_type, properties, created_at FROM relationships WHERE id = ?1"
        )?;

        let rel = stmt.query_row(params![id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let from_id_str: String = row.get(1)?;
            let to_id_str: String = row.get(2)?;
            let relation_type_str: String = row.get(3)?;
            let properties_json: String = row.get(4)?;
            let created_at_str: String = row.get(5)?;

            Ok((id_str, from_id_str, to_id_str, relation_type_str, properties_json, created_at_str))
        }).optional()?;

        match rel {
            Some((id_str, from_id_str, to_id_str, relation_type_str, properties_json, created_at_str)) => {
                let relation_type = Self::parse_relation_type(&relation_type_str);
                let properties: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&properties_json).unwrap_or_default();
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(Relationship {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    from_id: Uuid::parse_str(&from_id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    to_id: Uuid::parse_str(&to_id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    relation_type,
                    properties,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Query entities
    pub fn query_entities(&self, query: &EntityQuery) -> Result<Vec<Entity>> {
        let mut sql = String::from(
            "SELECT id, name, entity_type, properties, created_at, updated_at FROM entities WHERE 1=1"
        );
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(ref entity_type) = query.entity_type {
            sql.push_str(" AND entity_type = ?");
            params_vec.push(entity_type.to_string());
        }

        if let Some(ref name_pattern) = query.name_pattern {
            sql.push_str(" AND name LIKE ?");
            params_vec.push(format!("%{}%", name_pattern));
        }

        if let Some(ref has_property) = query.has_property {
            sql.push_str(" AND properties LIKE ?");
            params_vec.push(format!("%\"{}\":%", has_property));
        }

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let entities = stmt.query_map(params_refs.as_slice(), |row| {
            let id_str: String = row.get(0)?;
            let name: String = row.get(1)?;
            let entity_type_str: String = row.get(2)?;
            let properties_json: String = row.get(3)?;
            let created_at_str: String = row.get(4)?;
            let updated_at_str: String = row.get(5)?;

            Ok((id_str, name, entity_type_str, properties_json, created_at_str, updated_at_str))
        })?;

        let mut result = Vec::new();
        for entity_result in entities {
            let (id_str, name, entity_type_str, properties_json, created_at_str, updated_at_str) = entity_result?;
            let entity_type = Self::parse_entity_type(&entity_type_str);
            let properties: HashMap<String, serde_json::Value> =
                serde_json::from_str(&properties_json).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            result.push(Entity {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                name,
                entity_type,
                properties,
                created_at,
                updated_at,
            });
        }

        Ok(result)
    }

    /// Query relationships
    pub fn query_relationships(&self, query: &RelationshipQuery) -> Result<Vec<Relationship>> {
        let mut sql = String::from(
            "SELECT id, from_id, to_id, relation_type, properties, created_at FROM relationships WHERE 1=1"
        );
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(from_id) = query.from_id {
            sql.push_str(" AND from_id = ?");
            params_vec.push(from_id.to_string());
        }

        if let Some(to_id) = query.to_id {
            sql.push_str(" AND to_id = ?");
            params_vec.push(to_id.to_string());
        }

        if let Some(ref relation_type) = query.relation_type {
            sql.push_str(" AND relation_type = ?");
            params_vec.push(relation_type.to_string());
        }

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let relationships = stmt.query_map(params_refs.as_slice(), |row| {
            let id_str: String = row.get(0)?;
            let from_id_str: String = row.get(1)?;
            let to_id_str: String = row.get(2)?;
            let relation_type_str: String = row.get(3)?;
            let properties_json: String = row.get(4)?;
            let created_at_str: String = row.get(5)?;

            Ok((id_str, from_id_str, to_id_str, relation_type_str, properties_json, created_at_str))
        })?;

        let mut result = Vec::new();
        for rel_result in relationships {
            let (id_str, from_id_str, to_id_str, relation_type_str, properties_json, created_at_str) = rel_result?;
            let relation_type = Self::parse_relation_type(&relation_type_str);
            let properties: HashMap<String, serde_json::Value> =
                serde_json::from_str(&properties_json).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            result.push(Relationship {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                from_id: Uuid::parse_str(&from_id_str).unwrap_or_else(|_| Uuid::new_v4()),
                to_id: Uuid::parse_str(&to_id_str).unwrap_or_else(|_| Uuid::new_v4()),
                relation_type,
                properties,
                created_at,
            });
        }

        Ok(result)
    }

    /// Find all entities connected to a given entity
    pub fn find_connected(&self, entity_id: Uuid) -> Result<Vec<(Entity, Relationship)>> {
        let mut result = Vec::new();

        // Find outgoing relationships
        let outgoing = self.query_relationships(
            &RelationshipQuery::new().from_entity(entity_id)
        )?;
        for rel in outgoing {
            if let Some(entity) = self.get_entity(rel.to_id)? {
                result.push((entity, rel));
            }
        }

        // Find incoming relationships
        let incoming = self.query_relationships(
            &RelationshipQuery::new().to_entity(entity_id)
        )?;
        for rel in incoming {
            if let Some(entity) = self.get_entity(rel.from_id)? {
                result.push((entity, rel));
            }
        }

        Ok(result)
    }

    /// Delete an entity and its relationships
    pub fn delete_entity(&mut self, id: Uuid) -> Result<bool> {
        let id_str = id.to_string();

        // Delete relationships first
        self.conn.execute(
            "DELETE FROM relationships WHERE from_id = ?1 OR to_id = ?1",
            params![&id_str],
        )?;

        // Delete entity
        let rows = self.conn.execute(
            "DELETE FROM entities WHERE id = ?1",
            params![&id_str],
        )?;

        // Remove from cache
        self.entity_cache.remove(&id);
        self.relationship_cache.retain(|_, r| r.from_id != id && r.to_id != id);

        Ok(rows > 0)
    }

    /// Delete a relationship
    pub fn delete_relationship(&mut self, id: Uuid) -> Result<bool> {
        let rows = self.conn.execute(
            "DELETE FROM relationships WHERE id = ?1",
            params![id.to_string()],
        )?;

        self.relationship_cache.remove(&id);
        Ok(rows > 0)
    }

    /// Clear all data from the knowledge graph
    pub fn clear(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM relationships; DELETE FROM entities;"
        )?;
        self.entity_cache.clear();
        self.relationship_cache.clear();
        Ok(())
    }

    /// Get statistics about the knowledge graph
    pub fn stats(&self) -> Result<KnowledgeGraphStats> {
        let entity_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM entities",
            [],
            |row| row.get(0),
        )?;

        let relationship_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM relationships",
            [],
            |row| row.get(0),
        )?;

        Ok(KnowledgeGraphStats {
            entity_count: entity_count as usize,
            relationship_count: relationship_count as usize,
            cached_entities: self.entity_cache.len(),
            cached_relationships: self.relationship_cache.len(),
        })
    }

    /// Parse entity type from string
    fn parse_entity_type(s: &str) -> EntityType {
        match s {
            "file" => EntityType::File,
            "function" => EntityType::Function,
            "type" => EntityType::Type,
            "module" => EntityType::Module,
            "variable" => EntityType::Variable,
            "concept" => EntityType::Concept,
            "person" => EntityType::Person,
            "task" => EntityType::Task,
            other => EntityType::Custom(other.to_string()),
        }
    }

    /// Parse relation type from string
    fn parse_relation_type(s: &str) -> RelationType {
        match s {
            "contains" => RelationType::Contains,
            "imports" => RelationType::Imports,
            "calls" => RelationType::Calls,
            "inherits" => RelationType::Inherits,
            "implements" => RelationType::Implements,
            "depends_on" => RelationType::DependsOn,
            "related_to" => RelationType::RelatedTo,
            "references" => RelationType::References,
            "defined_in" => RelationType::DefinedIn,
            other => RelationType::Custom(other.to_string()),
        }
    }
}

/// Statistics about the knowledge graph
#[derive(Debug, Clone)]
pub struct KnowledgeGraphStats {
    pub entity_count: usize,
    pub relationship_count: usize,
    pub cached_entities: usize,
    pub cached_relationships: usize,
}

// ============================================================================
// File Context Memory
// ============================================================================

/// Status of file interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// File has been read
    Read,
    /// File has been modified
    Modified,
    /// File has been created
    Created,
    /// File has been deleted
    Deleted,
}

/// Information about a code structure element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeElement {
    /// Element name
    pub name: String,
    /// Element kind (function, class, struct, etc.)
    pub kind: String,
    /// Line number where element starts
    pub start_line: usize,
    /// Line number where element ends
    pub end_line: usize,
    /// Documentation/comments
    pub documentation: Option<String>,
    /// Signature (for functions/methods)
    pub signature: Option<String>,
    /// Visibility (public, private, etc.)
    pub visibility: Option<String>,
}

/// Context information about a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    /// File path
    pub path: PathBuf,
    /// File status
    pub status: FileStatus,
    /// When the file was first accessed
    pub first_accessed: DateTime<Utc>,
    /// When the file was last accessed
    pub last_accessed: DateTime<Utc>,
    /// Number of times accessed
    pub access_count: usize,
    /// File summary (if generated)
    pub summary: Option<String>,
    /// Hash of file content when last read
    pub content_hash: Option<String>,
    /// Programming language
    pub language: Option<String>,
    /// Line count
    pub line_count: Option<usize>,
    /// Code elements (functions, classes, etc.)
    pub elements: Vec<CodeElement>,
    /// Import statements
    pub imports: Vec<String>,
    /// Key information extracted from the file
    pub key_info: HashMap<String, String>,
}

impl FileContext {
    /// Create a new file context
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let now = Utc::now();
        Self {
            path: path.into(),
            status: FileStatus::Read,
            first_accessed: now,
            last_accessed: now,
            access_count: 1,
            summary: None,
            content_hash: None,
            language: None,
            line_count: None,
            elements: Vec::new(),
            imports: Vec::new(),
            key_info: HashMap::new(),
        }
    }

    /// Create a file context with content analysis
    pub fn with_content(path: impl Into<PathBuf>, content: &str) -> Self {
        let path = path.into();
        let mut ctx = Self::new(&path);

        // Calculate content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        ctx.content_hash = Some(format!("{:x}", hasher.finalize()));

        // Detect language from extension
        ctx.language = path
            .extension()
            .and_then(|e| e.to_str())
            .map(Self::extension_to_language)
            .flatten()
            .map(String::from);

        // Count lines
        ctx.line_count = Some(content.lines().count());

        ctx
    }

    /// Mark file as modified
    pub fn mark_modified(&mut self) {
        self.status = FileStatus::Modified;
        self.last_accessed = Utc::now();
        self.access_count += 1;
    }

    /// Update access timestamp
    pub fn record_access(&mut self) {
        self.last_accessed = Utc::now();
        self.access_count += 1;
    }

    /// Add a code element
    pub fn add_element(&mut self, element: CodeElement) {
        self.elements.push(element);
    }

    /// Add an import
    pub fn add_import(&mut self, import: impl Into<String>) {
        self.imports.push(import.into());
    }

    /// Set key information
    pub fn set_key_info(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.key_info.insert(key.into(), value.into());
    }

    /// Map file extension to language
    fn extension_to_language(ext: &str) -> Option<&'static str> {
        match ext.to_lowercase().as_str() {
            "rs" => Some("rust"),
            "py" => Some("python"),
            "js" => Some("javascript"),
            "ts" => Some("typescript"),
            "jsx" => Some("javascript"),
            "tsx" => Some("typescript"),
            "go" => Some("go"),
            "java" => Some("java"),
            "c" => Some("c"),
            "cpp" | "cc" | "cxx" => Some("cpp"),
            "h" | "hpp" => Some("cpp"),
            "rb" => Some("ruby"),
            "php" => Some("php"),
            "swift" => Some("swift"),
            "kt" | "kts" => Some("kotlin"),
            "cs" => Some("csharp"),
            "scala" => Some("scala"),
            "ex" | "exs" => Some("elixir"),
            "hs" => Some("haskell"),
            "ml" | "mli" => Some("ocaml"),
            "lua" => Some("lua"),
            "sh" | "bash" => Some("shell"),
            "sql" => Some("sql"),
            "html" | "htm" => Some("html"),
            "css" => Some("css"),
            "scss" | "sass" => Some("scss"),
            "json" => Some("json"),
            "yaml" | "yml" => Some("yaml"),
            "toml" => Some("toml"),
            "xml" => Some("xml"),
            "md" | "markdown" => Some("markdown"),
            _ => None,
        }
    }
}

/// Memory for tracking file context during a session
#[derive(Debug)]
pub struct FileContextMemory {
    /// Map of file path to context
    files: HashMap<PathBuf, FileContext>,
    /// Maximum number of files to track
    max_files: usize,
}

impl FileContextMemory {
    /// Create a new file context memory
    pub fn new(max_files: usize) -> Self {
        Self {
            files: HashMap::new(),
            max_files,
        }
    }

    /// Record that a file was read
    pub fn record_read(&mut self, path: impl Into<PathBuf>, content: Option<&str>) {
        let path = path.into();

        if let Some(ctx) = self.files.get_mut(&path) {
            ctx.record_access();
            if let Some(content) = content {
                ctx.line_count = Some(content.lines().count());
                let mut hasher = Sha256::new();
                hasher.update(content.as_bytes());
                ctx.content_hash = Some(format!("{:x}", hasher.finalize()));
            }
        } else {
            let ctx = if let Some(content) = content {
                FileContext::with_content(&path, content)
            } else {
                FileContext::new(&path)
            };
            self.add_context(ctx);
        }
    }

    /// Record that a file was modified
    pub fn record_modification(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();

        if let Some(ctx) = self.files.get_mut(&path) {
            ctx.mark_modified();
        } else {
            let mut ctx = FileContext::new(&path);
            ctx.status = FileStatus::Modified;
            self.add_context(ctx);
        }
    }

    /// Record that a file was created
    pub fn record_creation(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        let mut ctx = FileContext::new(&path);
        ctx.status = FileStatus::Created;
        self.add_context(ctx);
    }

    /// Record that a file was deleted
    pub fn record_deletion(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();

        if let Some(ctx) = self.files.get_mut(&path) {
            ctx.status = FileStatus::Deleted;
            ctx.record_access();
        } else {
            let mut ctx = FileContext::new(&path);
            ctx.status = FileStatus::Deleted;
            self.add_context(ctx);
        }
    }

    /// Add a file context
    fn add_context(&mut self, ctx: FileContext) {
        // Evict least recently accessed files if at capacity
        if self.files.len() >= self.max_files {
            self.evict_oldest();
        }
        self.files.insert(ctx.path.clone(), ctx);
    }

    /// Evict the oldest (least recently accessed) file
    fn evict_oldest(&mut self) {
        if let Some((oldest_path, _)) = self.files
            .iter()
            .min_by_key(|(_, ctx)| ctx.last_accessed)
            .map(|(p, c)| (p.clone(), c.clone()))
        {
            self.files.remove(&oldest_path);
        }
    }

    /// Get context for a specific file
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&FileContext> {
        self.files.get(path.as_ref())
    }

    /// Get mutable context for a specific file
    pub fn get_mut(&mut self, path: impl AsRef<Path>) -> Option<&mut FileContext> {
        self.files.get_mut(path.as_ref())
    }

    /// Get all tracked files
    pub fn all_files(&self) -> impl Iterator<Item = &FileContext> {
        self.files.values()
    }

    /// Get all modified files
    pub fn modified_files(&self) -> impl Iterator<Item = &FileContext> {
        self.files.values()
            .filter(|ctx| matches!(ctx.status, FileStatus::Modified | FileStatus::Created))
    }

    /// Get recently accessed files
    pub fn recent_files(&self, count: usize) -> Vec<&FileContext> {
        let mut files: Vec<_> = self.files.values().collect();
        files.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
        files.into_iter().take(count).collect()
    }

    /// Search files by path pattern
    pub fn search_by_path(&self, pattern: &str) -> Vec<&FileContext> {
        let pattern_lower = pattern.to_lowercase();
        self.files.values()
            .filter(|ctx| {
                ctx.path.to_string_lossy().to_lowercase().contains(&pattern_lower)
            })
            .collect()
    }

    /// Search files by language
    pub fn files_by_language(&self, language: &str) -> Vec<&FileContext> {
        self.files.values()
            .filter(|ctx| {
                ctx.language.as_ref().map(|l| l == language).unwrap_or(false)
            })
            .collect()
    }

    /// Clear all file context
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Get file count
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl Default for FileContextMemory {
    fn default() -> Self {
        Self::new(1000) // Default to tracking 1000 files
    }
}

// ============================================================================
// Session Persistence
// ============================================================================

/// Session state that can be persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session ID
    pub id: Uuid,
    /// Session name
    pub name: Option<String>,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last updated
    pub updated_at: DateTime<Utc>,
    /// Active conversation
    pub conversation: Conversation,
    /// Working directory
    pub working_directory: Option<PathBuf>,
    /// Project path
    pub project_path: Option<PathBuf>,
    /// File contexts (serializable subset)
    pub file_contexts: Vec<FileContext>,
    /// Custom session data
    pub custom_data: HashMap<String, serde_json::Value>,
}

impl SessionState {
    /// Create a new session state
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: None,
            created_at: now,
            updated_at: now,
            conversation: Conversation::new(),
            working_directory: None,
            project_path: None,
            file_contexts: Vec::new(),
            custom_data: HashMap::new(),
        }
    }

    /// Create a session with a specific ID
    pub fn with_id(id: Uuid) -> Self {
        let mut session = Self::new();
        session.id = id;
        session
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for session persistence
#[derive(Debug)]
pub struct SessionManager {
    /// Base directory for session storage
    storage_dir: PathBuf,
    /// SQLite connection for metadata
    conn: Connection,
    /// Auto-save enabled
    auto_save: bool,
    /// Current session
    current_session: Option<SessionState>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(storage_dir: impl Into<PathBuf>) -> Result<Self> {
        let storage_dir = storage_dir.into();
        std::fs::create_dir_all(&storage_dir)?;

        let db_path = storage_dir.join("sessions.db");
        let conn = Connection::open(&db_path)?;

        let mut manager = Self {
            storage_dir,
            conn,
            auto_save: true,
            current_session: None,
        };

        manager.init_schema()?;
        Ok(manager)
    }

    /// Initialize database schema
    fn init_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                project_path TEXT,
                message_count INTEGER NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_path);
            "#,
        )?;
        Ok(())
    }

    /// Create a new session
    pub fn new_session(&mut self) -> Result<&SessionState> {
        let session = SessionState::new();
        self.save_session(&session)?;
        self.current_session = Some(session);
        Ok(self.current_session.as_ref().unwrap())
    }

    /// Load a session by ID
    pub fn load_session(&mut self, id: Uuid) -> Result<&SessionState> {
        let session_path = self.session_file_path(id);

        if !session_path.exists() {
            return Err(MemoryError::SessionNotFound(id.to_string()));
        }

        let content = std::fs::read_to_string(&session_path)?;
        let session: SessionState = serde_json::from_str(&content)?;

        self.current_session = Some(session);
        Ok(self.current_session.as_ref().unwrap())
    }

    /// Save the current session
    pub fn save_current(&mut self) -> Result<()> {
        if let Some(ref mut session) = self.current_session {
            session.updated_at = Utc::now();
            // Clone the session to avoid borrow conflict
            let session_clone = session.clone();
            self.save_session(&session_clone)?;
        }
        Ok(())
    }

    /// Save a session to disk
    pub fn save_session(&self, session: &SessionState) -> Result<()> {
        // Save to JSON file
        let session_path = self.session_file_path(session.id);
        let content = serde_json::to_string_pretty(session)?;
        std::fs::write(&session_path, content)?;

        // Update metadata in SQLite
        self.conn.execute(
            "INSERT OR REPLACE INTO sessions (id, name, created_at, updated_at, project_path, message_count, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session.id.to_string(),
                session.name,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
                session.project_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                session.conversation.messages.len() as i64,
                self.current_session.as_ref().map(|s| s.id == session.id).unwrap_or(false) as i64,
            ],
        )?;

        Ok(())
    }

    /// Get current session
    pub fn current(&self) -> Option<&SessionState> {
        self.current_session.as_ref()
    }

    /// Get current session mutably
    pub fn current_mut(&mut self) -> Option<&mut SessionState> {
        self.current_session.as_mut()
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Result<Vec<SessionMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, created_at, updated_at, project_path, message_count
             FROM sessions ORDER BY updated_at DESC"
        )?;

        let sessions = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let name: Option<String> = row.get(1)?;
            let created_at_str: String = row.get(2)?;
            let updated_at_str: String = row.get(3)?;
            let project_path: Option<String> = row.get(4)?;
            let message_count: i64 = row.get(5)?;

            Ok((id_str, name, created_at_str, updated_at_str, project_path, message_count))
        })?;

        let mut result = Vec::new();
        for session_result in sessions {
            let (id_str, name, created_at_str, updated_at_str, project_path, message_count) = session_result?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            result.push(SessionMetadata {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                name,
                created_at,
                updated_at,
                project_path: project_path.map(PathBuf::from),
                message_count: message_count as usize,
            });
        }

        Ok(result)
    }

    /// List sessions for a specific project
    pub fn sessions_for_project(&self, project_path: impl AsRef<Path>) -> Result<Vec<SessionMetadata>> {
        let project_str = project_path.as_ref().to_string_lossy().to_string();

        let mut stmt = self.conn.prepare(
            "SELECT id, name, created_at, updated_at, project_path, message_count
             FROM sessions WHERE project_path = ?1 ORDER BY updated_at DESC"
        )?;

        let sessions = stmt.query_map(params![project_str], |row| {
            let id_str: String = row.get(0)?;
            let name: Option<String> = row.get(1)?;
            let created_at_str: String = row.get(2)?;
            let updated_at_str: String = row.get(3)?;
            let project_path: Option<String> = row.get(4)?;
            let message_count: i64 = row.get(5)?;

            Ok((id_str, name, created_at_str, updated_at_str, project_path, message_count))
        })?;

        let mut result = Vec::new();
        for session_result in sessions {
            let (id_str, name, created_at_str, updated_at_str, project_path, message_count) = session_result?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            result.push(SessionMetadata {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                name,
                created_at,
                updated_at,
                project_path: project_path.map(PathBuf::from),
                message_count: message_count as usize,
            });
        }

        Ok(result)
    }

    /// Delete a session
    pub fn delete_session(&mut self, id: Uuid) -> Result<bool> {
        let session_path = self.session_file_path(id);

        // Remove file
        if session_path.exists() {
            std::fs::remove_file(&session_path)?;
        }

        // Remove from database
        let rows = self.conn.execute(
            "DELETE FROM sessions WHERE id = ?1",
            params![id.to_string()],
        )?;

        // Clear current if it was deleted
        if self.current_session.as_ref().map(|s| s.id) == Some(id) {
            self.current_session = None;
        }

        Ok(rows > 0)
    }

    /// Export current session to markdown
    pub fn export_to_markdown(&self) -> Result<String> {
        let session = self.current_session.as_ref()
            .ok_or_else(|| MemoryError::SessionNotFound("no current session".to_string()))?;

        let mut md = String::new();

        // Session header
        md.push_str(&format!("# Session: {}\n\n",
            session.name.as_deref().unwrap_or(&session.id.to_string())));
        md.push_str(&format!("**ID:** {}\n", session.id));
        md.push_str(&format!("**Created:** {}\n", session.created_at));
        md.push_str(&format!("**Updated:** {}\n", session.updated_at));

        if let Some(ref path) = session.project_path {
            md.push_str(&format!("**Project:** {}\n", path.display()));
        }

        md.push_str("\n---\n\n");

        // File contexts
        if !session.file_contexts.is_empty() {
            md.push_str("## Files Accessed\n\n");
            for ctx in &session.file_contexts {
                md.push_str(&format!("- `{}` ({:?})\n", ctx.path.display(), ctx.status));
            }
            md.push_str("\n---\n\n");
        }

        // Conversation
        md.push_str(&session.conversation.to_markdown());

        Ok(md)
    }

    /// Get the file path for a session
    fn session_file_path(&self, id: Uuid) -> PathBuf {
        self.storage_dir.join(format!("{}.json", id))
    }

    /// Check for and recover crashed sessions
    pub fn recover_crashed_sessions(&mut self) -> Result<Vec<SessionMetadata>> {
        // Find sessions marked as active but not properly closed
        let mut stmt = self.conn.prepare(
            "SELECT id FROM sessions WHERE is_active = 1"
        )?;

        let ids: Vec<String> = stmt.query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Mark them as inactive (crashed)
        for id in &ids {
            self.conn.execute(
                "UPDATE sessions SET is_active = 0 WHERE id = ?1",
                params![id],
            )?;
        }

        // Return the list of recovered sessions
        let mut recovered = Vec::new();
        for id_str in ids {
            if let Ok(id) = Uuid::parse_str(&id_str) {
                if let Some(meta) = self.list_sessions()?.into_iter().find(|m| m.id == id) {
                    recovered.push(meta);
                }
            }
        }

        Ok(recovered)
    }

    /// Enable/disable auto-save
    pub fn set_auto_save(&mut self, enabled: bool) {
        self.auto_save = enabled;
    }

    /// Check if auto-save is enabled
    pub fn auto_save_enabled(&self) -> bool {
        self.auto_save
    }
}

/// Metadata about a session (for listing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: Uuid,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub project_path: Option<PathBuf>,
    pub message_count: usize,
}

// ============================================================================
// Semantic Search (preparation for embeddings)
// ============================================================================

/// A searchable item with relevance scoring
#[derive(Debug, Clone)]
pub struct SearchResult<T> {
    /// The item found
    pub item: T,
    /// Relevance score (0.0 - 1.0)
    pub score: f64,
    /// Matched terms or snippets
    pub matches: Vec<String>,
}

impl<T> SearchResult<T> {
    /// Create a new search result
    pub fn new(item: T, score: f64) -> Self {
        Self {
            item,
            score,
            matches: Vec::new(),
        }
    }

    /// Add a match
    pub fn with_match(mut self, matched: impl Into<String>) -> Self {
        self.matches.push(matched.into());
        self
    }
}

/// Trait for searchable memory stores
pub trait SearchableMemory {
    /// Type of items in this memory
    type Item;

    /// Search by text query
    fn search(&self, query: &str, limit: usize) -> Vec<SearchResult<Self::Item>>;

    /// Search with filters
    fn search_filtered(
        &self,
        query: &str,
        filter: &dyn Fn(&Self::Item) -> bool,
        limit: usize,
    ) -> Vec<SearchResult<Self::Item>>;
}

/// Configuration for semantic search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchConfig {
    /// Whether embeddings are enabled
    pub embeddings_enabled: bool,
    /// Embedding model name
    pub embedding_model: Option<String>,
    /// Embedding dimension
    pub embedding_dimension: usize,
    /// Similarity threshold (0.0 - 1.0)
    pub similarity_threshold: f64,
    /// Maximum results
    pub max_results: usize,
}

impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            embeddings_enabled: false,
            embedding_model: None,
            embedding_dimension: 1536, // OpenAI default
            similarity_threshold: 0.7,
            max_results: 10,
        }
    }
}

/// Embedding vector (placeholder for future implementation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Vector values
    pub values: Vec<f32>,
    /// Model used to generate this embedding
    pub model: String,
    /// Text that was embedded
    pub source_text: String,
    /// Hash of source text
    pub source_hash: String,
}

impl Embedding {
    /// Create a new embedding
    pub fn new(values: Vec<f32>, model: impl Into<String>, source: impl Into<String>) -> Self {
        let source_text = source.into();
        let mut hasher = Sha256::new();
        hasher.update(source_text.as_bytes());
        let source_hash = format!("{:x}", hasher.finalize());

        Self {
            values,
            model: model.into(),
            source_text,
            source_hash,
        }
    }

    /// Calculate cosine similarity with another embedding
    pub fn cosine_similarity(&self, other: &Embedding) -> f64 {
        if self.values.len() != other.values.len() {
            return 0.0;
        }

        let dot_product: f64 = self.values.iter()
            .zip(other.values.iter())
            .map(|(a, b)| (*a as f64) * (*b as f64))
            .sum();

        let norm_a: f64 = self.values.iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            .sqrt();

        let norm_b: f64 = other.values.iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            .sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }
}

/// Text-based search implementation (before embeddings)
pub struct TextSearch {
    config: SemanticSearchConfig,
}

impl TextSearch {
    /// Create a new text search
    pub fn new() -> Self {
        Self {
            config: SemanticSearchConfig::default(),
        }
    }

    /// Calculate text similarity score
    pub fn calculate_score(query: &str, text: &str) -> f64 {
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();

        // Exact match
        if text_lower.contains(&query_lower) {
            return 1.0;
        }

        // Word-based matching
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let text_words: Vec<&str> = text_lower.split_whitespace().collect();

        if query_words.is_empty() || text_words.is_empty() {
            return 0.0;
        }

        let mut matched = 0;
        for qw in &query_words {
            if text_words.iter().any(|tw| tw.contains(qw) || qw.contains(tw)) {
                matched += 1;
            }
        }

        matched as f64 / query_words.len() as f64
    }

    /// Find matching terms
    pub fn find_matches(query: &str, text: &str) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut matches = Vec::new();
        for word in query_words {
            if text.to_lowercase().contains(word) {
                matches.push(word.to_string());
            }
        }

        matches
    }
}

impl Default for TextSearch {
    fn default() -> Self {
        Self::new()
    }
}

// Implement SearchableMemory for Conversation
impl SearchableMemory for Conversation {
    type Item = Message;

    fn search(&self, query: &str, limit: usize) -> Vec<SearchResult<Self::Item>> {
        let mut results: Vec<SearchResult<Message>> = self.messages
            .iter()
            .map(|msg| {
                let score = TextSearch::calculate_score(query, &msg.content);
                let matches = TextSearch::find_matches(query, &msg.content);
                SearchResult {
                    item: msg.clone(),
                    score,
                    matches,
                }
            })
            .filter(|r| r.score > 0.0)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    fn search_filtered(
        &self,
        query: &str,
        filter: &dyn Fn(&Self::Item) -> bool,
        limit: usize,
    ) -> Vec<SearchResult<Self::Item>> {
        let mut results: Vec<SearchResult<Message>> = self.messages
            .iter()
            .filter(|msg| filter(msg))
            .map(|msg| {
                let score = TextSearch::calculate_score(query, &msg.content);
                let matches = TextSearch::find_matches(query, &msg.content);
                SearchResult {
                    item: msg.clone(),
                    score,
                    matches,
                }
            })
            .filter(|r| r.score > 0.0)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }
}

// ============================================================================
// Unified Memory System
// ============================================================================

/// The unified memory system combining all memory types
pub struct MemorySystem {
    /// Conversation memory
    pub conversation: Arc<RwLock<Conversation>>,
    /// Knowledge graph
    pub knowledge_graph: Arc<RwLock<KnowledgeGraph>>,
    /// File context memory
    pub file_context: Arc<RwLock<FileContextMemory>>,
    /// Session manager
    pub session_manager: Arc<RwLock<SessionManager>>,
    /// Semantic search configuration
    pub search_config: SemanticSearchConfig,
}

impl MemorySystem {
    /// Create a new memory system
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref();
        std::fs::create_dir_all(data_dir)?;

        let kg_path = data_dir.join("knowledge_graph.db");
        let sessions_dir = data_dir.join("sessions");

        Ok(Self {
            conversation: Arc::new(RwLock::new(Conversation::new())),
            knowledge_graph: Arc::new(RwLock::new(KnowledgeGraph::new(kg_path)?)),
            file_context: Arc::new(RwLock::new(FileContextMemory::default())),
            session_manager: Arc::new(RwLock::new(SessionManager::new(sessions_dir)?)),
            search_config: SemanticSearchConfig::default(),
        })
    }

    /// Create an in-memory memory system (for testing)
    pub fn in_memory() -> Result<Self> {
        let temp_dir = std::env::temp_dir().join(format!("ganesha-memory-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            conversation: Arc::new(RwLock::new(Conversation::new())),
            knowledge_graph: Arc::new(RwLock::new(KnowledgeGraph::in_memory()?)),
            file_context: Arc::new(RwLock::new(FileContextMemory::default())),
            session_manager: Arc::new(RwLock::new(SessionManager::new(&temp_dir)?)),
            search_config: SemanticSearchConfig::default(),
        })
    }

    /// Add a user message to the current conversation
    pub fn add_user_message(&self, content: impl Into<String>) -> Result<Uuid> {
        let mut conv = self.conversation.write()
            .map_err(|_| MemoryError::LockPoisoned)?;
        let msg = conv.add_user_message(content);
        Ok(msg.id)
    }

    /// Add an assistant message to the current conversation
    pub fn add_assistant_message(&self, content: impl Into<String>, model: Option<String>) -> Result<Uuid> {
        let mut conv = self.conversation.write()
            .map_err(|_| MemoryError::LockPoisoned)?;
        let msg = conv.add_assistant_message(content, model);
        Ok(msg.id)
    }

    /// Get conversation within token budget
    pub fn get_context(&self, max_tokens: usize) -> Result<Vec<Message>> {
        let conv = self.conversation.read()
            .map_err(|_| MemoryError::LockPoisoned)?;
        Ok(conv.messages_within_budget(max_tokens).into_iter().cloned().collect())
    }

    /// Record file read
    pub fn record_file_read(&self, path: impl Into<PathBuf>, content: Option<&str>) -> Result<()> {
        let mut fc = self.file_context.write()
            .map_err(|_| MemoryError::LockPoisoned)?;
        fc.record_read(path, content);
        Ok(())
    }

    /// Record file modification
    pub fn record_file_modification(&self, path: impl Into<PathBuf>) -> Result<()> {
        let mut fc = self.file_context.write()
            .map_err(|_| MemoryError::LockPoisoned)?;
        fc.record_modification(path);
        Ok(())
    }

    /// Add an entity to the knowledge graph
    pub fn add_entity(&self, entity: Entity) -> Result<Uuid> {
        let mut kg = self.knowledge_graph.write()
            .map_err(|_| MemoryError::LockPoisoned)?;
        kg.add_entity(entity)
    }

    /// Add a relationship to the knowledge graph
    pub fn add_relationship(&self, relationship: Relationship) -> Result<Uuid> {
        let mut kg = self.knowledge_graph.write()
            .map_err(|_| MemoryError::LockPoisoned)?;
        kg.add_relationship(relationship)
    }

    /// Query entities from the knowledge graph
    pub fn query_entities(&self, query: &EntityQuery) -> Result<Vec<Entity>> {
        let kg = self.knowledge_graph.read()
            .map_err(|_| MemoryError::LockPoisoned)?;
        kg.query_entities(query)
    }

    /// Save current state to session
    pub fn save_session(&self) -> Result<()> {
        let conv = self.conversation.read()
            .map_err(|_| MemoryError::LockPoisoned)?;
        let fc = self.file_context.read()
            .map_err(|_| MemoryError::LockPoisoned)?;
        let mut sm = self.session_manager.write()
            .map_err(|_| MemoryError::LockPoisoned)?;

        if let Some(session) = sm.current_mut() {
            session.conversation = conv.clone();
            session.file_contexts = fc.all_files().cloned().collect();
            sm.save_current()?;
        }

        Ok(())
    }

    /// Load state from session
    pub fn load_session(&self, id: Uuid) -> Result<()> {
        let mut sm = self.session_manager.write()
            .map_err(|_| MemoryError::LockPoisoned)?;
        sm.load_session(id)?;

        if let Some(session) = sm.current() {
            // Restore conversation
            let mut conv = self.conversation.write()
                .map_err(|_| MemoryError::LockPoisoned)?;
            *conv = session.conversation.clone();

            // Restore file contexts
            let mut fc = self.file_context.write()
                .map_err(|_| MemoryError::LockPoisoned)?;
            fc.clear();
            for ctx in &session.file_contexts {
                fc.record_read(&ctx.path, None);
            }
        }

        Ok(())
    }

    /// Search across all memory types
    pub fn search(&self, query: &str, limit: usize) -> Result<UnifiedSearchResults> {
        let conv = self.conversation.read()
            .map_err(|_| MemoryError::LockPoisoned)?;
        let kg = self.knowledge_graph.read()
            .map_err(|_| MemoryError::LockPoisoned)?;
        let fc = self.file_context.read()
            .map_err(|_| MemoryError::LockPoisoned)?;

        // Search messages
        let message_results = conv.search(query, limit);

        // Search entities
        let entity_results = kg.query_entities(
            &EntityQuery::new().with_name_pattern(query).with_limit(limit)
        )?;

        // Search files
        let file_results = fc.search_by_path(query);

        Ok(UnifiedSearchResults {
            messages: message_results,
            entities: entity_results,
            files: file_results.into_iter().cloned().collect(),
        })
    }
}

/// Results from a unified search across all memory types
#[derive(Debug)]
pub struct UnifiedSearchResults {
    pub messages: Vec<SearchResult<Message>>,
    pub entities: Vec<Entity>,
    pub files: Vec<FileContext>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello, world!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello, world!");
        assert!(msg.tokens > 0);
    }

    #[test]
    fn test_message_token_estimation() {
        let short_text = "Hello";
        let long_text = "This is a longer piece of text that should have more tokens.";

        assert!(Message::estimate_tokens(long_text) > Message::estimate_tokens(short_text));
    }

    #[test]
    fn test_conversation_add_messages() {
        let mut conv = Conversation::new();
        conv.add_user_message("Hello");
        conv.add_assistant_message("Hi there!", Some("gpt-4".to_string()));

        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.metadata.message_count, 2);
    }

    #[test]
    fn test_conversation_search() {
        let mut conv = Conversation::new();
        conv.add_user_message("I need help with Rust programming");
        conv.add_assistant_message("Sure, I can help with Rust!", None);
        conv.add_user_message("How do I create a vector?");

        let results = conv.search_messages("Rust");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_knowledge_graph_in_memory() {
        let mut kg = KnowledgeGraph::in_memory().unwrap();

        let entity = Entity::new("main.rs", EntityType::File)
            .with_property("language", "rust");
        let id = kg.add_entity(entity).unwrap();

        let retrieved = kg.get_entity(id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "main.rs");
    }

    #[test]
    fn test_knowledge_graph_relationships() {
        let mut kg = KnowledgeGraph::in_memory().unwrap();

        let file = Entity::new("main.rs", EntityType::File);
        let func = Entity::new("main", EntityType::Function);

        let file_id = kg.add_entity(file).unwrap();
        let func_id = kg.add_entity(func).unwrap();

        let rel = Relationship::new(file_id, func_id, RelationType::Contains);
        kg.add_relationship(rel).unwrap();

        let connected = kg.find_connected(file_id).unwrap();
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].0.name, "main");
    }

    #[test]
    fn test_file_context_memory() {
        let mut fcm = FileContextMemory::new(100);

        fcm.record_read("/path/to/file.rs", Some("fn main() {}"));
        fcm.record_modification("/path/to/file.rs");

        let ctx = fcm.get(&PathBuf::from("/path/to/file.rs")).unwrap();
        assert_eq!(ctx.status, FileStatus::Modified);
        assert_eq!(ctx.access_count, 2);
    }

    #[test]
    fn test_text_search_scoring() {
        let score1 = TextSearch::calculate_score("rust programming", "I love Rust programming");
        let score2 = TextSearch::calculate_score("rust programming", "Python is great");

        assert!(score1 > score2);
    }

    #[test]
    fn test_embedding_cosine_similarity() {
        let emb1 = Embedding::new(vec![1.0, 0.0, 0.0], "test", "text1");
        let emb2 = Embedding::new(vec![1.0, 0.0, 0.0], "test", "text2");
        let emb3 = Embedding::new(vec![0.0, 1.0, 0.0], "test", "text3");

        assert!((emb1.cosine_similarity(&emb2) - 1.0).abs() < 0.001);
        assert!((emb1.cosine_similarity(&emb3) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_conversation_to_markdown() {
        let mut conv = Conversation::new();
        conv.metadata.title = Some("Test Conversation".to_string());
        conv.add_user_message("Hello");
        conv.add_assistant_message("Hi!", None);

        let md = conv.to_markdown();
        assert!(md.contains("Test Conversation"));
        assert!(md.contains("Hello"));
        assert!(md.contains("Hi!"));
    }

    #[test]
    fn test_entity_query() {
        let mut kg = KnowledgeGraph::in_memory().unwrap();

        kg.add_entity(Entity::new("file1.rs", EntityType::File)).unwrap();
        kg.add_entity(Entity::new("file2.rs", EntityType::File)).unwrap();
        kg.add_entity(Entity::new("main", EntityType::Function)).unwrap();

        let files = kg.query_entities(&EntityQuery::new().with_type(EntityType::File)).unwrap();
        assert_eq!(files.len(), 2);

        let named = kg.query_entities(&EntityQuery::new().with_name_pattern("file1")).unwrap();
        assert_eq!(named.len(), 1);
    }
}
