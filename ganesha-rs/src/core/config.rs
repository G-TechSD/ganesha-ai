use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use toml;

/// Model tier for provider selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ModelTier {
    /// Fast local model for simple tasks (ministral-3b, llama-3.2-3b)
    Fast,
    /// Standard model for general tasks
    Standard,
    /// Capable local model for planning (gpt-oss-20b, qwen-32b)
    Capable,
    /// Vision model for screen analysis
    Vision,
    /// Cloud model for complex reasoning (Claude, GPT-4)
    Cloud,
    /// Premium cloud for critical decisions (Claude Opus, GPT-5)
    Premium,
}

use std::fmt;

/// Provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderType {
    LmStudio,
    Ollama,
    OpenAI,
    Anthropic,
    Google,
    Azure,
    OpenRouter,
    Custom,
}

impl fmt::Display for ProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderType::LmStudio => write!(f, "lmstudio"),
            ProviderType::Ollama => write!(f, "ollama"),
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::Google => write!(f, "google"),
            ProviderType::Azure => write!(f, "azure"),
            ProviderType::OpenRouter => write!(f, "openrouter"),
            ProviderType::Custom => write!(f, "custom"),
        }
    }
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

/// User-configurable tier mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierMapping {
    pub endpoint: String,
    pub model: String,
    pub description: String,
}

// Custom serialization for HashMap<u32, TierMapping> to make TOML happy
mod tier_map_serde {
    use super::TierMapping;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;

