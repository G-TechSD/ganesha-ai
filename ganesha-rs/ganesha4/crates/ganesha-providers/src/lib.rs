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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_error_display_http() {
        // Build a reqwest error from an invalid URL
        
        let err = ProviderError::Unavailable("test server".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("test server"));
    }

    #[test]
    fn test_provider_error_display_api() {
        let err = ProviderError::ApiError { status: 429, message: "rate limited".to_string() };
        let msg = format!("{}", err);
        assert!(msg.contains("429"));
        assert!(msg.contains("rate limited"));
    }

    #[test]
    fn test_provider_error_display_rate_limited() {
        let err = ProviderError::RateLimited { retry_after: Some(30) };
        let msg = format!("{}", err);
        assert!(msg.contains("Rate limited"));
    }

    #[test]
    fn test_provider_error_display_model_not_found() {
        let err = ProviderError::ModelNotFound("gpt-5".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("gpt-5"));
    }

    #[test]
    fn test_provider_error_display_auth() {
        let err = ProviderError::AuthError("invalid key".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Authentication failed"));
    }

    #[test]
    fn test_provider_error_display_unavailable() {
        let err = ProviderError::Unavailable("LM Studio".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("LM Studio"));
    }

    #[test]
    fn test_provider_error_display_timeout() {
        let err = ProviderError::Timeout(30);
        let msg = format!("{}", err);
        assert!(msg.contains("30"));
    }

    #[test]
    fn test_provider_error_display_config() {
        let err = ProviderError::ConfigError("missing API key".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Configuration error"));
    }

    #[test]
    fn test_provider_error_display_stream() {
        let err = ProviderError::StreamError("connection reset".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Stream error"));
    }

    #[test]
    fn test_provider_error_display_invalid_response() {
        let err = ProviderError::InvalidResponse("missing choices".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid response"));
    }

    // Cross-module integration tests
    #[test]
    fn test_all_providers_constructable() {
        let _openai = OpenAiProvider::new("test");
        let _anthropic = AnthropicProvider::new("test");
        let _gemini = GeminiProvider::new("test");
        let _openrouter = OpenRouterProvider::new("test");
        let _local = LocalProvider::new(LocalProviderType::LmStudio);
    }

    #[test]
    fn test_provider_priority_ordering() {
        assert!(ProviderPriority::Primary < ProviderPriority::Secondary);
        assert!(ProviderPriority::Secondary < ProviderPriority::Fallback);
        assert!(ProviderPriority::Fallback < ProviderPriority::LastResort);
    }

    #[test]
    fn test_model_tier_ordering() {
        // Ensure tier system works
        let tier = get_model_tier("claude-3-opus");
        assert!(matches!(tier, ModelTier::Exceptional | ModelTier::Capable | ModelTier::Limited | ModelTier::Unsafe));
    }

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_message_roles() {
        let user = Message::user("hi");
        let assistant = Message::assistant("hello");
        let system = Message::system("you are helpful");
        assert_eq!(user.role, MessageRole::User);
        assert_eq!(assistant.role, MessageRole::Assistant);
        assert_eq!(system.role, MessageRole::System);
    }
}
