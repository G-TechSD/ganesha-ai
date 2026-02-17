//! # Provider Manager
//!
//! Manages multiple LLM providers with automatic fallback and load balancing.

use crate::{
    GenerateOptions, LlmProvider, LocalProvider, Message, ModelInfo, ModelTier,
    OpenAiProvider, AnthropicProvider, GeminiProvider, OpenRouterProvider, ProviderError, Response, Result,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Provider priority for selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderPriority {
    /// Highest priority - use first
    Primary = 0,
    /// Second choice
    Secondary = 1,
    /// Fallback option
    Fallback = 2,
    /// Last resort
    LastResort = 3,
}

/// Configuration for a provider
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub priority: ProviderPriority,
    pub enabled: bool,
}

/// Provider with its configuration
struct ManagedProvider {
    provider: Arc<dyn LlmProvider>,
    config: ProviderConfig,
}

/// Manages multiple LLM providers
pub struct ProviderManager {
    providers: RwLock<Vec<ManagedProvider>>,
    default_provider: RwLock<Option<String>>,
    local_first: bool,
}

impl ProviderManager {
    /// Create a new provider manager
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(Vec::new()),
            default_provider: RwLock::new(None),
            local_first: true, // Prefer local by default
        }
    }

    /// Create with local-first preference
    pub fn local_first(mut self, enabled: bool) -> Self {
        self.local_first = enabled;
        self
    }

    /// Register a provider
    pub async fn register<P: LlmProvider + 'static>(
        &self,
        provider: P,
        priority: ProviderPriority,
    ) {
        let config = ProviderConfig {
            name: provider.name().to_string(),
            priority,
            enabled: true,
        };

        let managed = ManagedProvider {
            provider: Arc::new(provider),
            config,
        };

        let mut providers = self.providers.write().await;
        providers.push(managed);

        // Sort by priority
        providers.sort_by_key(|p| p.config.priority);

        info!("Registered provider: {} with priority {:?}",
              providers.last().unwrap().config.name,
              providers.last().unwrap().config.priority);
    }

    /// Auto-discover and register available providers
    pub async fn auto_discover(&self) -> Result<()> {
        info!("Auto-discovering available providers...");

        // Check for local providers first (if local-first mode)
        if self.local_first {
            let local_providers = LocalProvider::detect_available().await;
            for provider in local_providers {
                info!("Found local provider: {}", provider.name());
                self.register(provider, ProviderPriority::Primary).await;
            }
        }

        // Check for cloud providers via environment variables
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                info!("Found Anthropic API key");
                let priority = if self.local_first {
                    ProviderPriority::Secondary
                } else {
                    ProviderPriority::Primary
                };
                self.register(AnthropicProvider::new(key), priority).await;
            }
        }

        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.is_empty() {
                info!("Found OpenAI API key");
                let priority = if self.local_first {
                    ProviderPriority::Secondary
                } else {
                    ProviderPriority::Primary
                };
                self.register(OpenAiProvider::new(key), priority).await;
            }
        }

        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            if !key.is_empty() {
                info!("Found Gemini API key");
                let priority = if self.local_first {
                    ProviderPriority::Secondary
                } else {
                    ProviderPriority::Primary
                };
                self.register(GeminiProvider::new(key), priority).await;
            }
        }

        if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
            if !key.is_empty() {
                info!("Found OpenRouter API key");
                self.register(
                    OpenRouterProvider::new(key),
                    ProviderPriority::Fallback,
                ).await;
            }
        }

        Ok(())
    }

    /// Set the default provider by name
    pub async fn set_default(&self, name: &str) -> Result<()> {
        let providers = self.providers.read().await;
        if providers.iter().any(|p| p.config.name == name) {
            *self.default_provider.write().await = Some(name.to_string());
            Ok(())
        } else {
            Err(ProviderError::Unavailable(format!(
                "Provider '{}' not found",
                name
            )))
        }
    }

    /// Get the default provider
    pub async fn get_default(&self) -> Option<Arc<dyn LlmProvider>> {
        let default_name = self.default_provider.read().await;
        let providers = self.providers.read().await;

        if let Some(name) = default_name.as_ref() {
            providers
                .iter()
                .find(|p| &p.config.name == name)
                .map(|p| p.provider.clone())
        } else {
            // Return the first enabled provider
            providers
                .iter()
                .find(|p| p.config.enabled)
                .map(|p| p.provider.clone())
        }
    }

    /// Get a provider by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn LlmProvider>> {
        let providers = self.providers.read().await;
        providers
            .iter()
            .find(|p| p.config.name == name && p.config.enabled)
            .map(|p| p.provider.clone())
    }

    /// List all available providers
    pub async fn list_providers(&self) -> Vec<ProviderConfig> {
        let providers = self.providers.read().await;
        providers.iter().map(|p| p.config.clone()).collect()
    }

    /// List all available models across all providers
    pub async fn list_all_models(&self) -> Result<Vec<ModelInfo>> {
        let providers = self.providers.read().await;
        let mut all_models = Vec::new();

        for managed in providers.iter() {
            if managed.config.enabled {
                match managed.provider.list_models().await {
                    Ok(models) => all_models.extend(models),
                    Err(e) => debug!(
                        "Failed to list models from {}: {}",
                        managed.config.name, e
                    ),
                }
            }
        }

        Ok(all_models)
    }

    /// Chat with automatic provider selection and fallback
    pub async fn chat(
        &self,
        messages: &[Message],
        options: &GenerateOptions,
    ) -> Result<Response> {
        let providers = self.providers.read().await;

        // Try to find a specific provider if model specifies one
        if let Some(ref model) = options.model {
            // Check if model ID contains provider prefix (e.g., "anthropic/claude-3")
            if model.contains('/') {
                let parts: Vec<&str> = model.split('/').collect();
                if let Some(managed) = providers.iter().find(|p| {
                    p.config.enabled && p.config.name.contains(parts[0])
                }) {
                    debug!("Using provider {} for model {}", managed.config.name, model);
                    return managed.provider.chat(messages, options).await;
                }
            }
        }

        // Try providers in priority order
        let mut last_error = None;
        for managed in providers.iter() {
            if !managed.config.enabled {
                continue;
            }

            // Check if provider is available
            if !managed.provider.is_available().await {
                debug!("Provider {} not available, skipping", managed.config.name);
                continue;
            }

            debug!("Trying provider: {}", managed.config.name);
            match managed.provider.chat(messages, options).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    debug!("Provider {} failed: {}", managed.config.name, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or(ProviderError::Unavailable(
            "No providers available".to_string(),
        )))
    }

    /// Generate with automatic provider selection
    pub async fn generate(&self, system: &str, user: &str) -> Result<String> {
        let messages = vec![Message::system(system), Message::user(user)];
        let response = self.chat(&messages, &GenerateOptions::default()).await?;
        Ok(response.content)
    }

    /// Check if any provider is available
    pub async fn has_available_provider(&self) -> bool {
        let providers = self.providers.read().await;
        for managed in providers.iter() {
            if managed.config.enabled && managed.provider.is_available().await {
                return true;
            }
        }
        false
    }

    /// Get the tier for a model (checks all providers)
    pub fn model_tier(&self, model: &str) -> ModelTier {
        crate::get_model_tier(model)
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_manager_creation() {
        let manager = ProviderManager::new();
        assert!(manager.list_providers().await.is_empty());
    }

    #[tokio::test]
    async fn test_no_providers_available() {
        let manager = ProviderManager::new();
        assert!(!manager.has_available_provider().await);
    }
}
