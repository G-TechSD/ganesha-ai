//! Provider System with OAuth2 and Dynamic Model Discovery
//!
//! Supports:
//! - Local: LM Studio, Ollama
//! - Cloud: OpenAI (GPT-5.2), Anthropic (Opus 4.5), Google (Gemini 3)
//! - Aggregator: OpenRouter (access to many providers with one key)
//!
//! Authentication:
//! - OAuth2 for interactive login
//! - API keys for automation/CI
//! - Token refresh and caching

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::core::config::{
    ModelTier, ProviderType, AuthMethod, TierMapping, TierConfig,
    ProviderEndpoint, SlashCommand, parse_slash_command, OAuth2Config, ConfigManager,
    TokenResponse, ModelInfo,
};

pub struct ProviderManager {
    pub endpoints: HashMap<String, ProviderEndpoint>,
    pub tiers: TierConfig,
    models_cache: Arc<RwLock<HashMap<ProviderType, Vec<ModelInfo>>>>,
    cache_expiry: Arc<RwLock<HashMap<ProviderType, Instant>>>,
    config_manager: ConfigManager,
    setup_complete: bool,
    client: reqwest::Client,
}

impl ProviderManager {
    pub fn new() -> Self {
        let config_manager = ConfigManager::new();
        let config = config_manager.load();

        Self {
            endpoints: config.endpoints,
            tiers: config.tiers,
            models_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_expiry: Arc::new(RwLock::new(HashMap::new())),
            config_manager,
            setup_complete: config.setup_complete,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// Get tier configuration for system prompt
    pub fn get_tier_system_prompt(&self) -> String {
        self.tiers.system_prompt_section()
    }

    /// Execute a slash command, returns (endpoint_name, model, prompt)
    pub fn resolve_slash_command(&self, input: &str) -> Option<(String, String, String)> {
        let (cmd, prompt) = parse_slash_command(input)?;

        match cmd {
            SlashCommand::Tier(n) => {
                let mapping = self.tiers.get(n)?;
                Some((mapping.endpoint.clone(), mapping.model.clone(), prompt))
            }
            SlashCommand::Vision => {
                let mapping = self.tiers.vision.as_ref()?;
                Some((mapping.endpoint.clone(), mapping.model.clone(), prompt))
            }
        }
    }

    fn default_endpoints() -> HashMap<String, ProviderEndpoint> {
        let mut endpoints = HashMap::new();

        // LM Studio - localhost (user can add remote servers via settings)
        endpoints.insert("lmstudio".into(), ProviderEndpoint {
            provider_type: ProviderType::LmStudio,
            name: "LM Studio (Local)".into(),
            base_url: "http://localhost:1234".into(),
            auth: AuthMethod::None,
            default_model: "default".into(),
            enabled: true,
            priority: 1,
        });

        // OpenRouter - Aggregator (most users have this before local setup)
        if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
            endpoints.insert("openrouter".into(), ProviderEndpoint {
                provider_type: ProviderType::OpenRouter,
                name: "OpenRouter".into(),
                base_url: "https://openrouter.ai/api".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "anthropic/claude-sonnet-4".into(),
                enabled: true,
                priority: 4,
            });
        }

        // Ollama (local, but requires setup)
        endpoints.insert("ollama".into(), ProviderEndpoint {
            provider_type: ProviderType::Ollama,
            name: "Ollama".into(),
            base_url: "http://localhost:11434".into(),
            auth: AuthMethod::None,
            default_model: "llama3.3".into(),
            enabled: true,
            priority: 5,
        });

        // Direct cloud providers (premium, use when specified or escalating)
        // OpenAI - GPT-5.2
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            endpoints.insert("openai".into(), ProviderEndpoint {
                provider_type: ProviderType::OpenAI,
                name: "OpenAI".into(),
                base_url: "https://api.openai.com".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "gpt-5.2".into(),
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
                default_model: "claude-opus-4-5-20251101".into(),
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
                default_model: "gemini-3-pro".into(),
                enabled: true,
                priority: 12,
            });
        }

        endpoints
    }

