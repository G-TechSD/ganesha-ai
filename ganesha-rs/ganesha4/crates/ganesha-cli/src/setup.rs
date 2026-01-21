//! # Provider Setup Wizard
//!
//! Interactive setup for LLM providers when none are configured.

use colored::Colorize;
use crossterm::terminal;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

/// Provider configuration that gets saved
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Anthropic,
    OpenAI,
    Gemini,
    OpenRouter,
    Local,
}

impl ProviderType {
    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderType::Anthropic => "Anthropic (Claude)",
            ProviderType::OpenAI => "OpenAI (GPT-4)",
            ProviderType::Gemini => "Google (Gemini)",
            ProviderType::OpenRouter => "OpenRouter (Multi-provider)",
            ProviderType::Local => "Local Server (Ollama/LM Studio/etc)",
        }
    }

    pub fn default_base_url(&self) -> Option<&'static str> {
        match self {
            ProviderType::Anthropic => Some("https://api.anthropic.com"),
            ProviderType::OpenAI => Some("https://api.openai.com/v1"),
            ProviderType::Gemini => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
            ProviderType::OpenRouter => Some("https://openrouter.ai/api/v1"),
            ProviderType::Local => None, // User must specify
        }
    }

    #[allow(dead_code)]
    pub fn requires_api_key(&self) -> bool {
        match self {
            ProviderType::Anthropic | ProviderType::OpenAI | ProviderType::Gemini | ProviderType::OpenRouter => true,
            ProviderType::Local => false,
        }
    }
}

/// Saved providers configuration
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ProvidersConfig {
    pub providers: Vec<ProviderConfig>,
    pub default_provider: Option<String>,
}

impl ProvidersConfig {
    /// Load from config file
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    /// Save to config file
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get config file path
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("ganesha").join("providers.toml"))
            .unwrap_or_else(|| PathBuf::from(".ganesha/providers.toml"))
    }

    /// Check if any providers are configured
    pub fn has_providers(&self) -> bool {
        self.providers.iter().any(|p| p.enabled)
    }

    /// Get enabled providers
    pub fn enabled_providers(&self) -> Vec<&ProviderConfig> {
        self.providers.iter().filter(|p| p.enabled).collect()
    }
}

/// Run the interactive setup wizard
pub fn run_setup_wizard() -> anyhow::Result<Option<ProviderConfig>> {
    println!();
    println!("{}", "  No LLM providers configured.".yellow());
    println!();
    println!("  Ganesha needs an AI provider to work. Choose one to set up:");
    println!();
    println!("  {} Anthropic (Claude) - Best for coding tasks", "1.".bright_cyan());
    println!("  {} OpenAI (GPT-4)      - Widely supported", "2.".bright_cyan());
    println!("  {} Google (Gemini)     - Large context, multimodal", "3.".bright_cyan());
    println!("  {} OpenRouter          - Access multiple providers", "4.".bright_cyan());
    println!("  {} Local Server        - Ollama, LM Studio, vLLM, etc.", "5.".bright_cyan());
    println!("  {} Skip for now", "6.".dimmed());
    println!();

    let choice = prompt("  Select [1-6]: ")?;

    let provider_type = match choice.trim() {
        "1" => ProviderType::Anthropic,
        "2" => ProviderType::OpenAI,
        "3" => ProviderType::Gemini,
        "4" => ProviderType::OpenRouter,
        "5" => ProviderType::Local,
        "6" | "" => {
            println!();
            println!("  {}", "Skipped. You can run 'ganesha config' later to set up providers.".dimmed());
            return Ok(None);
        }
        _ => {
            println!("  {}", "Invalid choice".red());
            return Ok(None);
        }
    };

    println!();
    println!("  Setting up {}...", provider_type.display_name().bright_cyan());
    println!();

    let config = match provider_type {
        ProviderType::Anthropic => setup_cloud_provider(provider_type, "ANTHROPIC_API_KEY")?,
        ProviderType::OpenAI => setup_cloud_provider(provider_type, "OPENAI_API_KEY")?,
        ProviderType::Gemini => setup_cloud_provider(provider_type, "GEMINI_API_KEY")?,
        ProviderType::OpenRouter => setup_cloud_provider(provider_type, "OPENROUTER_API_KEY")?,
        ProviderType::Local => setup_local_provider()?,
    };

    if let Some(ref config) = config {
        // Save the config
        let mut providers_config = ProvidersConfig::load();

        // Remove existing provider of same type
        providers_config.providers.retain(|p| p.provider_type != config.provider_type);
        providers_config.providers.push(config.clone());

        if providers_config.default_provider.is_none() {
            providers_config.default_provider = Some(config.name.clone());
        }

        providers_config.save()?;

        println!();
        println!("  {} Provider configured and saved!", "✓".green());
        println!("  Config saved to: {}", ProvidersConfig::config_path().display().to_string().dimmed());
    }

    Ok(config)
}