    pub fn serialize<S>(map: &HashMap<u32, TierMapping>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string_map: HashMap<String, &TierMapping> = map
            .iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        string_map.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<u32, TierMapping>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string_map: HashMap<String, TierMapping> = HashMap::deserialize(deserializer)?;
        let mut result = HashMap::new();
        for (k, v) in string_map {
            if let Ok(num) = k.parse::<u32>() {
                result.insert(num, v);
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    #[serde(with = "tier_map_serde")]
    pub tiers: HashMap<u32, TierMapping>,
    pub vision: Option<TierMapping>,
}

impl TierConfig {
    /// Get tier mapping for a number
    pub fn get(&self, tier: u32) -> Option<&TierMapping> {
        self.tiers.get(&tier)
    }

    /// Set a tier mapping
    pub fn set(&mut self, tier: u32, endpoint: &str, model: &str, description: &str) {
        self.tiers.insert(tier, TierMapping {
            endpoint: endpoint.into(),
            model: model.into(),
            description: description.into(),
        });
    }

    /// Remove a tier
    pub fn remove(&mut self, tier: u32) -> Option<TierMapping> {
        self.tiers.remove(&tier)
    }

    /// Get all tier numbers sorted
    pub fn tier_numbers(&self) -> Vec<u32> {
        let mut nums: Vec<_> = self.tiers.keys().copied().collect();
        nums.sort();
        nums
    }

    /// Generate system prompt explaining available tiers to the model
    pub fn system_prompt_section(&self) -> String {
        let mut prompt = String::from(
            "\n## Mini-Me Sub-Agents\n\
            You can spawn sub-agent Mini-Me's to handle subtasks. Use these commands:\n\n"
        );

        for tier in self.tier_numbers() {
            if let Some(mapping) = self.tiers.get(&tier) {
                prompt.push_str(&format!(
                    "- `/{}: <task>` - {} ({})\n",
                    tier, mapping.description, mapping.model
                ));
            }
        }

        if let Some(vision) = &self.vision {
            prompt.push_str(&format!(
                "- `/vision: <task>` - {} ({})\n",
                vision.description, vision.model
            ));
        }

        prompt.push_str(
            "\nUse lower tiers for simple tasks (search, summarize) and higher tiers for complex reasoning.\n\
            Mini-Me agents receive focused context and return summaries, not full transcripts.\n"
        );

        prompt
    }
}

impl Default for TierConfig {
    fn default() -> Self {
        let mut tiers = HashMap::new();
        tiers.insert(1, TierMapping {
            endpoint: "openrouter".into(),
            model: "anthropic/claude-haiku-3-5".into(),
            description: "Fast & cheap (Haiku)".into(),
        });
        tiers.insert(2, TierMapping {
            endpoint: "openrouter".into(),
            model: "anthropic/claude-sonnet-4".into(),
            description: "Balanced (Sonnet)".into(),
        });
        tiers.insert(3, TierMapping {
            endpoint: "openrouter".into(),
            model: "anthropic/claude-opus-4".into(),
            description: "Premium (Opus)".into(),
        });
        Self {
            tiers,
            vision: Some(TierMapping {
                endpoint: "openrouter".into(),
                model: "anthropic/claude-sonnet-4".into(),
                description: "Vision model".into(),
            }),
        }
    }
}

/// Provider configuration for Orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub endpoint: String,
    pub model: String,
    pub tier: ModelTier,
    pub api_key: Option<String>,
    pub max_concurrent: usize,
    pub cost_per_1k_tokens: f64,
}

impl ProviderConfig {
    pub fn lm_studio_beast() -> Self {
        Self {
            name: "beast".into(),
            endpoint: "http://192.168.245.155:1234".into(),
            model: "gpt-oss-20b".into(),
            tier: ModelTier::Capable,
            api_key: None,
            max_concurrent: 1,
            cost_per_1k_tokens: 0.0,
        }
    }

    pub fn lm_studio_bedroom() -> Self {
        Self {
            name: "bedroom".into(),
            endpoint: "http://192.168.27.182:1234".into(),
            model: "ministral-3-3b".into(),
            tier: ModelTier::Fast,
            api_key: None,
            max_concurrent: 2,
            cost_per_1k_tokens: 0.0,
        }
    }

    pub fn bedroom_vision() -> Self {
        Self {
            name: "bedroom-vision".into(),
            endpoint: "http://192.168.27.182:1234".into(),
            model: "ministral-3-3b".into(),
            tier: ModelTier::Vision,
            api_key: None,
            max_concurrent: 1,
            cost_per_1k_tokens: 0.0,
        }
    }

    pub fn anthropic_sonnet() -> Self {
        Self {
            name: "anthropic-sonnet".into(),
            endpoint: "https://api.anthropic.com".into(),
            model: "claude-sonnet-4-5-20250514".into(),
            tier: ModelTier::Cloud,
            api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            max_concurrent: 5,
            cost_per_1k_tokens: 0.003,
        }
    }

    pub fn anthropic_opus() -> Self {
        Self {
            name: "anthropic-opus".into(),
            endpoint: "https://api.anthropic.com".into(),
            model: "claude-opus-4-20250514".into(),
            tier: ModelTier::Premium,
            api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            max_concurrent: 3,
            cost_per_1k_tokens: 0.015,
        }
    }

    pub fn openai_gpt4o() -> Self {
        Self {
            name: "openai-gpt4o".into(),
            endpoint: "https://api.openai.com".into(),
            model: "gpt-4o".into(),
            tier: ModelTier::Cloud,
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            max_concurrent: 5,
            cost_per_1k_tokens: 0.005,
        }
    }

    pub fn gemini_pro() -> Self {
        Self {
            name: "gemini-pro".into(),
            endpoint: "https://generativelanguage.googleapis.com".into(),
            model: "gemini-2.0-flash".into(),
            tier: ModelTier::Cloud,
            api_key: std::env::var("GOOGLE_API_KEY").ok(),
            max_concurrent: 5,
            cost_per_1k_tokens: 0.00035,
        }
    }
}

/// Provider configuration for ProviderManager
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

/// Full Ganesha configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaneshaConfig {
    pub providers: Vec<ProviderConfig>,
    pub endpoints: HashMap<String, ProviderEndpoint>,
    pub tiers: TierConfig,
    #[serde(default)]
    pub setup_complete: bool,
}

impl Default for GaneshaConfig {
    fn default() -> Self {
        Self {
            providers: vec![
                ProviderConfig::lm_studio_beast(),
                ProviderConfig::lm_studio_bedroom(),
                ProviderConfig::bedroom_vision(),
                ProviderConfig::anthropic_sonnet(),
                ProviderConfig::anthropic_opus(),
                ProviderConfig::openai_gpt4o(),
                ProviderConfig::gemini_pro(),
            ],
            endpoints: HashMap::new(),
            tiers: TierConfig::default(),
            setup_complete: false,
        }
    }
}

pub struct ConfigManager {
    path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ganesha")
            .join("config.toml");
        Self { path }
    }

    pub fn load(&self) -> GaneshaConfig {
        if self.path.exists() {
            if let Ok(content) = fs::read_to_string(&self.path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        GaneshaConfig::default()
    }

    pub fn save(&self, config: &GaneshaConfig) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData,
                format!("Failed to serialize config to TOML: {}", e)))?;
        fs::write(&self.path, content)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlashCommand {
    Tier(u32),
    Vision,
}

/// Parse a slash command from user input
pub fn parse_slash_command(input: &str) -> Option<(SlashCommand, String)> {
    let input = input.trim();

    if !input.starts_with('/') {
        return None;
    }

    // Find the colon
    let colon_pos = input.find(':')?;
    let command = &input[1..colon_pos].trim();
    let prompt = input[colon_pos + 1..].trim().to_string();

    if prompt.is_empty() {
        return None;
    }

    if command.eq_ignore_ascii_case("vision") {
        Some((SlashCommand::Vision, prompt))
    } else if let Ok(tier) = command.parse::<u32>() {
        Some((SlashCommand::Tier(tier), prompt))
    } else {
        None
    }
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

pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
}
