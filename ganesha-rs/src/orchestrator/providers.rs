//! Provider System with OAuth2 and Dynamic Model Discovery
//!
//! Supports:
//! - Local: LM Studio, Ollama
//! - Cloud: OpenAI (GPT-5.2), Anthropic (Opus 4.5), Google (Gemini 3)
//!
//! Authentication:
//! - OAuth2 for interactive login
//! - API keys for automation/CI
//! - Token refresh and caching

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderType {
    LmStudio,
    Ollama,
    OpenAI,
    Anthropic,
    Google,
    Azure,
    Groq,
    Together,
    Custom,
}

/// Authentication method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    None,
    ApiKey(String),
    OAuth2 {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<u64>,
    },
    Bearer(String),
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
    pub context_window: u32,
    pub max_output: u32,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub input_cost_per_1m: f64,
    pub output_cost_per_1m: f64,
    pub tier: ModelTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    Fast,       // Quick responses, lower capability
    Standard,   // Good balance
    Capable,    // High capability
    Vision,     // Optimized for vision
    Premium,    // Best available
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEndpoint {
    pub provider_type: ProviderType,
    pub name: String,
    pub base_url: String,
    pub auth: AuthMethod,
    pub default_model: String,
    pub enabled: bool,
    pub priority: u32,
}

/// OAuth2 configuration for each provider
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
}

impl OAuth2Config {
    pub fn openai() -> Self {
        Self {
            client_id: std::env::var("OPENAI_CLIENT_ID")
                .unwrap_or_else(|_| "ganesha-cli".into()),
            client_secret: std::env::var("OPENAI_CLIENT_SECRET").ok(),
            auth_url: "https://auth.openai.com/authorize".into(),
            token_url: "https://auth.openai.com/oauth/token".into(),
            scopes: vec!["openai.chat".into(), "openai.models.read".into()],
            redirect_uri: "http://localhost:8420/oauth/callback".into(),
        }
    }

    pub fn google() -> Self {
        Self {
            client_id: std::env::var("GOOGLE_CLIENT_ID")
                .unwrap_or_else(|_| "ganesha-cli".into()),
            client_secret: std::env::var("GOOGLE_CLIENT_SECRET").ok(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            scopes: vec![
                "https://www.googleapis.com/auth/generative-language".into(),
            ],
            redirect_uri: "http://localhost:8420/oauth/callback".into(),
        }
    }

    pub fn anthropic() -> Self {
        Self {
            client_id: std::env::var("ANTHROPIC_CLIENT_ID")
                .unwrap_or_else(|_| "ganesha-cli".into()),
            client_secret: std::env::var("ANTHROPIC_CLIENT_SECRET").ok(),
            auth_url: "https://console.anthropic.com/oauth/authorize".into(),
            token_url: "https://api.anthropic.com/oauth/token".into(),
            scopes: vec!["messages:write".into(), "models:read".into()],
            redirect_uri: "http://localhost:8420/oauth/callback".into(),
        }
    }
}

/// The unified provider manager
pub struct ProviderManager {
    endpoints: HashMap<String, ProviderEndpoint>,
    models_cache: Arc<RwLock<HashMap<ProviderType, Vec<ModelInfo>>>>,
    cache_expiry: Arc<RwLock<HashMap<ProviderType, Instant>>>,
    config_path: PathBuf,
    client: reqwest::Client,
}

impl ProviderManager {
    pub fn new() -> Self {
        let config_path = Self::get_config_path();
        let endpoints = Self::load_or_default(&config_path);

        Self {
            endpoints,
            models_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_expiry: Arc::new(RwLock::new(HashMap::new())),
            config_path,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    fn get_config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".ganesha").join("providers.json")
    }