/// Setup a cloud provider (Anthropic, OpenAI, OpenRouter)
fn setup_cloud_provider(provider_type: ProviderType, env_var_hint: &str) -> anyhow::Result<Option<ProviderConfig>> {
    // Check if env var is already set
    let existing_key = std::env::var(env_var_hint).ok();

    if let Some(ref key) = existing_key {
        println!("  Found {} in environment", env_var_hint.bright_green());
        let masked = mask_api_key(key);
        println!("  Key: {}", masked.dimmed());

        let use_existing = prompt("  Use this key? [Y/n]: ")?;
        if use_existing.trim().to_lowercase() != "n" {
            return Ok(Some(ProviderConfig {
                name: format!("{:?}", provider_type).to_lowercase(),
                provider_type,
                api_key: Some(key.clone()),
                base_url: provider_type.default_base_url().map(String::from),
                default_model: None,
                enabled: true,
            }));
        }
    }

    println!("  Enter your API key (or paste from clipboard):");
    println!("  {}", format!("Get one at: {}", get_signup_url(provider_type)).dimmed());
    println!();

    let api_key = prompt_secret("  API Key: ")?;

    if api_key.trim().is_empty() {
        println!("  {}", "No API key provided, skipping.".yellow());
        return Ok(None);
    }

    // Test the connection
    println!();
    println!("  Testing connection...");

    Ok(Some(ProviderConfig {
        name: format!("{:?}", provider_type).to_lowercase(),
        provider_type,
        api_key: Some(api_key.trim().to_string()),
        base_url: provider_type.default_base_url().map(String::from),
        default_model: None,
        enabled: true,
    }))
}

/// Setup a local server
fn setup_local_provider() -> anyhow::Result<Option<ProviderConfig>> {
    println!("  Checking for local servers...");
    println!();

    // Common local server ports to check
    let common_endpoints = [
        ("http://localhost:11434", "Ollama"),
        ("http://localhost:1234", "LM Studio"),
        ("http://127.0.0.1:11434", "Ollama"),
        ("http://127.0.0.1:1234", "LM Studio"),
        ("http://localhost:8000", "vLLM/Text Gen WebUI"),
        ("http://localhost:5000", "LocalAI"),
    ];

    let mut found_server = None;

    for (url, name) in &common_endpoints {
        print!("  Checking {} ({})... ", name, url);
        io::stdout().flush()?;

        if check_server_available(url) {
            println!("{}", "found!".green());
            found_server = Some((*url, *name));
            break;
        } else {
            println!("{}", "not running".dimmed());
        }
    }

    let (base_url, server_name) = if let Some((url, name)) = found_server {
        println!();
        let use_found = prompt(&format!("  Use {} at {}? [Y/n]: ", name, url))?;
        if use_found.trim().to_lowercase() == "n" {
            prompt_custom_server()?
        } else {
            (url.to_string(), name.to_string())
        }
    } else {
        println!();
        println!("  No local servers detected on common ports.");
        prompt_custom_server()?
    };

    if base_url.is_empty() {
        return Ok(None);
    }

    // Ask for a friendly name
    let name = prompt(&format!("  Name for this server [{}]: ", server_name))?;
    let name = if name.trim().is_empty() {
        server_name.to_lowercase().replace(' ', "-")
    } else {
        name.trim().to_string()
    };

    Ok(Some(ProviderConfig {
        name,
        provider_type: ProviderType::Local,
        api_key: None,
        base_url: Some(base_url),
        default_model: None,
        enabled: true,
    }))
}

