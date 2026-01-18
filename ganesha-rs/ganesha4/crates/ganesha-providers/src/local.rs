//! # Local Provider
//!
//! Implementation for local LLM providers:
//! - LM Studio
//! - Ollama
//! - llama.cpp server
//! - vLLM
//! - Text Generation WebUI

use crate::{
    GenerateOptions, LlmProvider, Message, MessageRole, ModelInfo, ModelTier,
    ProviderError, Response, Result, Usage, get_model_tier,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use std::time::Duration;

/// Type of local provider
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalProviderType {
    /// LM Studio (default port 1234)
    LmStudio,
    /// Ollama (default port 11434)
    Ollama,
    /// llama.cpp server (default port 8080)
    LlamaCpp,
    /// vLLM (default port 8000)
    Vllm,
    /// Text Generation WebUI (default port 5000)
    TextGenWebUi,
    /// Generic OpenAI-compatible
    OpenAiCompatible,
}

impl LocalProviderType {
    /// Get the default port for this provider type
    pub fn default_port(&self) -> u16 {
        match self {
            Self::LmStudio => 1234,
            Self::Ollama => 11434,
            Self::LlamaCpp => 8080,
            Self::Vllm => 8000,
            Self::TextGenWebUi => 5000,
            Self::OpenAiCompatible => 8080,
        }
    }

    /// Get the default base URL for this provider type
    pub fn default_base_url(&self) -> String {
        let port = self.default_port();
        match self {
            Self::Ollama => format!("http://localhost:{}/api", port),
            _ => format!("http://localhost:{}/v1", port),
        }
    }
}

/// Local LLM provider (LM Studio, Ollama, etc.)
pub struct LocalProvider {
    client: Client,
    provider_type: LocalProviderType,
    base_url: String,
    default_model: Option<String>,
}

impl LocalProvider {
    /// Create a new local provider
    pub fn new(provider_type: LocalProviderType) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(300)) // 5 min timeout for local inference
                .build()
                .unwrap(),
            base_url: provider_type.default_base_url(),
            provider_type,
            default_model: None,
        }
    }

    /// Create with a custom base URL
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the default model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Detect available local providers
    pub async fn detect_available() -> Vec<LocalProvider> {
        let mut available = Vec::new();

        // Try each provider type
        for provider_type in [
            LocalProviderType::LmStudio,
            LocalProviderType::Ollama,
            LocalProviderType::LlamaCpp,
            LocalProviderType::Vllm,
        ] {
            let provider = LocalProvider::new(provider_type);
            if provider.is_available().await {
                info!("Detected local provider: {:?}", provider_type);
                available.push(provider);
            }
        }

        available
    }

    /// Convert messages to provider-specific format
    fn convert_messages(&self, messages: &[Message]) -> Vec<LocalMessage> {
        messages
            .iter()
            .map(|m| LocalMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "tool".to_string(),
                },
                content: m.content.clone(),
            })
            .collect()
    }

    /// Chat using Ollama's native API
    async fn chat_ollama(
        &self,
        messages: &[Message],
        options: &GenerateOptions,
    ) -> Result<Response> {
        let model = options
            .model
            .clone()
            .or_else(|| self.default_model.clone())
            .ok_or_else(|| ProviderError::ConfigError("No model specified".to_string()))?;

        let request = OllamaChatRequest {
            model: model.clone(),
            messages: self.convert_messages(messages),
            stream: false,
            options: OllamaOptions {
                temperature: options.temperature,
                num_predict: options.max_tokens.map(|t| t as i32),
                stop: options.stop.clone(),
            },
        };

        let url = format!("{}/chat", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError {
                status,
                message: body,
            });
        }

        let chat_response: OllamaChatResponse = response.json().await?;

        Ok(Response {
            content: chat_response.message.content,
            model,
            finish_reason: Some("stop".to_string()),
            usage: Some(Usage {
                prompt_tokens: chat_response.prompt_eval_count.unwrap_or(0),
                completion_tokens: chat_response.eval_count.unwrap_or(0),
                total_tokens: chat_response.prompt_eval_count.unwrap_or(0)
                    + chat_response.eval_count.unwrap_or(0),
            }),
        })
    }

    /// Chat using OpenAI-compatible API (LM Studio, vLLM, etc.)
    async fn chat_openai_compatible(
        &self,
        messages: &[Message],
        options: &GenerateOptions,
    ) -> Result<Response> {
        let model = options
            .model
            .clone()
            .or_else(|| self.default_model.clone())
            .unwrap_or_else(|| "default".to_string());

        let mut oai_messages = self.convert_messages(messages);

        // Add system prompt if provided
        if let Some(system) = &options.system {
            oai_messages.insert(
                0,
                LocalMessage {
                    role: "system".to_string(),
                    content: system.clone(),
                },
            );
        }

        let request = OpenAiCompatRequest {
            model: model.clone(),
            messages: oai_messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            stop: options.stop.clone(),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError {
                status,
                message: body,
            });
        }

        let chat_response: OpenAiCompatResponse = response.json().await?;

        let choice = chat_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".to_string()))?;

        Ok(Response {
            content: choice.message.content.unwrap_or_default(),
            model,
            finish_reason: choice.finish_reason,
            usage: chat_response.usage.map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        })
    }
}

