//! # Ganesha Core Engine
//!
//! The core engine for Ganesha 4.0 - an AI-powered coding assistant written in Rust.
//!
//! This crate provides the fundamental building blocks for the Ganesha assistant:
//!
//! - **Planning**: Task decomposition and step-by-step planning
//! - **Execution**: Execute planned steps with rollback support
//! - **Verification**: Verify execution results and check for issues
//! - **Consent**: User consent management for operations
//! - **Session**: Conversation session management with checkpointing
//! - **Configuration**: Multi-source configuration system
//! - **Risk**: Operation risk assessment
//! - **Rollback**: Undo/redo capabilities
//! - **Sandbox**: Isolated execution environments
//! - **Memory**: Conversation and knowledge memory
//! - **MiniMe**: Subagent management for parallel tasks
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      GaneshaEngine                          │
//! │                                                             │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
//! │  │ Planner  │  │ Executor │  │ Verifier │  │  Memory  │   │
//! │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
//! │                                                             │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
//! │  │ Rollback │  │ Sandbox  │  │  MiniMe  │  │ Consent  │   │
//! │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```ignore
//! use ganesha_core::prelude::*;
//!
//! // Load configuration
//! let config = CoreConfig::load()?;
//!
//! // Create a session
//! let mut session_manager = SessionManager::new(&config.session.storage_dir)?;
//! let session = session_manager.create_session(".")?;
//!
//! // Create a planner and plan a task
//! let planner = SimplePlanner::new();
//! let plan = planner.plan("Add a new feature", &PlanningContext::new()).await?;
//!
//! // Execute with consent checking
//! let mut consent_manager = ConsentManager::new(config.risk_level);
//! let executor = StandardExecutor::new();
//!
//! for step in plan.steps() {
//!     let request = ConsentRequest::new(&step.description, step.risk);
//!     if consent_manager.request_consent(&request)? == ConsentDecision::Approved {
//!         let result = executor.execute_step(step, &ExecutionContext::default()).await?;
//!         // Verify the result
//!         let verifier = StandardVerifier::new();
//!         let verification = verifier.verify(&result, &VerificationContext::default()).await?;
//!     }
//! }
//! ```

// Core modules - all public for complete access
pub mod config;
pub mod consent;
pub mod executor;
pub mod memory;
pub mod minime;
pub mod planner;
pub mod risk;
pub mod rollback;
pub mod sandbox;
pub mod session;
pub mod verifier;

use thiserror::Error;