    /// Check if first-run setup is needed
    pub fn needs_setup(&self) -> bool {
        !self.setup_complete
    }

    /// Interactive first-run setup with Local AI vs Cloud Providers paths
    pub async fn first_run_setup(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1;36m╭─────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[1;36m│           WELCOME TO GANESHA            │\x1b[0m");
        println!("\x1b[1;36m│      The Remover of Obstacles           │\x1b[0m");
        println!("\x1b[1;36m╰─────────────────────────────────────────╯\x1b[0m\n");

        println!("How would you like to use Ganesha?\n");
        println!("  \x1b[1m[1]\x1b[0m \x1b[32mLocal AI\x1b[0m - Run models on your own hardware");
        println!("      LM Studio, Ollama, or any OpenAI-compatible server");
        println!("      Free, private, no API keys needed\n");
        println!("  \x1b[1m[2]\x1b[0m \x1b[34mCloud Providers\x1b[0m - Use cloud AI services");
        println!("      OpenRouter, OpenAI, Anthropic, Google");
        println!("      Easy OAuth2 sign-in or API keys\n");
        println!("  \x1b[1m[3]\x1b[0m \x1b[33mBoth\x1b[0m - Local for fast tasks, cloud for heavy lifting\n");

        print!("Choose [1/2/3]: ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;

        match choice.trim() {
            "1" => self.setup_local_ai().await?,
            "2" => self.setup_cloud_providers().await?,
            "3" | "" => {
                self.setup_local_ai().await?;
                self.setup_cloud_providers().await?;
            }
            _ => {
                println!("\x1b[33mInvalid choice, defaulting to Both\x1b[0m");
                self.setup_local_ai().await?;
                self.setup_cloud_providers().await?;
            }
        }

        // Configure tiers
        self.setup_tiers().await?;

        // Mark setup as complete and save
        self.setup_complete = true;
        self.save()?;

        println!("\n\x1b[32m╭─────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[32m│         Setup Complete!                 │\x1b[0m");
        println!("\x1b[32m╰─────────────────────────────────────────╯\x1b[0m\n");

        println!("You can now use Ganesha with these commands:");
        for tier in self.tiers.tier_numbers() {
            if let Some(m) = self.tiers.get(tier) {
                println!("  /{}:  {} - {}", tier, m.description, m.model);
            }
        }
        if let Some(v) = &self.tiers.vision {
            println!("  /vision:  {} - {}", v.description, v.model);
        }
        println!("\n  \x1b[2mReconfigure anytime: ganesha --configure\x1b[0m");

        Ok(())
    }

    /// Setup path for Local AI
    async fn setup_local_ai(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1;32m── Local AI Setup ──\x1b[0m\n");
        println!("Scanning for local AI servers...\n");

        let mut found_any = false;

        // Check localhost endpoints only - no hardcoded network addresses
        let local_checks = [
            ("localhost:1234", "LM Studio"),
            ("127.0.0.1:1234", "LM Studio"),
            ("localhost:11434", "Ollama"),
            ("127.0.0.1:11434", "Ollama"),
        ];

        for (addr, name) in &local_checks {
            let url = format!("http://{}", addr);
            let is_ollama = addr.contains("11434");
            let test_url = if is_ollama {
                format!("{}/api/tags", url)
            } else {
                format!("{}/v1/models", url)
            };

            if let Ok(resp) = self.client.get(&test_url).timeout(Duration::from_secs(2)).send().await {
                if resp.status().is_success() {
                    found_any = true;
                    println!("  \x1b[32m✓\x1b[0m {} found at {}", name, addr);

                    // Assign generic endpoint names
                    let (endpoint_name, priority) = if is_ollama {
                        ("ollama", 2)
                    } else {
                        ("local", 1)  // LM Studio on localhost
                    };

                    self.endpoints.insert(endpoint_name.into(), ProviderEndpoint {
                        provider_type: if is_ollama { ProviderType::Ollama } else { ProviderType::LmStudio },
                        name: name.to_string(),
                        base_url: url,
                        auth: AuthMethod::None,
                        default_model: "default".into(),
                        enabled: true,
                        priority,
                    });
                }
            }
        }

        if !found_any {
            println!("  \x1b[33m⚠ No local servers detected\x1b[0m\n");
            println!("Would you like to add a custom OpenAI-compatible endpoint?");
            println!("(e.g., LM Studio running on another machine)\n");

            print!("Add custom endpoint? [y/N]: ");
            io::stdout().flush()?;

            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;

            if answer.trim().to_lowercase() == "y" {
                self.add_custom_endpoint().await?;
            }
        }

        Ok(())
    }

    /// Add a custom OpenAI-compatible endpoint
    async fn add_custom_endpoint(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1mAdd Custom Endpoint\x1b[0m\n");

        print!("Name (e.g., 'gpu-server'): ");
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        let name = name.trim();

        print!("URL (e.g., 'http://192.168.1.100:1234'): ");
        io::stdout().flush()?;
        let mut url = String::new();
        io::stdin().read_line(&mut url)?;
        let url = url.trim();

        print!("API Key (leave empty if none): ");
        io::stdout().flush()?;
        let mut api_key = String::new();
        io::stdin().read_line(&mut api_key)?;
        let api_key = api_key.trim();

        let auth = if api_key.is_empty() {
            AuthMethod::None
        } else {
            AuthMethod::ApiKey(api_key.to_string())
        };

        // Test the endpoint
        print!("Testing connection... ");
        io::stdout().flush()?;

        let test_url = format!("{}/v1/models", url);
        let mut req = self.client.get(&test_url).timeout(Duration::from_secs(5));
        if let AuthMethod::ApiKey(key) = &auth {
            req = req.bearer_auth(key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                println!("\x1b[32m✓ Connected!\x1b[0m");

                self.endpoints.insert(name.to_string(), ProviderEndpoint {
                    provider_type: ProviderType::LmStudio,
                    name: format!("Custom: {}", name),
                    base_url: url.to_string(),
                    auth,
                    default_model: "default".into(),
                    enabled: true,
                    priority: 1,
                });
            }
            _ => {
                println!("\x1b[31m✗ Connection failed\x1b[0m");
                println!("  The endpoint will be saved but may not work until the server is running.");

                self.endpoints.insert(name.to_string(), ProviderEndpoint {
                    provider_type: ProviderType::LmStudio,
                    name: format!("Custom: {}", name),
                    base_url: url.to_string(),
                    auth,
                    default_model: "default".into(),
                    enabled: true,
                    priority: 1,
                });
            }
        }

        Ok(())
    }

    /// Setup path for Cloud Providers
    async fn setup_cloud_providers(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1;34m── Cloud Provider Setup ──\x1b[0m\n");
        println!("Available providers:\n");
        println!("  \x1b[1m[1]\x1b[0m OpenRouter - Access many models with one API key (recommended)");
        println!("  \x1b[1m[2]\x1b[0m Google     - Sign in with Google (OAuth2)");
        println!("  \x1b[1m[3]\x1b[0m Anthropic  - Claude models (API key)");
        println!("  \x1b[1m[4]\x1b[0m OpenAI     - GPT models (API key)");
        println!("  \x1b[1m[S]\x1b[0m Skip cloud setup\n");

        print!("Select providers (e.g., '1,2' or 'all'): ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        let choice = choice.trim().to_lowercase();

        if choice == "s" || choice == "skip" {
            return Ok(());
        }

        let choices: Vec<&str> = if choice == "all" {
            vec!["1", "2", "3", "4"]
        } else {
            choice.split(',').map(|s| s.trim()).collect()
        };

        for c in choices {
            match c {
                "1" => self.setup_openrouter().await?,
                "2" => self.setup_google_oauth().await?,
                "3" => self.setup_anthropic().await?,
                "4" => self.setup_openai().await?,
                _ => {}
            }
        }

        Ok(())
    }

    /// Setup OpenRouter
    async fn setup_openrouter(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1mOpenRouter Setup\x1b[0m");
        println!("Get your API key at: https://openrouter.ai/keys\n");

        // Check env var first
        if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
            println!("  \x1b[32m✓\x1b[0m Found OPENROUTER_API_KEY in environment");
            self.endpoints.insert("openrouter".into(), ProviderEndpoint {
                provider_type: ProviderType::OpenRouter,
                name: "OpenRouter".into(),
                base_url: "https://openrouter.ai/api".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "anthropic/claude-sonnet-4".into(),
                enabled: true,
                priority: 2,
            });
            return Ok(());
        }

        print!("Enter OpenRouter API key: ");
        io::stdout().flush()?;

        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim();

        if key.is_empty() {
            println!("  \x1b[33mSkipped\x1b[0m");
            return Ok(());
        }

        // Test the key
        print!("  Verifying... ");
        io::stdout().flush()?;

        let resp = self.client
            .get("https://openrouter.ai/api/v1/models")
            .bearer_auth(key)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                println!("\x1b[32m✓\x1b[0m");
                self.endpoints.insert("openrouter".into(), ProviderEndpoint {
                    provider_type: ProviderType::OpenRouter,
                    name: "OpenRouter".into(),
                    base_url: "https://openrouter.ai/api".into(),
                    auth: AuthMethod::ApiKey(key.to_string()),
                    default_model: "anthropic/claude-sonnet-4".into(),
                    enabled: true,
                    priority: 2,
                });
            }
            _ => {
                println!("\x1b[31m✗ Invalid key\x1b[0m");
            }
        }

        Ok(())
    }

    /// Setup Google with OAuth2
    async fn setup_google_oauth(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("\n\x1b[1mGoogle AI Setup\x1b[0m");

        // Check for existing API key
        if let Ok(key) = std::env::var("GOOGLE_API_KEY") {
            println!("  \x1b[32m✓\x1b[0m Found GOOGLE_API_KEY in environment");
            self.endpoints.insert("google".into(), ProviderEndpoint {
                provider_type: ProviderType::Google,
                name: "Google AI".into(),
                base_url: "https://generativelanguage.googleapis.com".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "gemini-2.0-flash".into(),
                enabled: true,
                priority: 5,
            });
            return Ok(());
        }

        println!("Starting OAuth2 sign-in with Google...");
        self.oauth2_login(ProviderType::Google).await?;

        Ok(())
    }

    /// Setup Anthropic with API key
    async fn setup_anthropic(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1mAnthropic Setup\x1b[0m");
        println!("Get your API key at: https://console.anthropic.com/\n");

        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            println!("  \x1b[32m✓\x1b[0m Found ANTHROPIC_API_KEY in environment");
            self.endpoints.insert("anthropic".into(), ProviderEndpoint {
                provider_type: ProviderType::Anthropic,
                name: "Anthropic".into(),
                base_url: "https://api.anthropic.com".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "claude-sonnet-4-5-20250514".into(),
                enabled: true,
                priority: 10,
            });
            return Ok(());
        }

        print!("Enter Anthropic API key: ");
        io::stdout().flush()?;

        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim();

        if !key.is_empty() {
            self.endpoints.insert("anthropic".into(), ProviderEndpoint {
                provider_type: ProviderType::Anthropic,
                name: "Anthropic".into(),
                base_url: "https://api.anthropic.com".into(),
                auth: AuthMethod::ApiKey(key.to_string()),
                default_model: "claude-sonnet-4-5-20250514".into(),
                enabled: true,
                priority: 10,
            });
            println!("  \x1b[32m✓\x1b[0m Added Anthropic");
        } else {
            println!("  \x1b[33mSkipped\x1b[0m");
        }

        Ok(())
    }

    /// Setup OpenAI with API key
    async fn setup_openai(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1mOpenAI Setup\x1b[0m");
        println!("Get your API key at: https://platform.openai.com/api-keys\n");

        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            println!("  \x1b[32m✓\x1b[0m Found OPENAI_API_KEY in environment");
            self.endpoints.insert("openai".into(), ProviderEndpoint {
                provider_type: ProviderType::OpenAI,
                name: "OpenAI".into(),
                base_url: "https://api.openai.com".into(),
                auth: AuthMethod::ApiKey(key),
                default_model: "gpt-4o".into(),
                enabled: true,
                priority: 10,
            });
            return Ok(());
        }

        print!("Enter OpenAI API key: ");
        io::stdout().flush()?;

        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim();

        if !key.is_empty() {
            self.endpoints.insert("openai".into(), ProviderEndpoint {
                provider_type: ProviderType::OpenAI,
                name: "OpenAI".into(),
                base_url: "https://api.openai.com".into(),
                auth: AuthMethod::ApiKey(key.to_string()),
                default_model: "gpt-4o".into(),
                enabled: true,
                priority: 10,
            });
            println!("  \x1b[32m✓\x1b[0m Added OpenAI");
        } else {
            println!("  \x1b[33mSkipped\x1b[0m");
        }

        Ok(())
    }

    /// Setup model tiers
    async fn setup_tiers(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1;33m── Model Tiers ──\x1b[0m\n");
        println!("Ganesha uses numbered tiers for different tasks:");
        println!("  /1: prompt  - Fast tier (quick tasks)");
        println!("  /2: prompt  - Balanced tier (default)");
        println!("  /3: prompt  - Premium tier (complex reasoning)");
        println!("  /vision: prompt - Vision tasks\n");

        // Auto-configure based on available endpoints
        let has_openrouter = self.endpoints.contains_key("openrouter");
        let has_local = self.endpoints.contains_key("local")
            || self.endpoints.contains_key("ollama");
        let has_anthropic = self.endpoints.contains_key("anthropic");

        if has_openrouter {
            println!("Configuring tiers using OpenRouter...");

            self.tiers.set(1, "openrouter", "anthropic/claude-haiku-3-5", "Fast (Haiku)");
            self.tiers.set(2, "openrouter", "anthropic/claude-sonnet-4", "Balanced (Sonnet)");
            self.tiers.set(3, "openrouter", "anthropic/claude-opus-4", "Premium (Opus)");
            self.tiers.vision = Some(TierMapping {
                endpoint: "openrouter".into(),
                model: "anthropic/claude-sonnet-4".into(),
                description: "Vision (Sonnet)".into(),
            });
        } else if has_anthropic {
            println!("Configuring tiers using Anthropic...");

            self.tiers.set(1, "anthropic", "claude-haiku-3-5-20241022", "Fast (Haiku)");
            self.tiers.set(2, "anthropic", "claude-sonnet-4-5-20250514", "Balanced (Sonnet)");
            self.tiers.set(3, "anthropic", "claude-opus-4-20250514", "Premium (Opus)");
            self.tiers.vision = Some(TierMapping {
                endpoint: "anthropic".into(),
                model: "claude-sonnet-4-5-20250514".into(),
                description: "Vision (Sonnet)".into(),
            });
        } else if has_local {
            // Prefer local LM Studio > Ollama
            let local_name = if self.endpoints.contains_key("local") {
                "local"
            } else {
                "ollama"
            };
            println!("Configuring tiers using local models ({})...", local_name);

            self.tiers.set(1, local_name, "default", "Local model");
            self.tiers.set(2, local_name, "default", "Local model");
            self.tiers.tiers.remove(&3); // Remove premium tier if only local

            self.tiers.vision = None;
        }

        print!("\nCustomize tiers? [y/N]: ");
        io::stdout().flush()?;

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;

        if answer.trim().to_lowercase() == "y" {
            self.configure_tiers_interactive().await?;
        } else {
            println!("\n  Using default tier configuration.");
        }

        Ok(())
    }

    /// Interactive tier configuration
    async fn configure_tiers_interactive(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1mTier Configuration\x1b[0m");
        println!("Commands:");
        println!("  set <tier> <endpoint> <model>  - Configure a tier");
        println!("  remove <tier>                   - Remove a tier");
        println!("  vision <endpoint> <model>       - Set vision model");
        println!("  list                            - Show current tiers");
        println!("  done                            - Finish configuration\n");

        loop {
            print!("\x1b[36mtiers>\x1b[0m ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let parts: Vec<&str> = input.split_whitespace().collect();

            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "set" if parts.len() >= 4 => {
                    if let Ok(tier) = parts[1].parse::<u32>() {
                        self.tiers.set(tier, parts[2], parts[3], &format!("Tier {}", tier));
                        println!("  Set tier {} to {}/{}", tier, parts[2], parts[3]);
                    }
                }
                "remove" if parts.len() >= 2 => {
                    if let Ok(tier) = parts[1].parse::<u32>() {
                        self.tiers.remove(tier);
                        println!("  Removed tier {}", tier);
                    }
                }
                "vision" if parts.len() >= 3 => {
                    self.tiers.vision = Some(TierMapping {
                        endpoint: parts[1].into(),
                        model: parts[2].into(),
                        description: "Vision".into(),
                    });
                    println!("  Set vision to {}/{}", parts[1], parts[2]);
                }
                "list" => {
                    println!("\n  Current tiers:");
                    for tier in self.tiers.tier_numbers() {
                        if let Some(m) = self.tiers.get(tier) {
                            println!("    /{}: {} -> {}", tier, m.endpoint, m.model);
                        }
                    }
                    if let Some(v) = &self.tiers.vision {
                        println!("    /vision: {} -> {}", v.endpoint, v.model);
                    }
                    println!();
                }
                "done" | "exit" | "q" => break,
                _ => println!("  Unknown command"),
            }
        }

        Ok(())
    }

    /// Interactive provider configuration
    pub async fn configure_interactive(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        println!("\n\x1b[1;36mProvider Configuration\x1b[0m\n");

        // Show current providers
        println!("\x1b[1mCurrent providers (by priority):\x1b[0m\n");

        let mut providers: Vec<_> = self.endpoints.iter_mut().collect();
        providers.sort_by_key(|(_, e)| e.priority);

        for (i, (name, endpoint)) in providers.iter().enumerate() {
            let status = if endpoint.enabled { "\x1b[32m●\x1b[0m" } else { "\x1b[31m○\x1b[0m" };
            println!("  {} {}. {} - {} (priority {})",
                status, i + 1, name, endpoint.name, endpoint.priority);
        }

        println!("\n\x1b[1mCommands:\x1b[0m");
        println!("  priority <name> <num>  - Set provider priority (lower = preferred)");
        println!("  enable <name>          - Enable a provider");
        println!("  disable <name>         - Disable a provider");
        println!("  add <name> <url>       - Add custom LM Studio/Ollama endpoint");
        println!("  test                   - Test all providers");
        println!("  save                   - Save and exit");
        println!("  quit                   - Exit without saving\n");

        loop {
            print!("\x1b[36mganesha providers>\x1b[0m ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let parts: Vec<&str> = input.split_whitespace().collect();

            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "priority" if parts.len() >= 3 => {
                    let name = parts[1];
                    if let Ok(priority) = parts[2].parse::<u32>() {
                        if let Some(endpoint) = self.endpoints.get_mut(name) {
                            endpoint.priority = priority;
                            println!("  Set {} priority to {}", name, priority);
                        } else {
                            println!("  \x1b[31mProvider '{}' not found\x1b[0m", name);
                        }
                    }
                }
                "enable" if parts.len() >= 2 => {
                    let name = parts[1];
                    if let Some(endpoint) = self.endpoints.get_mut(name) {
                        endpoint.enabled = true;
                        println!("  Enabled {}", name);
                    } else {
                        println!("  \x1b[31mProvider '{}' not found\x1b[0m", name);
                    }
                }
                "disable" if parts.len() >= 2 => {
                    let name = parts[1];
                    if let Some(endpoint) = self.endpoints.get_mut(name) {
                        endpoint.enabled = false;
                        println!("  Disabled {}", name);
                    } else {
                        println!("  \x1b[31mProvider '{}' not found\x1b[0m", name);
                    }
                }
                "add" if parts.len() >= 3 => {
                    let name = parts[1];
                    let url = parts[2];
                    let provider_type = if url.contains("11434") {
                        ProviderType::Ollama
                    } else {
                        ProviderType::LmStudio
                    };
                    let max_priority = self.endpoints.values()
                        .map(|e| e.priority)
                        .max()
                        .unwrap_or(0);

                    self.endpoints.insert(name.to_string(), ProviderEndpoint {
                        provider_type,
                        name: format!("Custom: {}", name),
                        base_url: url.to_string(),
                        auth: AuthMethod::None,
                        default_model: "default".into(),
                        enabled: true,
                        priority: max_priority + 1,
                    });
                    println!("  Added {} at {}", name, url);
                }
                "test" => {
                    println!("\n  Testing providers...\n");
                    for name in self.endpoints.keys() {
                        let online = self.check_endpoint(name).await;
                        let status = if online { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
                        println!("  {} {}", status, name);
                    }
                    println!();
                }
                "save" => {
                    self.save()?;
                    println!("  \x1b[32m✓ Configuration saved\x1b[0m");
                    break;
                }
                "quit" | "exit" | "q" => {
                    println!("  Exiting without saving");
                    break;
                }
                _ => {
                    println!("  \x1b[31mUnknown command. Try: priority, enable, disable, add, test, save, quit\x1b[0m");
                }
            }
        }

        Ok(())
    }

    /// Set provider priority programmatically
    pub fn set_priority(&mut self, name: &str, priority: u32) -> bool {
        if let Some(endpoint) = self.endpoints.get_mut(name) {
            endpoint.priority = priority;
            true
        } else {
            false
        }
    }

    /// Enable/disable a provider
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(endpoint) = self.endpoints.get_mut(name) {
            endpoint.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Save current configuration
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut config = self.config_manager.load();
        config.endpoints = self.endpoints.clone();
        config.tiers = self.tiers.clone();
        config.setup_complete = self.setup_complete;

        self.config_manager.save(&config)?;
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
            ProviderType::OpenRouter => self.fetch_openrouter_models().await?,
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
                id: "claude-sonnet-4-5-20250514".into(),
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

        for endpoint in self.endpoints.values() {
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

    async fn fetch_openrouter_models(&self) -> Result<Vec<ModelInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = self.endpoints.get("openrouter");
        let auth = match endpoint {
            Some(e) => &e.auth,
            None => return Ok(Self::default_openrouter_models()),
        };

        let mut req = self.client.get("https://openrouter.ai/api/v1/models");
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
                                let context = m["context_length"].as_u64().unwrap_or(8192) as u32;
                                let pricing = &m["pricing"];
                                let input_cost = pricing["prompt"].as_str()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0) * 1_000_000.0;
                                let output_cost = pricing["completion"].as_str()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0) * 1_000_000.0;

                                Some(ModelInfo {
                                    id: id.to_string(),
                                    name: m["name"].as_str().unwrap_or(id).to_string(),
                                    provider: ProviderType::OpenRouter,
                                    context_window: context,
                                    max_output: (context / 4).min(32768),
                                    supports_vision: id.contains("vision") || id.contains("gpt-4") || id.contains("claude") || id.contains("gemini"),
                                    supports_tools: true,
                                    input_cost_per_1m: input_cost,
                                    output_cost_per_1m: output_cost,
                                    tier: self.infer_openrouter_tier(id),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_else(Self::default_openrouter_models);
                Ok(models)
            }
            _ => Ok(Self::default_openrouter_models()),
        }
    }

    fn default_openrouter_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "anthropic/claude-opus-4".into(),
                name: "Claude Opus 4 (via OpenRouter)".into(),
                provider: ProviderType::OpenRouter,
                context_window: 200000,
                max_output: 32768,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 15.0,
                output_cost_per_1m: 75.0,
                tier: ModelTier::Premium,
            },
            ModelInfo {
                id: "anthropic/claude-sonnet-4".into(),
                name: "Claude Sonnet 4 (via OpenRouter)".into(),
                provider: ProviderType::OpenRouter,
                context_window: 200000,
                max_output: 16384,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 3.0,
                output_cost_per_1m: 15.0,
                tier: ModelTier::Capable,
            },
            ModelInfo {
                id: "openai/gpt-4o".into(),
                name: "GPT-4o (via OpenRouter)".into(),
                provider: ProviderType::OpenRouter,
                context_window: 128000,
                max_output: 16384,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 2.5,
                output_cost_per_1m: 10.0,
                tier: ModelTier::Capable,
            },
            ModelInfo {
                id: "google/gemini-2.0-flash-exp".into(),
                name: "Gemini 2.0 Flash (via OpenRouter)".into(),
                provider: ProviderType::OpenRouter,
                context_window: 1000000,
                max_output: 8192,
                supports_vision: true,
                supports_tools: true,
                input_cost_per_1m: 0.0,
                output_cost_per_1m: 0.0,
                tier: ModelTier::Fast,
            },
            ModelInfo {
                id: "meta-llama/llama-3.3-70b-instruct".into(),
                name: "Llama 3.3 70B (via OpenRouter)".into(),
                provider: ProviderType::OpenRouter,
                context_window: 131072,
                max_output: 8192,
                supports_vision: false,
                supports_tools: true,
                input_cost_per_1m: 0.4,
                output_cost_per_1m: 0.4,
                tier: ModelTier::Capable,
            },
            ModelInfo {
                id: "qwen/qwen-2.5-72b-instruct".into(),
                name: "Qwen 2.5 72B (via OpenRouter)".into(),
                provider: ProviderType::OpenRouter,
                context_window: 131072,
                max_output: 8192,
                supports_vision: false,
                supports_tools: true,
                input_cost_per_1m: 0.35,
                output_cost_per_1m: 0.4,
                tier: ModelTier::Capable,
            },
            ModelInfo {
                id: "deepseek/deepseek-chat".into(),
                name: "DeepSeek V3 (via OpenRouter)".into(),
                provider: ProviderType::OpenRouter,
                context_window: 64000,
                max_output: 8192,
                supports_vision: false,
                supports_tools: true,
                input_cost_per_1m: 0.14,
                output_cost_per_1m: 0.28,
                tier: ModelTier::Standard,
            },
        ]
    }

    fn infer_openrouter_tier(&self, model_id: &str) -> ModelTier {
        if model_id.contains("opus") || model_id.contains("gpt-5") || model_id.contains("o3") {
            ModelTier::Premium
        } else if model_id.contains("sonnet") || model_id.contains("gpt-4") || model_id.contains("70b") || model_id.contains("72b") {
            ModelTier::Capable
        } else if model_id.contains("haiku") || model_id.contains("flash") || model_id.contains("mini") {
            ModelTier::Fast
        } else if model_id.contains("vision") || model_id.contains("llava") {
            ModelTier::Vision
        } else {
            ModelTier::Standard
        }
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
        
        if model_id.contains("5.2") {
            if is_input { 5.0 } else { 15.0 }
        } else if model_id.contains("o3") {
            if is_input { 1.1 } else { 4.4 }
        } else if model_id.contains("4o") {
            if is_input { 2.5 } else { 10.0 }
        } else if is_input { 0.5 } else { 1.5 }
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
            .next_back()
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



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_endpoints() {
        let endpoints = ProviderManager::default_endpoints();
        // Default endpoints should include local LM Studio and Ollama
        assert!(endpoints.contains_key("lmstudio"));
        assert!(endpoints.contains_key("ollama"));
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