#[async_trait]
impl LlmProvider for LocalProvider {
    fn name(&self) -> &str {
        match self.provider_type {
            LocalProviderType::LmStudio => "lmstudio",
            LocalProviderType::Ollama => "ollama",
            LocalProviderType::LlamaCpp => "llamacpp",
            LocalProviderType::Vllm => "vllm",
            LocalProviderType::TextGenWebUi => "textgenwebui",
            LocalProviderType::OpenAiCompatible => "local",
        }
    }

    async fn is_available(&self) -> bool {
        // Try to connect to the server
        let health_url = match self.provider_type {
            LocalProviderType::Ollama => format!("{}/tags", self.base_url),
            _ => format!("{}/models", self.base_url),
        };

        match self.client.get(&health_url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    fn default_model(&self) -> &str {
        self.default_model.as_deref().unwrap_or("default")
    }

    fn model_tier(&self, model: &str) -> ModelTier {
        get_model_tier(model)
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        match self.provider_type {
            LocalProviderType::Ollama => {
                let url = format!("{}/tags", self.base_url);
                let response = self.client.get(&url).send().await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    return Err(ProviderError::ApiError {
                        status,
                        message: body,
                    });
                }

                let models_response: OllamaModelsResponse = response.json().await?;

                Ok(models_response
                    .models
                    .into_iter()
                    .map(|m| ModelInfo {
                        id: m.name.clone(),
                        name: m.name.clone(),
                        provider: "ollama".to_string(),
                        tier: get_model_tier(&m.name),
                        context_length: None,
                        supports_vision: m.name.contains("llava") || m.name.contains("vision"),
                        supports_tools: false,
                    })
                    .collect())
            }
            _ => {
                // OpenAI-compatible endpoint
                let url = format!("{}/models", self.base_url);
                let response = self.client.get(&url).send().await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    return Err(ProviderError::ApiError {
                        status,
                        message: body,
                    });
                }

                let models_response: OpenAiCompatModelsResponse = response.json().await?;

                Ok(models_response
                    .data
                    .into_iter()
                    .map(|m| ModelInfo {
                        id: m.id.clone(),
                        name: m.id.clone(),
                        provider: self.name().to_string(),
                        tier: get_model_tier(&m.id),
                        context_length: None,
                        supports_vision: false,
                        supports_tools: false,
                    })
                    .collect())
            }
        }
    }

    async fn chat(&self, messages: &[Message], options: &GenerateOptions) -> Result<Response> {
        debug!("Local provider chat with {:?}", self.provider_type);

        match self.provider_type {
            LocalProviderType::Ollama => self.chat_ollama(messages, options).await,
            _ => self.chat_openai_compatible(messages, options).await,
        }
    }
}

// API types

#[derive(Debug, Serialize)]
struct LocalMessage {
    role: String,
    content: String,
}

// Ollama types

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<LocalMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    prompt_eval_count: Option<u32>,
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

// OpenAI-compatible types

#[derive(Debug, Serialize)]
struct OpenAiCompatRequest {
    model: String,
    messages: Vec<LocalMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatResponse {
    choices: Vec<OpenAiCompatChoice>,
    usage: Option<OpenAiCompatUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatChoice {
    message: OpenAiCompatMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatModelsResponse {
    data: Vec<OpenAiCompatModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatModel {
    id: String,
}
