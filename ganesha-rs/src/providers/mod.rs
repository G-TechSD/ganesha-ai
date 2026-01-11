//! LLM Provider Abstraction
//!
//! Supports local-first approach:
//! 1. LM Studio (local)
//! 2. Ollama (local)
//! 3. Anthropic Claude (cloud)
//! 4. OpenAI (cloud)

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("No providers available")]
    NoProviders,

    #[error("Timeout")]
    Timeout,
}

/// Chat message for conversation history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self { role: "system".into(), content: content.into() }
    }
    pub fn user(content: &str) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: &str) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

/// LLM Provider trait
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;

    /// Single-turn generation (for backwards compatibility)
    async fn generate(&self, system: &str, user: &str) -> Result<String, ProviderError>;

    /// Multi-turn generation with conversation history
    async fn generate_with_history(&self, messages: &[ChatMessage]) -> Result<String, ProviderError>;
}

/// OpenAI-compatible provider (LM Studio, OpenAI, etc.)
pub struct OpenAiCompatible {
    name: String,
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

impl OpenAiCompatible {
    pub fn lm_studio(url: &str) -> Self {
        Self {
            name: "lmstudio".into(),
            base_url: url.trim_end_matches('/').into(),
            api_key: None,
            model: "default".into(),
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap(),
        }
    }

    pub fn openai(api_key: &str) -> Self {
        Self {
            name: "openai".into(),
            base_url: "https://api.openai.com".into(),
            api_key: Some(api_key.into()),
            model: "gpt-4o".into(),
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatible {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        // Quick sync check - use std::thread to avoid async runtime conflicts
        let url = format!("{}/v1/models", self.base_url);
        let handle = std::thread::spawn(move || {
            reqwest::blocking::Client::new()
                .get(&url)
                .timeout(Duration::from_secs(2))
                .send()
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        });
        handle.join().unwrap_or(false)
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".into(),
                    content: system.into(),
                },
                Message {
                    role: "user".into(),
                    content: user.into(),
                },
            ],
            temperature: 0.3,
            max_tokens: 2000,
            stream: false,
        };

        let mut req = self.client.post(&url).json(&request);

        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("{}: {}", status, body)));
        }

        let chat_response: ChatResponse = response.json().await?;

        chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| ProviderError::Api("No response content".into()))
    }

    async fn generate_with_history(&self, messages: &[ChatMessage]) -> Result<String, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.iter().map(|m| Message {
                role: m.role.clone(),
                content: m.content.clone(),
            }).collect(),
            temperature: 0.3,
            max_tokens: 16000,  // Large responses for code/HTML generation
            stream: false,
        };

        let mut req = self.client.post(&url).json(&request);

        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("{}: {}", status, body)));
        }

        let chat_response: ChatResponse = response.json().await?;

        chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| ProviderError::Api("No response content".into()))
    }
}

/// Ollama provider
pub struct Ollama {
    base_url: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: Message,
}

impl Ollama {
    pub fn new(url: &str, model: &str) -> Self {
        Self {
            base_url: url.trim_end_matches('/').into(),
            model: model.into(),
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap(),
        }
    }

    pub fn default() -> Self {
        Self::new("http://localhost:11434", "llama3")
    }
}

#[async_trait]
impl LlmProvider for Ollama {
    fn name(&self) -> &str {
        "ollama"
    }

    fn is_available(&self) -> bool {
        // Quick sync check - use std::thread to avoid async runtime conflicts
        let url = format!("{}/api/tags", self.base_url);
        let handle = std::thread::spawn(move || {
            reqwest::blocking::Client::new()
                .get(&url)
                .timeout(Duration::from_secs(2))
                .send()
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        });
        handle.join().unwrap_or(false)
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        let url = format!("{}/api/chat", self.base_url);

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".into(),
                    content: system.into(),
                },
                Message {
                    role: "user".into(),
                    content: user.into(),
                },
            ],
            stream: false,
            options: OllamaOptions {
                temperature: 0.3,
                num_predict: 2000,
            },
        };

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(body));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        Ok(ollama_response.message.content)
    }

    async fn generate_with_history(&self, messages: &[ChatMessage]) -> Result<String, ProviderError> {
        let url = format!("{}/api/chat", self.base_url);

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: messages.iter().map(|m| Message {
                role: m.role.clone(),
                content: m.content.clone(),
            }).collect(),
            stream: false,
            options: OllamaOptions {
                temperature: 0.3,
                num_predict: 16000,  // Large responses for code/HTML generation
            },
        };

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(body));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        Ok(ollama_response.message.content)
    }
}