    fn load_or_default(path: &PathBuf) -> HashMap<String, ProviderEndpoint> {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(endpoints) = serde_json::from_str(&content) {
                    return endpoints;
                }
            }
        }
        Self::default_endpoints()
    }

    fn default_endpoints() -> HashMap<String, ProviderEndpoint> {
        let mut endpoints = HashMap::new();

        // LM Studio - BEAST (primary local)
        endpoints.insert("beast".into(), ProviderEndpoint {
            provider_type: ProviderType::LmStudio,
            name: "LM Studio BEAST".into(),
            base_url: "http://192.168.245.155:1234".into(),
            auth: AuthMethod::None,
            default_model: "gpt-oss-20b".into(),
            enabled: true,
            priority: 1,
        });

        // LM Studio - BEDROOM (fast/vision)
        endpoints.insert("bedroom".into(), ProviderEndpoint {
            provider_type: ProviderType::LmStudio,
            name: "LM Studio BEDROOM".into(),
            base_url: "http://192.168.27.182:1234".into(),
            auth: AuthMethod::None,
            default_model: "ministral-3-3b".into(),
            enabled: true,
            priority: 2,
        });

        // LM Studio - localhost
        endpoints.insert("local".into(), ProviderEndpoint {
            provider_type: ProviderType::LmStudio,
            name: "LM Studio Local".into(),
            base_url: "http://localhost:1234".into(),
            auth: AuthMethod::None,
            default_model: "default".into(),
            enabled: true,
            priority: 3,
        });

        // Ollama
        endpoints.insert("ollama".into(), ProviderEndpoint {
            provider_type: ProviderType::Ollama,
            name: "Ollama".into(),
            base_url: "http://localhost:11434".into(),
            auth: AuthMethod::None,
            default_model: "llama3.3".into(),
            enabled: true,
            priority: 4,
        });

        // OpenAI - GPT-5.2
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            endpoints.insert("openai".into(), ProviderEndpoint {
                provider_type: ProviderType::OpenAI,
                name: "OpenAI".into(),
                base_url: "https://api.openai.com".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "gpt-5.2".into(), // Latest
                enabled: true,
                priority: 10,
            });
        }

        // Anthropic - Claude Opus 4.5
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            endpoints.insert("anthropic".into(), ProviderEndpoint {
                provider_type: ProviderType::Anthropic,
                name: "Anthropic".into(),
                base_url: "https://api.anthropic.com".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "claude-opus-4-5-20251101".into(), // Latest
                enabled: true,
                priority: 11,
            });
        }

        // Google - Gemini 3
        if let Ok(key) = std::env::var("GOOGLE_API_KEY") {
            endpoints.insert("google".into(), ProviderEndpoint {
                provider_type: ProviderType::Google,
                name: "Google AI".into(),
                base_url: "https://generativelanguage.googleapis.com".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "gemini-3-pro".into(), // Latest
                enabled: true,
                priority: 12,
            });
        }

        // Groq (fast inference)
        if let Ok(key) = std::env::var("GROQ_API_KEY") {
            endpoints.insert("groq".into(), ProviderEndpoint {
                provider_type: ProviderType::Groq,
                name: "Groq".into(),
                base_url: "https://api.groq.com/openai".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "llama-3.3-70b-versatile".into(),
                enabled: true,
                priority: 5,
            });
        }

        // Together AI
        if let Ok(key) = std::env::var("TOGETHER_API_KEY") {
            endpoints.insert("together".into(), ProviderEndpoint {
                provider_type: ProviderType::Together,
                name: "Together AI".into(),
                base_url: "https://api.together.xyz".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "meta-llama/Llama-3.3-70B-Instruct-Turbo".into(),
                enabled: true,
                priority: 6,
            });
        }

        endpoints
    }

    /// Save current configuration
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.endpoints)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// Get available endpoints sorted by priority
    pub fn get_available(&self) -> Vec<&ProviderEndpoint> {
        let mut endpoints: Vec<_> = self.endpoints.values()
            .filter(|e| e.enabled)
            .collect();
        endpoints.sort_by_key(|e| e.priority);
        endpoints
    }

    /// Check if an endpoint is online
    pub async fn check_endpoint(&self, name: &str) -> bool {
        let endpoint = match self.endpoints.get(name) {
            Some(e) => e,
            None => return false,
        };

        let url = match endpoint.provider_type {
            ProviderType::Ollama => format!("{}/api/tags", endpoint.base_url),
            _ => format!("{}/v1/models", endpoint.base_url),
        };

        self.client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Fetch models from a provider
    pub async fn fetch_models(&self, provider_type: ProviderType) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        // Check cache first
        {
            let cache = self.models_cache.read().await;
            let expiry = self.cache_expiry.read().await;

            if let (Some(models), Some(exp)) = (cache.get(&provider_type), expiry.get(&provider_type)) {
                if exp.elapsed() < Duration::from_secs(3600) {
                    return Ok(models.clone());
                }
            }
        }

        // Fetch from API
        let models = match provider_type {
            ProviderType::OpenAI => self.fetch_openai_models().await?,
            ProviderType::Anthropic => self.fetch_anthropic_models().await?,
            ProviderType::Google => self.fetch_google_models().await?,
            ProviderType::Ollama => self.fetch_ollama_models().await?,
            ProviderType::LmStudio => self.fetch_lmstudio_models().await?,
            ProviderType::Groq => self.fetch_groq_models().await?,
            _ => vec![],
        };

        // Update cache
        {
            let mut cache = self.models_cache.write().await;
            let mut expiry = self.cache_expiry.write().await;

            cache.insert(provider_type, models.clone());
            expiry.insert(provider_type, Instant::now());
        }

        Ok(models)
    }

    async fn fetch_openai_models(&self) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = self.endpoints.get("openai");
        let auth = match endpoint {
            Some(e) => &e.auth,
            None => return Ok(Self::default_openai_models()),
        };

        let mut req = self.client.get("https://api.openai.com/v1/models");
        req = self.apply_auth(req, auth);

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let json: serde_json::Value = resp.json().await?;
                let models = json["data"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                let id = m["id"].as_str()?;
                                // Filter to chat models
                                if id.contains("gpt") || id.contains("o1") || id.contains("o3") {
                                    Some(ModelInfo {
                                        id: id.to_string(),
                                        name: id.to_string(),
                                        provider: ProviderType::OpenAI,
                                        context_window: self.infer_context_window(id),
                                        max_output: 16384,
                                        supports_vision: id.contains("vision") || id.contains("gpt-4") || id.contains("gpt-5"),
                                        supports_tools: true,
                                        input_cost_per_1m: self.infer_cost(id, true),
                                        output_cost_per_1m: self.infer_cost(id, false),
                                        tier: self.infer_tier(id),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_else(Self::default_openai_models);
                Ok(models)
            }
            _ => Ok(Self::default_openai_models()),
        }
    }

    fn default_openai_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gpt-5.2".into(),
                name: "GPT-5.2".into(),
                provider: ProviderType::OpenAI,
                context_window: 256000,
                max_output: 32768,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 5.0,
                output_cost_per_1m: 15.0,
                tier: ModelTier::Premium,
            },
            ModelInfo {
                id: "gpt-5.2-mini".into(),
                name: "GPT-5.2 Mini".into(),
                provider: ProviderType::OpenAI,
                context_window: 128000,
                max_output: 16384,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 0.5,
                output_cost_per_1m: 1.5,
                tier: ModelTier::Standard,
            },
            ModelInfo {
                id: "o3-mini".into(),
                name: "O3 Mini".into(),
                provider: ProviderType::OpenAI,
                context_window: 128000,
                max_output: 65536,
                supports_vision: false,
                supports_tools: true,
                input_cost_per_1m: 1.1,
                output_cost_per_1m: 4.4,
                tier: ModelTier::Capable,
            },
            ModelInfo {
                id: "gpt-4o".into(),
                name: "GPT-4o".into(),
                provider: ProviderType::OpenAI,
                context_window: 128000,
                max_output: 16384,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 2.5,
                output_cost_per_1m: 10.0,
                tier: ModelTier::Capable,
            },
        ]
    }

    async fn fetch_anthropic_models(&self) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        // Anthropic doesn't have a public models list endpoint, use defaults
        Ok(Self::default_anthropic_models())
    }

    fn default_anthropic_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-opus-4-5-20251101".into(),
                name: "Claude Opus 4.5".into(),
                provider: ProviderType::Anthropic,
                context_window: 200000,
                max_output: 32768,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 15.0,
                output_cost_per_1m: 75.0,
                tier: ModelTier::Premium,
            },
            ModelInfo {
                id: "claude-sonnet-4-20250514".into(),
                name: "Claude Sonnet 4".into(),
                provider: ProviderType::Anthropic,
                context_window: 200000,
                max_output: 16384,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 3.0,
                output_cost_per_1m: 15.0,
                tier: ModelTier::Capable,
            },
            ModelInfo {
                id: "claude-haiku-3-5-20241022".into(),
                name: "Claude Haiku 3.5".into(),
                provider: ProviderType::Anthropic,
                context_window: 200000,
                max_output: 8192,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 0.8,
                output_cost_per_1m: 4.0,
                tier: ModelTier::Fast,
            },
        ]
    }

    async fn fetch_google_models(&self) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = self.endpoints.get("google");
        let auth = match endpoint {
            Some(e) => &e.auth,
            None => return Ok(Self::default_google_models()),
        };

        let api_key = match auth {
            AuthMethod::ApiKey(k) => k.clone(),
            _ => return Ok(Self::default_google_models()),
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            api_key
        );

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let json: serde_json::Value = resp.json().await?;
                let models = json["models"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                let name = m["name"].as_str()?.replace("models/", "");
                                if name.contains("gemini") {
                                    Some(ModelInfo {
                                        id: name.clone(),
                                        name: m["displayName"].as_str().unwrap_or(&name).to_string(),
                                        provider: ProviderType::Google,
                                        context_window: m["inputTokenLimit"].as_u64().unwrap_or(32000) as u32,
                                        max_output: m["outputTokenLimit"].as_u64().unwrap_or(8192) as u32,
                                        supports_vision: name.contains("pro") || name.contains("flash"),
                                        supports_tools: true,
                                        input_cost_per_1m: 0.0, // Google has free tier
                                        output_cost_per_1m: 0.0,
                                        tier: if name.contains("ultra") || name.contains("3-pro") {
                                            ModelTier::Premium
                                        } else if name.contains("pro") {
                                            ModelTier::Capable
                                        } else {
                                            ModelTier::Fast
                                        },
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_else(Self::default_google_models);
                Ok(models)
            }
            _ => Ok(Self::default_google_models()),
        }
    }

    fn default_google_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gemini-3-pro".into(),
                name: "Gemini 3 Pro".into(),
                provider: ProviderType::Google,
                context_window: 2000000,
                max_output: 65536,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 1.25,
                output_cost_per_1m: 5.0,
                tier: ModelTier::Premium,
            },
            ModelInfo {
                id: "gemini-2.5-flash".into(),
                name: "Gemini 2.5 Flash".into(),
                provider: ProviderType::Google,
                context_window: 1000000,
                max_output: 8192,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 0.075,
                output_cost_per_1m: 0.3,
                tier: ModelTier::Fast,
            },
            ModelInfo {
                id: "gemini-2.0-flash".into(),
                name: "Gemini 2.0 Flash".into(),
                provider: ProviderType::Google,
                context_window: 1000000,
                max_output: 8192,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 0.0,
                output_cost_per_1m: 0.0,
                tier: ModelTier::Fast,
            },
        ]
    }

    async fn fetch_ollama_models(&self) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = self.endpoints.get("ollama");
        let base_url = match endpoint {
            Some(e) => &e.base_url,
            None => return Ok(vec![]),
        };

        match self.client.get(format!("{}/api/tags", base_url)).send().await {
            Ok(resp) if resp.status().is_success() => {
                let json: serde_json::Value = resp.json().await?;
                let models = json["models"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                let name = m["name"].as_str()?;
                                Some(ModelInfo {
                                    id: name.to_string(),
                                    name: name.to_string(),
                                    provider: ProviderType::Ollama,
                                    context_window: 8192,
                                    max_output: 4096,
                                    supports_vision: name.contains("llava") || name.contains("vision"),
                                    supports_tools: name.contains("llama3") || name.contains("qwen"),
                                    input_cost_per_1m: 0.0,
                                    output_cost_per_1m: 0.0,
                                    tier: ModelTier::Standard,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(models)
            }
            _ => Ok(vec![]),
        }
    }

    async fn fetch_lmstudio_models(&self) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_models = vec![];

        for (_, endpoint) in &self.endpoints {
            if endpoint.provider_type != ProviderType::LmStudio {
                continue;
            }

            if let Ok(resp) = self.client
                .get(format!("{}/v1/models", endpoint.base_url))
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(arr) = json["data"].as_array() {
                            for m in arr {
                                if let Some(id) = m["id"].as_str() {
                                    all_models.push(ModelInfo {
                                        id: id.to_string(),
                                        name: id.to_string(),
                                        provider: ProviderType::LmStudio,
                                        context_window: 32768,
                                        max_output: 8192,
                                        supports_vision: id.contains("vision") || id.contains("llava"),
                                        supports_tools: true,
                                        input_cost_per_1m: 0.0,
                                        output_cost_per_1m: 0.0,
                                        tier: ModelTier::Standard,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(all_models)
    }

    async fn fetch_groq_models(&self) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![
            ModelInfo {
                id: "llama-3.3-70b-versatile".into(),
                name: "Llama 3.3 70B".into(),
                provider: ProviderType::Groq,
                context_window: 128000,
                max_output: 32768,
                supports_vision: false,
                supports_tools: true,
                input_cost_per_1m: 0.59,
                output_cost_per_1m: 0.79,
                tier: ModelTier::Capable,
            },
            ModelInfo {
                id: "llama-3.2-90b-vision-preview".into(),
                name: "Llama 3.2 90B Vision".into(),
                provider: ProviderType::Groq,
                context_window: 128000,
                max_output: 8192,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 0.9,
                output_cost_per_1m: 0.9,
                tier: ModelTier::Vision,
            },
            ModelInfo {
                id: "mixtral-8x7b-32768".into(),
                name: "Mixtral 8x7B".into(),
                provider: ProviderType::Groq,
                context_window: 32768,
                max_output: 8192,
                supports_vision: false,
                supports_tools: true,
                input_cost_per_1m: 0.24,
                output_cost_per_1m: 0.24,
                tier: ModelTier::Fast,
            },
        ])
    }

    fn apply_auth(&self, req: reqwest::RequestBuilder, auth: &AuthMethod) -> reqwest::RequestBuilder {
        match auth {
            AuthMethod::ApiKey(key) => req.bearer_auth(key),
            AuthMethod::OAuth2 { access_token, .. } => req.bearer_auth(access_token),
            AuthMethod::Bearer(token) => req.bearer_auth(token),
            AuthMethod::None => req,
        }
    }

    fn infer_context_window(&self, model_id: &str) -> u32 {
        if model_id.contains("5.2") || model_id.contains("o3") {
            256000
        } else if model_id.contains("4o") || model_id.contains("4-turbo") {
            128000
        } else {
            32000
        }
    }

    fn infer_cost(&self, model_id: &str, is_input: bool) -> f64 {
        let base = if model_id.contains("5.2") {
            if is_input { 5.0 } else { 15.0 }
        } else if model_id.contains("o3") {
            if is_input { 1.1 } else { 4.4 }
        } else if model_id.contains("4o") {
            if is_input { 2.5 } else { 10.0 }
        } else {
            if is_input { 0.5 } else { 1.5 }
        };
        base
    }

    fn infer_tier(&self, model_id: &str) -> ModelTier {
        if model_id.contains("5.2") || model_id.contains("o3") {
            ModelTier::Premium
        } else if model_id.contains("4o") || model_id.contains("4-turbo") {
            ModelTier::Capable
        } else if model_id.contains("mini") || model_id.contains("flash") {
            ModelTier::Fast
        } else {
            ModelTier::Standard
        }
    }

    /// Start OAuth2 login flow
    pub async fn oauth2_login(&mut self, provider_type: ProviderType) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = match provider_type {
            ProviderType::OpenAI => OAuth2Config::openai(),
            ProviderType::Google => OAuth2Config::google(),
            ProviderType::Anthropic => OAuth2Config::anthropic(),
            _ => return Err("OAuth2 not supported for this provider".into()),
        };

        // Generate state for CSRF protection
        let state = uuid::Uuid::new_v4().to_string();

        // Build authorization URL
        let auth_url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            config.auth_url,
            urlencoding::encode(&config.client_id),
            urlencoding::encode(&config.redirect_uri),
            urlencoding::encode(&config.scopes.join(" ")),
            state
        );

        println!("\n\x1b[1;36mOAuth2 Login\x1b[0m");
        println!("Open this URL in your browser:\n");
        println!("  {}", auth_url);
        println!("\nWaiting for callback on {}...", config.redirect_uri);

        // Start local server to receive callback
        let (code, _) = self.wait_for_oauth_callback(&config.redirect_uri, &state).await?;

        // Exchange code for tokens
        let tokens = self.exchange_oauth_code(&config, &code).await?;

        // Update endpoint with OAuth2 auth
        let provider_name = match provider_type {
            ProviderType::OpenAI => "openai",
            ProviderType::Google => "google",
            ProviderType::Anthropic => "anthropic",
            _ => return Ok(()),
        };

        if let Some(endpoint) = self.endpoints.get_mut(provider_name) {
            endpoint.auth = AuthMethod::OAuth2 {
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                expires_at: tokens.expires_at,
            };
        } else {
            // Create new endpoint
            let base_url = match provider_type {
                ProviderType::OpenAI => "https://api.openai.com",
                ProviderType::Google => "https://generativelanguage.googleapis.com",
                ProviderType::Anthropic => "https://api.anthropic.com",
                _ => return Ok(()),
            };

            self.endpoints.insert(provider_name.into(), ProviderEndpoint {
                provider_type,
                name: provider_name.to_string(),
                base_url: base_url.into(),
                auth: AuthMethod::OAuth2 {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                    expires_at: tokens.expires_at,
                },
                default_model: match provider_type {
                    ProviderType::OpenAI => "gpt-5.2".into(),
                    ProviderType::Google => "gemini-3-pro".into(),
                    ProviderType::Anthropic => "claude-opus-4-5-20251101".into(),
                    _ => "default".into(),
                },
                enabled: true,
                priority: 10,
            });
        }

        self.save()?;
        println!("\n\x1b[32m✓ OAuth2 login successful!\x1b[0m");

        Ok(())
    }

    async fn wait_for_oauth_callback(&self, redirect_uri: &str, expected_state: &str) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
        use tokio::net::TcpListener;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        // Parse port from redirect URI
        let port: u16 = redirect_uri
            .split(':')
            .last()
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(8420);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

        let (mut socket, _) = listener.accept().await?;
        let (reader, mut writer) = socket.split();
        let mut reader = BufReader::new(reader);

        let mut request_line = String::new();
        reader.read_line(&mut request_line).await?;

        // Parse the request
        // GET /oauth/callback?code=xxx&state=yyy HTTP/1.1
        let url_part = request_line.split_whitespace().nth(1).unwrap_or("");
        let query = url_part.split('?').nth(1).unwrap_or("");

        let mut code = String::new();
        let mut state = String::new();

        for param in query.split('&') {
            let mut parts = param.split('=');
            match (parts.next(), parts.next()) {
                (Some("code"), Some(v)) => code = urlencoding::decode(v)?.to_string(),
                (Some("state"), Some(v)) => state = urlencoding::decode(v)?.to_string(),
                _ => {}
            }
        }

        // Send response
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Login Successful!</h1><p>You can close this window.</p></body></html>";
        writer.write_all(response.as_bytes()).await?;

        if state != expected_state {
            return Err("State mismatch - possible CSRF attack".into());
        }

        Ok((code, state))
    }

    async fn exchange_oauth_code(&self, config: &OAuth2Config, code: &str) -> Result<TokenResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("redirect_uri", &config.redirect_uri);
        params.insert("client_id", &config.client_id);

        let mut req = self.client.post(&config.token_url).form(&params);

        if let Some(ref secret) = config.client_secret {
            req = req.basic_auth(&config.client_id, Some(secret));
        }

        let resp = req.send().await?;

        if !resp.status().is_success() {
            let body = resp.text().await?;
            return Err(format!("Token exchange failed: {}", body).into());
        }

        let json: serde_json::Value = resp.json().await?;

        let expires_at = json["expires_in"]
            .as_u64()
            .map(|secs| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() + secs
            });

        Ok(TokenResponse {
            access_token: json["access_token"].as_str().unwrap_or("").to_string(),
            refresh_token: json["refresh_token"].as_str().map(|s| s.to_string()),
            expires_at,
        })
    }

    /// Refresh OAuth2 token if expired
    pub async fn refresh_token(&mut self, provider_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = match self.endpoints.get(provider_name) {
            Some(e) => e.clone(),
            None => return Ok(()),
        };

        let (refresh_token, expires_at) = match &endpoint.auth {
            AuthMethod::OAuth2 { refresh_token: Some(rt), expires_at: Some(exp), .. } => {
                (rt.clone(), *exp)
            }
            _ => return Ok(()),
        };

        // Check if token is expired
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if now < expires_at - 300 {
            // Still valid (with 5 min buffer)
            return Ok(());
        }

        let config = match endpoint.provider_type {
            ProviderType::OpenAI => OAuth2Config::openai(),
            ProviderType::Google => OAuth2Config::google(),
            ProviderType::Anthropic => OAuth2Config::anthropic(),
            _ => return Ok(()),
        };

        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", &refresh_token);
        params.insert("client_id", &config.client_id);

        let resp = self.client.post(&config.token_url).form(&params).send().await?;

        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await?;

            let new_expires_at = json["expires_in"]
                .as_u64()
                .map(|secs| now + secs);

            if let Some(e) = self.endpoints.get_mut(provider_name) {
                e.auth = AuthMethod::OAuth2 {
                    access_token: json["access_token"].as_str().unwrap_or("").to_string(),
                    refresh_token: json["refresh_token"]
                        .as_str()
                        .map(|s| s.to_string())
                        .or(Some(refresh_token)),
                    expires_at: new_expires_at,
                };
            }

            self.save()?;
        }

        Ok(())
    }

    /// Print provider status
    pub async fn print_status(&self) {
        println!("\n\x1b[1;36mProvider Status:\x1b[0m\n");

        for (name, endpoint) in &self.endpoints {
            let online = self.check_endpoint(name).await;
            let status = if online { "\x1b[32m●\x1b[0m" } else { "\x1b[31m○\x1b[0m" };
            let auth = match &endpoint.auth {
                AuthMethod::None => "no auth",
                AuthMethod::ApiKey(_) => "API key",
                AuthMethod::OAuth2 { .. } => "OAuth2",
                AuthMethod::Bearer(_) => "Bearer token",
            };

            println!("  {} {} ({:?})", status, name, endpoint.provider_type);
            println!("    URL: {}", endpoint.base_url);
            println!("    Auth: {} | Model: {}", auth, endpoint.default_model);
            println!();
        }
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_endpoints() {
        let endpoints = ProviderManager::default_endpoints();
        assert!(endpoints.contains_key("beast"));
        assert!(endpoints.contains_key("bedroom"));
    }

    #[test]
    fn test_default_models() {
        let openai = ProviderManager::default_openai_models();
        assert!(openai.iter().any(|m| m.id.contains("5.2")));

        let anthropic = ProviderManager::default_anthropic_models();
        assert!(anthropic.iter().any(|m| m.id.contains("opus")));

        let google = ProviderManager::default_google_models();
        assert!(google.iter().any(|m| m.id.contains("gemini-3")));
    }

    #[test]
    fn test_oauth2_config() {
        let openai = OAuth2Config::openai();
        assert!(openai.auth_url.contains("openai"));

        let google = OAuth2Config::google();
        assert!(google.auth_url.contains("google"));
    }
}