/// Core error types
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Planning failed: {0}")]
    PlanningError(String),

    #[error("Execution failed: {0}")]
    ExecutionError(String),

    #[error("Verification failed: {0}")]
    VerificationError(String),

    #[error("User cancelled the operation")]
    UserCancelled,

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Provider error: {0}")]
    ProviderError(#[from] ganesha_providers::ProviderError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Rollback failed: {0}")]
    RollbackError(String),

    #[error("Sandbox error: {0}")]
    SandboxError(String),

    #[error("Mini-Me error: {0}")]
    MiniMeError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Consent error: {0}")]
    ConsentError(String),

    #[error("Memory error: {0}")]
    MemoryError(String),

    #[error("Planner error: {0}")]
    PlannerError(String),

    #[error("Executor error: {0}")]
    ExecutorError(String),

    #[error("Verifier error: {0}")]
    VerifierError(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

// ============================================================================
// Risk exports
// ============================================================================
pub use risk::{OperationRisk, RiskLevel};

// ============================================================================
// Memory system exports
// ============================================================================
pub use memory::{
    // Conversation Memory
    Role, Message as MemoryMessage, Conversation, ConversationMetadata, ConversationSummary,
    SummarizationRequest,

    // Knowledge Graph
    Entity, EntityType, Relationship, RelationType,
    EntityQuery, RelationshipQuery, KnowledgeGraph, KnowledgeGraphStats,

    // File Context Memory
    FileStatus, FileContext, CodeElement, FileContextMemory,

    // Session Persistence (memory)
    SessionState, SessionManager as MemorySessionManager, SessionMetadata,

    // Semantic Search
    SearchResult, SearchableMemory, SemanticSearchConfig, Embedding, TextSearch,

    // Unified Memory System
    MemorySystem, UnifiedSearchResults,

    // Error Types
    MemoryError,
};

// ============================================================================
// MiniMe Subagent exports
// ============================================================================
pub use minime::{
    // Core types
    SubAgent, AgentId, AgentStatus, AgentHandle, AgentResult, TokenUsage,
    // Manager
    SubAgentManager, MAX_CONCURRENT_AGENTS, DEFAULT_TIMEOUT_SECS,
    // Progress
    ProgressUpdate, ProgressType,
    // Task distribution
    WorkItem, OutputType, TaskSplitter, ResultAggregator,
    LlmTaskSplitter, SimpleAggregator, LlmAggregator,
    // Model selection
    ModelSelector,
    // Agent types
    AgentType, SpecializedAgentBuilder,
    // Orchestrator
    TaskOrchestrator,
};

// ============================================================================
// Sandbox exports
// ============================================================================
pub use sandbox::{
    Sandbox, SandboxConfig, SandboxMode, SandboxManager,
    ExecutedCommand, ApplyResult, SandboxError,
};

// ============================================================================
// Rollback exports
// ============================================================================
pub use rollback::{
    Checkpoint as RollbackCheckpoint, FileBackup, RollbackManager, RollbackResult,
    AutoCheckpoint, RollbackError,
};

// ============================================================================
// Planner exports
// ============================================================================
pub use planner::{
    ActionType, PlanBuilder, PlanStep, Planner, PlannerError, PlanningContext,
    RollbackStrategy, SimplePlanner, StepId, TaskPlan,
};

// ============================================================================
// Executor exports
// ============================================================================
pub use executor::{
    ExecutionContext, ExecutionResult, Executor, ExecutorError,
    FileChange, FileChangeType, StandardExecutor,
};

// ============================================================================
// Verifier exports
// ============================================================================
pub use verifier::{
    CheckType, IssueSeverity, StandardVerifier, VerificationContext,
    VerificationIssue, VerificationResult, VerificationStatus, Verifier, VerifierError,
};

// ============================================================================
// Consent exports
// ============================================================================
pub use consent::{
    ConsentDecision, ConsentError, ConsentLevel, ConsentManager, ConsentRequest,
    ConsentResponse, ConsentRule, ConsentRuleBuilder, OperationCategory, RememberScope,
};

// ============================================================================
// Session exports
// ============================================================================
pub use session::{
    Checkpoint, Message, MessageRole, Session, SessionError,
    SessionManager, SessionStatus, SessionSummary, ToolCall,
};

// ============================================================================
// Config exports
// ============================================================================
pub use config::{
    AiConfig, ConfigBuilder, ConfigError, CoreConfig, DisplayConfig,
    ExecutionConfig, McpConfig, McpServerConfig, SessionConfig, VerificationConfig,
};

/// Prelude module for convenient imports
pub mod prelude {
    // Re-export everything from the parent module
    pub use super::{
        // Core error
        CoreError, Result,

        // Risk
        OperationRisk, RiskLevel,

        // Planner
        ActionType, PlanBuilder, PlanStep, Planner, PlannerError, PlanningContext,
        RollbackStrategy, SimplePlanner, StepId, TaskPlan,

        // Executor
        ExecutionContext, ExecutionResult, Executor, ExecutorError,
        FileChange, FileChangeType, StandardExecutor,

        // Verifier
        CheckType, IssueSeverity, StandardVerifier, VerificationContext,
        VerificationIssue, VerificationResult, VerificationStatus, Verifier, VerifierError,

        // Consent
        ConsentDecision, ConsentError, ConsentLevel, ConsentManager, ConsentRequest,
        ConsentResponse, ConsentRule, ConsentRuleBuilder, OperationCategory, RememberScope,

        // Session
        Checkpoint, Message, MessageRole, Session, SessionError,
        SessionManager, SessionStatus, SessionSummary, ToolCall,

        // Config
        AiConfig, ConfigBuilder, ConfigError, CoreConfig, DisplayConfig,
        ExecutionConfig, McpConfig, McpServerConfig, SessionConfig, VerificationConfig,

        // Sandbox
        Sandbox, SandboxConfig, SandboxMode, SandboxManager, SandboxError,

        // Rollback
        RollbackCheckpoint, RollbackManager, RollbackResult, RollbackError,

        // Memory
        MemorySystem, Conversation, FileContextMemory, KnowledgeGraph,

        // MiniMe
        SubAgent, AgentId, AgentStatus, AgentHandle, AgentResult,
        SubAgentManager, WorkItem, OutputType, AgentType, ModelSelector, TaskOrchestrator,
    };
}

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_prelude_imports() {
        // Just test that prelude types are accessible
        use crate::prelude::*;

        let _ = RiskLevel::Normal;
        let _ = OperationRisk::ReadOnly;
    }
}