/// Anthropic Claude provider
pub struct Anthropic {
    api_key: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

impl Anthropic {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.into(),
            model: "claude-sonnet-4-20250514".into(),
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl LlmProvider for Anthropic {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 2000,
            system: system.into(),
            messages: vec![Message {
                role: "user".into(),
                content: user.into(),
            }],
            temperature: 0.3,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(body));
        }

        let anthropic_response: AnthropicResponse = response.json().await?;

        anthropic_response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| ProviderError::Api("No response content".into()))
    }

    async fn generate_with_history(&self, messages: &[ChatMessage]) -> Result<String, ProviderError> {
        // Extract system message and user/assistant messages
        let system = messages.iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let non_system: Vec<Message> = messages.iter()
            .filter(|m| m.role != "system")
            .map(|m| Message {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 16000,  // Large responses for code/HTML generation
            system,
            messages: non_system,
            temperature: 0.3,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(body));
        }

        let anthropic_response: AnthropicResponse = response.json().await?;

        anthropic_response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| ProviderError::Api("No response content".into()))
    }
}

/// Provider chain with fallback
pub struct ProviderChain {
    providers: Vec<Box<dyn LlmProvider>>,
    /// Provider URLs for agent mode access
    pub provider_urls: Vec<(String, String)>, // (url, model)
}

impl ProviderChain {
    pub fn new() -> Self {
        Self { providers: vec![], provider_urls: vec![] }
    }

    pub fn add<P: LlmProvider + 'static>(mut self, provider: P) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    /// Create default chain (local-first)
    pub fn default_chain() -> Self {
        let mut chain = Self::new();

        // LM Studio instances - track URLs for agent mode
        chain.provider_urls.push(("http://192.168.245.155:1234".into(), "default".into()));
        chain.provider_urls.push(("http://192.168.27.182:1234".into(), "default".into()));
        chain.provider_urls.push(("http://localhost:1234".into(), "default".into()));

        chain = chain.add(OpenAiCompatible::lm_studio("http://192.168.245.155:1234")); // BEAST
        chain = chain.add(OpenAiCompatible::lm_studio("http://192.168.27.182:1234")); // BEDROOM
        chain = chain.add(OpenAiCompatible::lm_studio("http://localhost:1234")); // Local

        // Ollama
        chain = chain.add(Ollama::default());

        // Cloud fallbacks (if API keys present)
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            chain = chain.add(Anthropic::new(&key));
        }
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            chain = chain.add(OpenAiCompatible::openai(&key));
        }

        chain
    }

    /// Get the first available provider URL and model for agent mode
    pub fn get_first_available_url(&self) -> Option<(String, String)> {
        for (url, model) in &self.provider_urls {
            // Quick check if this URL is available
            let check_url = format!("{}/v1/models", url);
            let handle = std::thread::spawn(move || {
                reqwest::blocking::Client::new()
                    .get(&check_url)
                    .timeout(std::time::Duration::from_secs(2))
                    .send()
                    .map(|r| r.status().is_success())
                    .unwrap_or(false)
            });
            if handle.join().unwrap_or(false) {
                return Some((url.clone(), model.clone()));
            }
        }
        None
    }

    pub fn get_available(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|p| p.is_available())
            .map(|p| p.name())
            .collect()
    }
}

#[async_trait]
impl LlmProvider for ProviderChain {
    fn name(&self) -> &str {
        "chain"
    }

    fn is_available(&self) -> bool {
        self.providers.iter().any(|p| p.is_available())
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        let mut errors = vec![];

        for provider in &self.providers {
            if !provider.is_available() {
                continue;
            }

            match provider.generate(system, user).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    errors.push(format!("{}: {}", provider.name(), e));
                }
            }
        }

        if errors.is_empty() {
            Err(ProviderError::NoProviders)
        } else {
            Err(ProviderError::Api(errors.join("; ")))
        }
    }

    async fn generate_with_history(&self, messages: &[ChatMessage]) -> Result<String, ProviderError> {
        let mut errors = vec![];

        for provider in &self.providers {
            if !provider.is_available() {
                continue;
            }

            match provider.generate_with_history(messages).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    errors.push(format!("{}: {}", provider.name(), e));
                }
            }
        }

        if errors.is_empty() {
            Err(ProviderError::NoProviders)
        } else {
            Err(ProviderError::Api(errors.join("; ")))
        }
    }
}

impl Default for ProviderChain {
    fn default() -> Self {
        Self::default_chain()
    }
}
