//! # Ganesha Providers
//!
//! LLM provider abstraction layer supporting local and cloud providers.
//!
//! ## Supported Providers
//!
//! ### Local First
//! - LM Studio
//! - Ollama
//! - llama.cpp server
//! - vLLM
//! - Text Generation WebUI
//!
//! ### Cloud
//! - OpenRouter (aggregator)
//! - Anthropic (Claude)
//! - OpenAI (GPT-4, etc)
//! - Google (Gemini)
//! - Groq
//!
//! ## Model Tiers
//!
//! Models are classified into quality tiers:
//! - 🟢 Exceptional: Best for agentic tasks
//! - 🟡 Capable: Good for most tasks
//! - 🟠 Limited: Simple tasks only
//! - 🔴 Unsafe: May produce dangerous commands

pub mod traits;
pub mod openai;
pub mod anthropic;
pub mod gemini;
pub mod openrouter;
pub mod local;
pub mod manager;
pub mod tiers;
pub mod message;

pub use traits::{
    LlmProvider, StreamingProvider, ToolProvider,
    Response, Usage, GenerateOptions,
    ToolDefinition, ToolResponse, ToolCall,
};
pub use openai::OpenAiProvider;
pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use openrouter::OpenRouterProvider;
pub use local::{LocalProvider, LocalProviderType};
pub use manager::{ProviderManager, ProviderPriority, ProviderConfig};
pub use tiers::{ModelTier, ModelInfo, get_model_tier};
pub use message::{Message, MessageRole};

use thiserror::Error;

/// Provider errors
#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Rate limited: retry after {retry_after:?}")]
    RateLimited { retry_after: Option<u64> },

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Provider not available: {0}")]
    Unavailable(String),

    #[error("Timeout after {0}s")]
    Timeout(u64),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

pub type Result<T> = std::result::Result<T, ProviderError>;
