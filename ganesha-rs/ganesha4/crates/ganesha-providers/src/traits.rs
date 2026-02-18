//! # Provider Traits
//!
//! Core traits that all LLM providers must implement.

use crate::{Message, ModelInfo, ModelTier, Result};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Response from an LLM
#[derive(Debug, Clone)]
pub struct Response {
    /// The generated content
    pub content: String,
    /// Model used
    pub model: String,
    /// Finish reason (stop, length, etc)
    pub finish_reason: Option<String>,
    /// Token usage
    pub usage: Option<Usage>,
}

/// Token usage information
#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Options for generation
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Model to use (None = default)
    pub model: Option<String>,
    /// Temperature (0.0-2.0)
    pub temperature: Option<f32>,
    /// Max tokens to generate
    pub max_tokens: Option<u32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// System prompt override
    pub system: Option<String>,
    /// Enable JSON mode
    pub json_mode: bool,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            model: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
            stop: None,
            system: None,
            json_mode: false,
        }
    }
}

/// Core LLM provider trait
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name (e.g., "openai", "anthropic", "lmstudio")
    fn name(&self) -> &str;

    /// Check if provider is available (online, has API key, etc)
    async fn is_available(&self) -> bool;

    /// Get the default model for this provider
    fn default_model(&self) -> &str;

    /// Get model tier information
    fn model_tier(&self, model: &str) -> ModelTier;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Generate a response (non-streaming)
    async fn chat(
        &self,
        messages: &[Message],
        options: &GenerateOptions,
    ) -> Result<Response>;

    /// Simple generate with just system and user prompts
    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        let messages = vec![
            Message::system(system),
            Message::user(user),
        ];
        let response = self.chat(&messages, &GenerateOptions::default()).await?;
        Ok(response.content)
    }
}

/// Streaming provider trait
#[async_trait]
pub trait StreamingProvider: LlmProvider {
    /// Generate a streaming response
    async fn stream(
        &self,
        messages: &[Message],
        options: &GenerateOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;
}

/// Provider that supports tool/function calling
#[async_trait]
pub trait ToolProvider: LlmProvider {
    /// Call with tools available
    async fn chat_with_tools(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        options: &GenerateOptions,
    ) -> Result<ToolResponse>;
}

/// Definition of a tool that can be called
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

/// Response that may include tool calls
#[derive(Debug, Clone)]
pub struct ToolResponse {
    /// Text content (if any)
    pub content: Option<String>,
    /// Tool calls to make
    pub tool_calls: Vec<ToolCall>,
    /// Finish reason
    pub finish_reason: Option<String>,
}

/// A requested tool call
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_options_default() {
        let opts = GenerateOptions::default();
        assert!(opts.model.is_none());
        assert_eq!(opts.temperature, Some(0.7));
        assert_eq!(opts.max_tokens, Some(4096));
        assert!(opts.stop.is_none());
        assert!(opts.system.is_none());
        assert!(!opts.json_mode);
    }

    #[test]
    fn test_response_fields() {
        let response = Response {
            content: "Hello!".to_string(),
            model: "test-model".to_string(),
            finish_reason: Some("stop".to_string()),
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };
        assert_eq!(response.content, "Hello!");
        assert_eq!(response.model, "test-model");
        let usage = response.usage.unwrap();
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read file contents".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        };
        assert_eq!(tool.name, "read_file");
        assert!(tool.parameters["properties"]["path"]["type"] == "string");
    }

    #[test]
    fn test_tool_response() {
        let resp = ToolResponse {
            content: Some("File contents here".to_string()),
            tool_calls: vec![
                ToolCall {
                    id: "call_1".to_string(),
                    name: "write_file".to_string(),
                    arguments: serde_json::json!({"path": "/tmp/test.txt", "content": "hello"}),
                }
            ],
            finish_reason: Some("tool_calls".to_string()),
        };
        assert!(resp.content.is_some());
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "write_file");
    }

    #[test]
    fn test_generate_options_custom() {
        let opts = GenerateOptions {
            model: Some("claude-3-opus".to_string()),
            temperature: Some(0.0),
            max_tokens: Some(100),
            stop: Some(vec!["---".to_string()]),
            system: Some("Be concise".to_string()),
            json_mode: true,
        };
        assert_eq!(opts.model.unwrap(), "claude-3-opus");
        assert_eq!(opts.temperature.unwrap(), 0.0);
        assert!(opts.json_mode);
    }
}