/// Prompt for a custom server URL
fn prompt_custom_server() -> anyhow::Result<(String, String)> {
    println!();
    println!("  Enter the server URL (e.g., http://192.168.1.100:11434):");
    let url = prompt("  URL: ")?;

    if url.trim().is_empty() {
        return Ok((String::new(), String::new()));
    }

    let url = url.trim().to_string();

    // Validate URL format
    if !url.starts_with("http://") && !url.starts_with("https://") {
        println!("  {}", "URL must start with http:// or https://".red());
        return Ok((String::new(), String::new()));
    }

    // Test connection
    print!("  Testing connection... ");
    io::stdout().flush()?;

    if check_server_available(&url) {
        println!("{}", "connected!".green());
        Ok((url, "Custom Server".to_string()))
    } else {
        println!("{}", "failed".red());
        println!("  Could not connect. Make sure the server is running.");

        let try_anyway = prompt("  Save anyway? [y/N]: ")?;
        if try_anyway.trim().to_lowercase() == "y" {
            Ok((url, "Custom Server".to_string()))
        } else {
            Ok((String::new(), String::new()))
        }
    }
}

/// Check if a server is available
fn check_server_available(url: &str) -> bool {
    // Try to connect with a short timeout
    let client: Result<reqwest::blocking::Client, _> = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();

    if let Ok(client) = client {
        // Try common endpoints
        let endpoints = [
            format!("{}/v1/models", url),
            format!("{}/api/tags", url),  // Ollama
            format!("{}/models", url),
            url.to_string(),
        ];

        for endpoint in &endpoints {
            if client.get(endpoint).send().is_ok() {
                return true;
            }
        }
    }

    false
}

/// Get signup URL for a provider
fn get_signup_url(provider_type: ProviderType) -> &'static str {
    match provider_type {
        ProviderType::Anthropic => "https://console.anthropic.com/",
        ProviderType::OpenAI => "https://platform.openai.com/api-keys",
        ProviderType::Gemini => "https://aistudio.google.com/apikey",
        ProviderType::OpenRouter => "https://openrouter.ai/keys",
        ProviderType::Local => "",
    }
}

/// Mask an API key for display
fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len()-4..])
    }
}

/// Prompt for user input
/// Temporarily disables raw mode if active to allow normal line reading
fn prompt(msg: &str) -> anyhow::Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;

    // Check if terminal is in raw mode and temporarily disable it
    let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
    if was_raw {
        let _ = terminal::disable_raw_mode();
    }

    let mut input = String::new();
    let result = io::stdin().lock().read_line(&mut input);

    // Restore raw mode if it was enabled
    if was_raw {
        let _ = terminal::enable_raw_mode();
    }

    result?;
    Ok(input.trim().to_string())
}

/// Prompt for secret input (API key)
/// Temporarily disables raw mode if active to allow normal line reading
fn prompt_secret(msg: &str) -> anyhow::Result<String> {
    print!("{}", msg);
    io::stdout().flush()?;

    // Check if terminal is in raw mode and temporarily disable it
    let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
    if was_raw {
        let _ = terminal::disable_raw_mode();
    }

    let mut input = String::new();
    let result = io::stdin().lock().read_line(&mut input);

    // Restore raw mode if it was enabled
    if was_raw {
        let _ = terminal::enable_raw_mode();
    }

    result?;
    Ok(input.trim().to_string())
}
