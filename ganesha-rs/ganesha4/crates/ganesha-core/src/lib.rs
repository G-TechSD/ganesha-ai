//! # Ganesha Core
//!
//! The heart of Ganesha - task planning, execution, verification, and memory.
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
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
//! │  │ Rollback │  │ Sandbox  │  │  MiniMe  │                  │
//! │  └──────────┘  └──────────┘  └──────────┘                  │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod risk;

// These modules are placeholders for future implementation
mod planner;
mod executor;
mod verifier;
mod memory;
mod rollback;
mod sandbox;
mod minime;
mod consent;
mod session;
mod config;

pub use risk::RiskLevel;

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
}

pub type Result<T> = std::result::Result<T, CoreError>;
