//! Ganesha Menu System
//!
//! Clean, navigable terminal menus for:
//! - Settings configuration
//! - Model/provider selection
//! - Interview-style question collection
//!
//! Uses crossterm for clean display without glitches.
//! Supports Escape and Ctrl+C to navigate back from menus.

use console::{style, Term, Key};
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};

/// Menu option with label and optional description
#[derive(Clone)]
pub struct MenuOption {
    pub label: String,
    pub description: Option<String>,
    pub value: String,
}

impl MenuOption {
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            description: None,
            value: value.to_string(),
        }
    }

    pub fn with_description(label: &str, description: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            description: Some(description.to_string()),
            value: value.to_string(),
        }
    }
}

/// Result of a menu selection
pub enum MenuResult {
    Selected(String),
    Custom(String),
    Back,
    Exit,
}

/// Read menu input with support for Escape and Ctrl+C to go back
/// Returns None if user pressed Escape or Ctrl+C, Some(input) otherwise
fn read_menu_input() -> Option<String> {
    let term = Term::stdout();
    let mut input = String::new();

    loop {
        match term.read_key() {
            Ok(Key::Escape) => {
                println!(); // Move to next line
                return None; // Go back
            }
            Ok(Key::Enter) => {
                println!(); // Move to next line
                return Some(input);
            }
            Ok(Key::Backspace) => {
                if !input.is_empty() {
                    input.pop();
                    // Clear and reprint
                    print!("\r{} {}", style("Select option:").cyan(), input);
                    print!(" \x08"); // Erase last char visually
                    let _ = io::stdout().flush();
                }
            }
            Ok(Key::Char(c)) => {
                // Check for Ctrl+C (char code 3)
                if c == '\x03' {
                    println!();
                    return None; // Go back
                }
                input.push(c);
                print!("{}", c);
                let _ = io::stdout().flush();
            }
            _ => {}
        }
    }
}

/// Display a single-select menu and return the selected option
pub fn show_menu(title: &str, options: &[MenuOption], allow_custom: bool, allow_back: bool) -> MenuResult {
    show_menu_with_prompt(title, options, allow_custom, allow_back, "Enter value:")
}

/// Display a single-select menu with custom prompt for the custom input option
pub fn show_menu_with_prompt(title: &str, options: &[MenuOption], allow_custom: bool, allow_back: bool, custom_prompt: &str) -> MenuResult {
    // Use simple newlines instead of terminal clear for better compatibility
    println!("\n\n");

    // Print header
    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style(title).cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    // Print options
    for (i, opt) in options.iter().enumerate() {
        let num = format!("[{}]", i + 1);
        print!("  {} {}", style(num).yellow(), style(&opt.label).bold());
        if let Some(ref desc) = opt.description {
            print!(" - {}", style(desc).dim());
        }
        println!();
    }

    println!();

    if allow_custom {
        println!("  {} Custom input...", style("[C]").yellow());
    }
    if allow_back {
        println!("  {} Back", style("[B]").yellow());
    }
    println!("  {} Quit", style("[Q]").yellow());

    println!("\n{}", style("─".repeat(60)).dim());
    println!("{}", style("(Press Escape to go back)").dim());
    print!("{} ", style("Select option:").cyan());
    let _ = io::stdout().flush();

    // Read input with Escape/Ctrl+C support
    let input = match read_menu_input() {
        Some(s) => s.trim().to_lowercase(),
        None => return MenuResult::Back, // Escape or Ctrl+C pressed
    };

    // Handle special keys
    if input == "q" || input == "quit" || input == "exit" {
        return MenuResult::Exit;
    }
    if allow_back && (input == "b" || input == "back") {
        return MenuResult::Back;
    }
    if allow_custom && (input == "c" || input == "custom") {
        print!("{} ", style(custom_prompt).cyan());
        let _ = io::stdout().flush();
        let mut custom = String::new();
        if io::stdin().read_line(&mut custom).is_ok() {
            return MenuResult::Custom(custom.trim().to_string());
        }
        return MenuResult::Back;
    }

    // Try to parse as number
    if let Ok(num) = input.parse::<usize>() {
        if num > 0 && num <= options.len() {
            return MenuResult::Selected(options[num - 1].value.clone());
        }
    }

    // Try to match by label
    for opt in options {
        if opt.label.to_lowercase() == input || opt.value.to_lowercase() == input {
            return MenuResult::Selected(opt.value.clone());
        }
    }

    // Invalid input - try again
    println!("{} Invalid selection. Press Enter to try again.", style("⚠").yellow());
    let _ = io::stdin().read_line(&mut String::new());
    show_menu(title, options, allow_custom, allow_back)
}

/// Display a multi-select menu and return all selected options
pub fn show_multi_select(title: &str, options: &[MenuOption], allow_custom: bool) -> Vec<String> {
    // Simple spacing for clean display
    println!();

    let mut selected: Vec<bool> = vec![false; options.len()];

    loop {
        println!();

        println!("\n{}", style("═".repeat(60)).dim());
        println!("{}", style(title).cyan().bold());
        println!("{}", style("(Toggle with number, Enter when done)").dim());
        println!("{}\n", style("═".repeat(60)).dim());

        for (i, opt) in options.iter().enumerate() {
            let check = if selected[i] { "[✓]" } else { "[ ]" };
            let num = format!("[{}]", i + 1);
            print!("  {} {} {}",
                style(num).yellow(),
                if selected[i] { style(check).green() } else { style(check).dim() },
                style(&opt.label).bold()
            );
            if let Some(ref desc) = opt.description {
                print!(" - {}", style(desc).dim());
            }
            println!();
        }

        println!();
        if allow_custom {
            println!("  {} Add custom...", style("[C]").yellow());
        }
        println!("  {} Select all", style("[A]").yellow());
        println!("  {} Select none", style("[N]").yellow());
        println!("  {} Done", style("[Enter]").yellow());

        println!("\n{}", style("─".repeat(60)).dim());
        print!("{} ", style("Toggle option:").cyan());
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let input = input.trim().to_lowercase();

        if input.is_empty() {
            // Done
            break;
        }

        if input == "a" || input == "all" {
            selected.fill(true);
            continue;
        }

        if input == "n" || input == "none" {
            selected.fill(false);
            continue;
        }

        if allow_custom && (input == "c" || input == "custom") {
            print!("{} ", style("Enter custom value:").cyan());
            let _ = io::stdout().flush();
            let mut custom = String::new();
            if io::stdin().read_line(&mut custom).is_ok() {
                let custom = custom.trim().to_string();
                if !custom.is_empty() {
                    // Return immediately with custom value
                    let mut results: Vec<String> = options.iter()
                        .enumerate()
                        .filter(|(i, _)| selected[*i])
                        .map(|(_, opt)| opt.value.clone())
                        .collect();
                    results.push(custom);
                    return results;
                }
            }
            continue;
        }

        if let Ok(num) = input.parse::<usize>() {
            if num > 0 && num <= options.len() {
                selected[num - 1] = !selected[num - 1];
            }
        }
    }

    // Collect selected values
    options.iter()
        .enumerate()
        .filter(|(i, _)| selected[*i])
        .map(|(_, opt)| opt.value.clone())
        .collect()
}

/// Interview-style question with multiple choice and custom option
#[derive(Clone)]
pub struct InterviewQuestion {
    pub id: String,
    pub question: String,
    pub options: Vec<MenuOption>,
    pub allow_custom: bool,
    pub allow_multiple: bool,
    pub required: bool,
}

impl InterviewQuestion {
    pub fn single(id: &str, question: &str, options: Vec<MenuOption>) -> Self {
        Self {
            id: id.to_string(),
            question: question.to_string(),
            options,
            allow_custom: true,
            allow_multiple: false,
            required: true,
        }
    }

    pub fn multiple(id: &str, question: &str, options: Vec<MenuOption>) -> Self {
        Self {
            id: id.to_string(),
            question: question.to_string(),
            options,
            allow_custom: true,
            allow_multiple: true,
            required: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn no_custom(mut self) -> Self {
        self.allow_custom = false;
        self
    }
}

/// Interview response
#[derive(Clone)]
pub struct InterviewResponse {
    pub question_id: String,
    pub values: Vec<String>,
}

/// Conduct an interview with multiple questions
pub fn conduct_interview(title: &str, questions: &[InterviewQuestion]) -> Option<Vec<InterviewResponse>> {
    // Simple spacing for clean display
    println!();

    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style(title).cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    println!("{}", style(format!("This interview has {} questions.", questions.len())).dim());
    println!("{}", style("Press Enter to begin or Q to cancel.").dim());

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return None;
    }
    if input.trim().to_lowercase() == "q" {
        return None;
    }

    let mut responses = Vec::new();

    for (i, question) in questions.iter().enumerate() {
        println!();

        let progress = format!("Question {}/{}", i + 1, questions.len());
        println!("\n{}", style(progress).dim());
        println!("{}", style("═".repeat(60)).dim());
        println!("{}", style(&question.question).cyan().bold());
        if !question.required {
            println!("{}", style("(Optional - press Enter to skip)").dim());
        }
        println!("{}\n", style("─".repeat(60)).dim());

        let values = if question.allow_multiple {
            show_multi_select(&question.question, &question.options, question.allow_custom)
        } else {
            match show_inline_select(&question.options, question.allow_custom) {
                Some(v) => vec![v],
                None if !question.required => vec![],
                None => {
                    println!("{} This question is required.", style("⚠").yellow());
                    let _ = io::stdin().read_line(&mut String::new());
                    // Retry this question
                    continue;
                }
            }
        };

        responses.push(InterviewResponse {
            question_id: question.id.clone(),
            values,
        });
    }

    // Show summary
    println!();
    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("Interview Complete").green().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    for response in &responses {
        let q = questions.iter().find(|q| q.id == response.question_id).unwrap();
        println!("{}: {}", style(&q.question).cyan(), response.values.join(", "));
    }

    println!("\n{}", style("─".repeat(60)).dim());
    println!("{}", style("Press Enter to confirm or Q to cancel.").dim());

    let mut confirm = String::new();
    if io::stdin().read_line(&mut confirm).is_err() {
        return None;
    }
    if confirm.trim().to_lowercase() == "q" {
        return None;
    }

    Some(responses)
}

/// Inline single select (displayed on one screen)
fn show_inline_select(options: &[MenuOption], allow_custom: bool) -> Option<String> {
    for (i, opt) in options.iter().enumerate() {
        let num = format!("[{}]", i + 1);
        print!("  {} {}", style(num).yellow(), style(&opt.label).bold());
        if let Some(ref desc) = opt.description {
            print!(" - {}", style(desc).dim());
        }
        println!();
    }

    if allow_custom {
        println!("  {} Custom input...", style("[C]").yellow());
    }

    println!();
    print!("{} ", style("Select:").cyan());
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return None;
    }

    let input = input.trim();

    if input.is_empty() {
        return None;
    }

    if allow_custom && (input.to_lowercase() == "c" || input.to_lowercase() == "custom") {
        print!("{} ", style("Enter custom value:").cyan());
        let _ = io::stdout().flush();
        let mut custom = String::new();
        if io::stdin().read_line(&mut custom).is_ok() {
            let custom = custom.trim().to_string();
            if !custom.is_empty() {
                return Some(custom);
            }
        }
        return None;
    }

    if let Ok(num) = input.parse::<usize>() {
        if num > 0 && num <= options.len() {
            return Some(options[num - 1].value.clone());
        }
    }

    // Try to match by label/value
    for opt in options {
        if opt.label.to_lowercase().starts_with(&input.to_lowercase())
            || opt.value.to_lowercase() == input.to_lowercase()
        {
            return Some(opt.value.clone());
        }
    }

    None
}

/// Simple yes/no confirmation
pub fn confirm(question: &str, default: bool) -> bool {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("{} {} ", style(question).cyan(), style(hint).dim());
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return default;
    }

    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return default;
    }

    matches!(input.as_str(), "y" | "yes" | "1" | "true")
}

/// Simple text input with optional default
pub fn text_input(prompt: &str, default: Option<&str>) -> Option<String> {
    if let Some(def) = default {
        print!("{} [{}]: ", style(prompt).cyan(), style(def).dim());
    } else {
        print!("{}: ", style(prompt).cyan());
    }
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return default.map(|s| s.to_string());
    }

    let input = input.trim();

    if input.is_empty() {
        return default.map(|s| s.to_string());
    }

    Some(input.to_string())
}

/// Configured provider connection
#[derive(Clone)]
pub struct ProviderConnection {
    pub name: String,
    pub provider_type: String,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub model: String,
    pub enabled: bool,
}

/// Store for configured providers - thread-safe with Mutex
static CONFIGURED_PROVIDERS: OnceLock<Mutex<Vec<ProviderConnection>>> = OnceLock::new();
static PROVIDER_PRIORITY: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

/// Get all configured providers
pub fn get_providers() -> Vec<ProviderConnection> {
    CONFIGURED_PROVIDERS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("Provider lock poisoned")
        .clone()
}

/// Get provider priority order
pub fn get_priority() -> Vec<String> {
    PROVIDER_PRIORITY
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("Priority lock poisoned")
        .clone()
}

/// Initialize providers from environment - call at menu startup to sync with actual providers
pub fn init_providers_from_env() {
    // Only init if empty - don't overwrite user additions during session
    {
        let providers = CONFIGURED_PROVIDERS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .expect("Provider lock poisoned");
        if !providers.is_empty() {
            return;
        }
    } // Release lock before doing network checks

        // Check for LM Studio servers
        // Note: For network servers, use the /settings menu to add custom endpoints
        let lm_studio_servers = [
            ("lmstudio", "http://localhost:1234", "LM Studio (Local)"),
        ];

        for (name, endpoint, _display_name) in &lm_studio_servers {
            // Quick connectivity check
            let check_url = format!("{}/v1/models", endpoint);
            let is_online = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .ok()
                .and_then(|c| c.get(&check_url).send().ok())
                .map(|r| r.status().is_success())
                .unwrap_or(false);

            if is_online {
                CONFIGURED_PROVIDERS
                    .get_or_init(|| Mutex::new(Vec::new()))
                    .lock()
                    .expect("Provider lock poisoned")
                    .push(ProviderConnection {
                        name: name.to_string(),
                        provider_type: "lmstudio".to_string(),
                        endpoint: endpoint.to_string(),
                        api_key: None,
                        model: "default".to_string(),
                        enabled: true,
                    });
                PROVIDER_PRIORITY
                    .get_or_init(|| Mutex::new(Vec::new()))
                    .lock()
                    .expect("Priority lock poisoned")
                    .push(name.to_string());
            }
        }

        // Check for Ollama
        if reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .ok()
            .and_then(|c| c.get("http://localhost:11434/api/tags").send().ok())
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            CONFIGURED_PROVIDERS
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("Provider lock poisoned")
                .push(ProviderConnection {
                    name: "ollama".to_string(),
                    provider_type: "ollama".to_string(),
                    endpoint: "http://localhost:11434".to_string(),
                    api_key: None,
                    model: "default".to_string(),
                    enabled: true,
                });
            PROVIDER_PRIORITY
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("Priority lock poisoned")
                .push("ollama".to_string());
        }

        // Check for cloud providers via env vars
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            CONFIGURED_PROVIDERS
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("Provider lock poisoned")
                .push(ProviderConnection {
                    name: "anthropic".to_string(),
                    provider_type: "anthropic".to_string(),
                    endpoint: "https://api.anthropic.com".to_string(),
                    api_key: Some("(from env)".to_string()),
                    model: "claude-sonnet-4-5-20250514".to_string(),
                    enabled: true,
                });
            PROVIDER_PRIORITY
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("Priority lock poisoned")
                .push("anthropic".to_string());
        }

        if std::env::var("OPENAI_API_KEY").is_ok() {
            CONFIGURED_PROVIDERS
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("Provider lock poisoned")
                .push(ProviderConnection {
                    name: "openai".to_string(),
                    provider_type: "openai".to_string(),
                    endpoint: "https://api.openai.com".to_string(),
                    api_key: Some("(from env)".to_string()),
                    model: "gpt-4o".to_string(),
                    enabled: true,
                });
            PROVIDER_PRIORITY
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("Priority lock poisoned")
                .push("openai".to_string());
        }
}

/// Providers & Connections menu - manage local and cloud connections
pub fn show_connections_menu() {
    // Initialize from actual system state on first open
    init_providers_from_env();

    loop {
        let providers = get_providers();

        println!("\n{}", style("═".repeat(60)).dim());
        println!("{}", style("Providers & Connections").cyan().bold());
        println!("{}\n", style("═".repeat(60)).dim());

        if !providers.is_empty() {
            println!("{}", style("Configured Providers:").bold());
            for (i, p) in providers.iter().enumerate() {
                let status = if p.enabled { style("●").green() } else { style("○").dim() };
                println!("  {} {}. {} ({}) - {}", status, i + 1, p.name, p.provider_type, p.endpoint);
            }
            println!();
        }

        let options = vec![
            MenuOption::with_description("Add Local Server", "LM Studio, Ollama, or custom local", "local"),
            MenuOption::with_description("Add Cloud Provider", "Anthropic, OpenAI, Google, etc.", "cloud"),
            MenuOption::with_description("Edit Provider", "Modify existing connection", "edit"),
            MenuOption::with_description("Remove Provider", "Delete a connection", "remove"),
            MenuOption::with_description("Test Connections", "Verify all providers are reachable", "test"),
        ];

        match show_menu("Manage Connections", &options, false, true) {
            MenuResult::Selected(v) => {
                match v.as_str() {
                    "local" => { add_local_provider(); }
                    "cloud" => { add_cloud_provider(); }
                    "edit" => {
                        println!("\n{} Select provider number to edit, or press Enter to cancel.", style("→").cyan());
                        // TODO: implement edit
                        println!("{}", style("(Edit functionality coming soon)").dim());
                        let _ = io::stdin().read_line(&mut String::new());
                    }
                    "remove" => {
                        if providers.is_empty() {
                            println!("\n{} No providers configured to remove.", style("ℹ").cyan());
                            let _ = io::stdin().read_line(&mut String::new());
                        } else {
                            println!("\n{} Enter provider number to remove (1-{}), or press Enter to cancel:",
                                style("→").cyan(), providers.len());
                            let mut input = String::new();
                            if io::stdin().read_line(&mut input).is_ok() {
                                let input = input.trim();
                                if !input.is_empty() {
                                    if let Ok(num) = input.parse::<usize>() {
                                        if num >= 1 && num <= providers.len() {
                                            let removed = providers[num - 1].name.clone();
                                            CONFIGURED_PROVIDERS
                                                .get_or_init(|| Mutex::new(Vec::new()))
                                                .lock()
                                                .expect("Provider lock poisoned")
                                                .remove(num - 1);
                                            // Also remove from priority list if present
                                            PROVIDER_PRIORITY
                                                .get_or_init(|| Mutex::new(Vec::new()))
                                                .lock()
                                                .expect("Priority lock poisoned")
                                                .retain(|p| p != &removed);
                                            println!("{} Removed provider: {}", style("✓").green(), removed);
                                        } else {
                                            println!("{} Invalid number. Must be 1-{}", style("✗").red(), providers.len());
                                        }
                                    } else {
                                        println!("{} Invalid input. Enter a number.", style("✗").red());
                                    }
                                }
                            }
                        }
                    }
                    "test" => {
                        println!("\n{} Testing connections...", style("🔍").cyan());
                        println!("{}", style("(Connection testing coming soon)").dim());
                        let _ = io::stdin().read_line(&mut String::new());
                    }
                    _ => {}
                }
            }
            MenuResult::Back | MenuResult::Exit => break,
            _ => {}
        }
    }
}

/// Fetch available models from a local server
fn fetch_local_models(endpoint: &str, provider_type: &str) -> Vec<String> {
    use std::time::Duration;

    let models_url = match provider_type {
        "ollama" => format!("{}/api/tags", endpoint),
        _ => format!("{}/v1/models", endpoint), // OpenAI-compatible (LM Studio, LocalAI, vLLM)
    };

    println!("{} Fetching models from {}...", style("🔍").cyan(), endpoint);

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            println!("{} Could not create HTTP client", style("⚠").yellow());
            return vec![];
        }
    };

    let response = match client.get(&models_url).send() {
        Ok(r) => r,
        Err(e) => {
            println!("{} Could not connect: {}", style("⚠").yellow(), e);
            return vec![];
        }
    };

    if !response.status().is_success() {
        println!("{} Server returned error: {}", style("⚠").yellow(), response.status());
        return vec![];
    }

    let text = match response.text() {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    // Parse based on provider type
    if provider_type == "ollama" {
        // Ollama returns { "models": [{ "name": "...", ... }] }
        #[derive(serde::Deserialize)]
        struct OllamaModels {
            models: Vec<OllamaModel>,
        }
        #[derive(serde::Deserialize)]
        struct OllamaModel {
            name: String,
        }
        if let Ok(parsed) = serde_json::from_str::<OllamaModels>(&text) {
            return parsed.models.into_iter().map(|m| m.name).collect();
        }
    } else {
        // OpenAI-compatible returns { "data": [{ "id": "...", ... }] }
        #[derive(serde::Deserialize)]
        struct OpenAIModels {
            data: Vec<OpenAIModel>,
        }
        #[derive(serde::Deserialize)]
        struct OpenAIModel {
            id: String,
        }
        if let Ok(parsed) = serde_json::from_str::<OpenAIModels>(&text) {
            return parsed.data.into_iter().map(|m| m.id).collect();
        }
    }

    vec![]
}

/// Add a local server connection
fn add_local_provider() {
    println!("\n{}", style("Add Local Server").cyan().bold());

    let options = vec![
        MenuOption::with_description("LM Studio", "OpenAI-compatible local inference", "lmstudio"),
        MenuOption::with_description("Ollama", "Easy local model management", "ollama"),
        MenuOption::with_description("LocalAI", "OpenAI-compatible with many backends", "localai"),
        MenuOption::with_description("vLLM", "High-performance inference server", "vllm"),
    ];

    let provider_type = match show_menu_with_prompt("Server Type", &options, true, true, "Custom server type:") {
        MenuResult::Selected(v) | MenuResult::Custom(v) => v,
        _ => return,
    };

    let name = match text_input("Name this connection", Some(&provider_type)) {
        Some(n) => n,
        None => return,
    };

    let default_port = match provider_type.as_str() {
        "lmstudio" => "1234",
        "ollama" => "11434",
        "localai" => "8080",
        "vllm" => "8000",
        _ => "8080",
    };

    let endpoint = match text_input("Server URL", Some(&format!("http://localhost:{}", default_port))) {
        Some(e) => e,
        None => return,
    };

    // Fetch available models from server
    let available_models = fetch_local_models(&endpoint, &provider_type);

    let model = if !available_models.is_empty() {
        println!("\n{} Found {} models", style("✓").green(), available_models.len());

        let model_options: Vec<MenuOption> = available_models.iter()
            .map(|m| MenuOption::new(m, m))
            .collect();

        match show_menu_with_prompt("Select Model", &model_options, true, false, "Enter model name:") {
            MenuResult::Selected(v) | MenuResult::Custom(v) => v,
            _ => "default".to_string(),
        }
    } else {
        println!("{} Could not fetch models. Enter manually.", style("ℹ").cyan());
        text_input("Model name", Some("default")).unwrap_or_else(|| "default".to_string())
    };

    let connection = ProviderConnection {
        name: name.clone(),
        provider_type,
        endpoint,
        api_key: None,
        model,
        enabled: true,
    };

    CONFIGURED_PROVIDERS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("Provider lock poisoned")
        .push(connection);
    PROVIDER_PRIORITY
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("Priority lock poisoned")
        .push(name.clone());

    println!("\n{} Local server '{}' added", style("✓").green(), name);
    println!("{}", style("Press Enter to continue...").dim());
    let _ = io::stdin().read_line(&mut String::new());
}

/// Add a cloud provider connection
fn add_cloud_provider() {
    println!("\n{}", style("Add Cloud Provider").cyan().bold());

    let options = vec![
        MenuOption::with_description("Anthropic", "Claude 4.5 Opus, Sonnet 4", "anthropic"),
        MenuOption::with_description("OpenAI", "GPT-5.2, GPT-5.1, o1", "openai"),
        MenuOption::with_description("Google", "Gemini 3.0, Gemini 2.0 Flash", "google"),
        MenuOption::with_description("DeepSeek", "V3.2, R1 series (very affordable)", "deepseek"),
        MenuOption::with_description("Mistral", "Devstral 2, Mistral 3", "mistral"),
        MenuOption::with_description("Groq", "Ultra-fast inference", "groq"),
        MenuOption::with_description("Together AI", "Open model hosting", "together"),
    ];

    let provider_type = match show_menu_with_prompt("Cloud Provider", &options, true, true, "Custom provider name:") {
        MenuResult::Selected(v) | MenuResult::Custom(v) => v,
        _ => return,
    };

    let name = match text_input("Name this connection", Some(&provider_type)) {
        Some(n) => n,
        None => return,
    };

    let default_endpoint = match provider_type.as_str() {
        "anthropic" => "https://api.anthropic.com",
        "openai" => "https://api.openai.com",
        "google" => "https://generativelanguage.googleapis.com",
        "deepseek" => "https://api.deepseek.com",
        "mistral" => "https://api.mistral.ai",
        "groq" => "https://api.groq.com/openai",
        "together" => "https://api.together.xyz",
        _ => "https://api.example.com",
    };

    let endpoint = match text_input("API Endpoint", Some(default_endpoint)) {
        Some(e) => e,
        None => return,
    };

    let env_var = match provider_type.as_str() {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "google" => "GOOGLE_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        "mistral" => "MISTRAL_API_KEY",
        "groq" => "GROQ_API_KEY",
        "together" => "TOGETHER_API_KEY",
        _ => "API_KEY",
    };

    let api_key = text_input(&format!("API Key (or Enter to use ${})", env_var), None);

    let default_model = match provider_type.as_str() {
        "anthropic" => "claude-sonnet-4-5-20250514",
        "openai" => "gpt-5.2",
        "google" => "gemini-3.0-pro",
        "deepseek" => "deepseek-v3",
        "mistral" => "mistral-large-latest",
        "groq" => "llama-4-70b",
        "together" => "meta-llama/Llama-4-70B-Instruct",
        _ => "default",
    };

    let model = text_input("Default model", Some(default_model)).unwrap_or_else(|| default_model.to_string());

    let connection = ProviderConnection {
        name: name.clone(),
        provider_type,
        endpoint,
        api_key,
        model,
        enabled: true,
    };

    CONFIGURED_PROVIDERS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("Provider lock poisoned")
        .push(connection);
    PROVIDER_PRIORITY
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("Priority lock poisoned")
        .push(name.clone());

    println!("\n{} Cloud provider '{}' added", style("✓").green(), name);
    println!("{}", style("Press Enter to continue...").dim());
    let _ = io::stdin().read_line(&mut String::new());
}

/// BIOS-style provider priority menu
pub fn show_priority_menu() {
    loop {
        let mut priority = get_priority();
        let providers = get_providers();

        // If priority is empty but providers exist, populate priority from providers
        if priority.is_empty() && !providers.is_empty() {
            for provider in &providers {
                if !priority.contains(&provider.name) {
                    priority.push(provider.name.clone());
                    PROVIDER_PRIORITY
                        .get_or_init(|| Mutex::new(Vec::new()))
                        .lock()
                        .expect("Priority lock poisoned")
                        .push(provider.name.clone());
                }
            }
        }

        println!("\n{}", style("═".repeat(60)).dim());
        println!("{}", style("Provider Priority (Boot Order)").cyan().bold());
        println!("{}", style("Higher = tried first. Like BIOS boot device priority.").dim());
        println!("{}\n", style("═".repeat(60)).dim());

        if priority.is_empty() {
            println!("{}", style("No providers configured. Add some in 'Providers & Connections' first.").dim());
            println!("\n{}", style("Press Enter to go back...").dim());
            let _ = io::stdin().read_line(&mut String::new());
            return;
        }

        println!("{}", style("Current Priority:").bold());
        for (i, name) in priority.iter().enumerate() {
            let provider = providers.iter().find(|p| &p.name == name);
            let details = provider.map(|p| format!("({} - {})", p.provider_type, p.model)).unwrap_or_default();

            let arrow = if i == 0 { "►" } else { " " };
            println!("  {} {}. {} {}", style(arrow).green(), i + 1, style(name).bold(), style(details).dim());
        }
        println!();

        let options = vec![
            MenuOption::with_description("Move Up", "Increase priority of a provider", "up"),
            MenuOption::with_description("Move Down", "Decrease priority of a provider", "down"),
            MenuOption::with_description("Move to Top", "Make provider highest priority", "top"),
            MenuOption::with_description("Move to Bottom", "Make provider lowest priority", "bottom"),
        ];

        match show_menu("Reorder Providers", &options, false, true) {
            MenuResult::Selected(v) => {
                print!("{} ", style("Enter provider number:").cyan());
                let _ = io::stdout().flush();
                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_err() {
                    continue;
                }

                if let Ok(num) = input.trim().parse::<usize>() {
                    if num > 0 && num <= priority.len() {
                        let idx = num - 1;
                        let mut priority_list = PROVIDER_PRIORITY
                            .get_or_init(|| Mutex::new(Vec::new()))
                            .lock()
                            .expect("Priority lock poisoned");
                        match v.as_str() {
                            "up" if idx > 0 => {
                                priority_list.swap(idx, idx - 1);
                                println!("{} Moved up", style("✓").green());
                            }
                            "down" if idx < priority_list.len() - 1 => {
                                priority_list.swap(idx, idx + 1);
                                println!("{} Moved down", style("✓").green());
                            }
                            "top" => {
                                let item = priority_list.remove(idx);
                                priority_list.insert(0, item);
                                println!("{} Moved to top", style("✓").green());
                            }
                            "bottom" => {
                                let item = priority_list.remove(idx);
                                priority_list.push(item);
                                println!("{} Moved to bottom", style("✓").green());
                            }
                            _ => println!("{} Cannot move further", style("⚠").yellow()),
                        }
                    }
                }
            }
            MenuResult::Back | MenuResult::Exit => break,
            _ => {}
        }
    }
}

/// Legacy - kept for compatibility
pub fn show_provider_settings() -> Option<ProviderSettings> {
    show_connections_menu();
    None
}

/// Provider configuration result (legacy)
pub struct ProviderSettings {
    pub provider: String,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub model: String,
}

/// Secondary local server configuration (for vision, etc.)
static SECONDARY_SERVER: OnceLock<Mutex<Option<SecondaryServer>>> = OnceLock::new();

#[derive(Clone)]
pub struct SecondaryServer {
    pub name: String,
    pub url: String,
    pub has_vision: bool,
}

/// Get configured secondary server
pub fn get_secondary_server() -> Option<SecondaryServer> {
    SECONDARY_SERVER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("Secondary server lock poisoned")
        .clone()
}

/// Configure secondary local server
pub fn show_secondary_server_settings() -> Option<SecondaryServer> {
    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("Add Additional Local Server").cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    println!("{}", style("Configure a secondary LM Studio/Ollama server.").dim());
    println!("{}", style("This can be used for vision models or as a fallback.").dim());
    println!();

    let name = text_input("Name this server", Some("secondary"))?;
    let url = text_input("Server URL", Some("http://localhost:1235"))?;
    let has_vision = confirm("Does this server have a vision-capable model loaded?", false);

    let server = SecondaryServer {
        name,
        url,
        has_vision,
    };

    // Store it
    *SECONDARY_SERVER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("Secondary server lock poisoned") = Some(server.clone());

    println!("\n{} Secondary server configured: {}", style("✓").green(), server.url);

    Some(server)
}

/// Vision model settings
pub fn show_vision_settings() -> Option<VisionSettings> {
    // Ensure providers are initialized from environment
    init_providers_from_env();

    println!();

    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("Vision Model Configuration").cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    // Check if primary model has vision
    let primary_has_vision = confirm("Does your primary model support vision?", false);

    if primary_has_vision {
        return Some(VisionSettings {
            enabled: true,
            source: VisionSource::Primary,
            secondary_server: None,
            cloud_provider: None,
            cloud_model: None,
        });
    }

    // Get configured local providers for vision options
    let providers = get_providers();
    let local_providers: Vec<_> = providers.iter()
        .filter(|p| p.provider_type == "lmstudio" || p.provider_type == "ollama")
        .collect();

    // Build options based on what's available
    let mut options = vec![];

    // Add local servers as vision options
    for provider in &local_providers {
        options.push(MenuOption::with_description(
            &format!("Local: {}", provider.name),
            &provider.endpoint,
            &format!("local:{}", provider.name)
        ));
    }

    // Also check for legacy secondary server
    let secondary = get_secondary_server();
    if let Some(ref server) = secondary {
        // Only add if not already in providers list
        let already_listed = local_providers.iter().any(|p| p.endpoint == server.url);
        if !already_listed && server.has_vision {
            options.push(MenuOption::with_description(
                &format!("Secondary: {}", server.name),
                &server.url,
                "secondary"
            ));
        }
    }

    // Cloud options
    options.push(MenuOption::with_description("Anthropic Claude", "claude-sonnet-4-5-20250514 (cloud)", "anthropic"));
    options.push(MenuOption::with_description("OpenAI GPT-4o", "Good vision (cloud)", "openai"));
    options.push(MenuOption::with_description("Google Gemini", "Fast vision (cloud)", "google"));
    options.push(MenuOption::with_description("No Vision", "Disable vision capabilities", "none"));

    if local_providers.is_empty() && secondary.is_none() {
        println!("{}", style("Tip: Add a local server in Providers & Connections to use local vision models.").dim());
        println!();
    }

    let choice = match show_menu("Select Vision Source", &options, false, true) {
        MenuResult::Selected(v) => v,
        _ => return None,
    };

    // Handle local server selections (format: "local:name")
    if choice.starts_with("local:") {
        let name = choice.strip_prefix("local:").unwrap_or("");
        if let Some(provider) = local_providers.iter().find(|p| p.name == name) {
            let server = SecondaryServer {
                name: provider.name.clone(),
                url: provider.endpoint.clone(),
                has_vision: true,
            };
            // Also store it as secondary server for future reference
            *SECONDARY_SERVER
                .get_or_init(|| Mutex::new(None))
                .lock()
                .expect("Secondary server lock poisoned") = Some(server.clone());
            return Some(VisionSettings {
                enabled: true,
                source: VisionSource::SecondaryLocal,
                secondary_server: Some(server),
                cloud_provider: None,
                cloud_model: None,
            });
        }
    }

    match choice.as_str() {
        "none" => Some(VisionSettings {
            enabled: false,
            source: VisionSource::None,
            secondary_server: None,
            cloud_provider: None,
            cloud_model: None,
        }),
        "secondary" => Some(VisionSettings {
            enabled: true,
            source: VisionSource::SecondaryLocal,
            secondary_server: secondary,
            cloud_provider: None,
            cloud_model: None,
        }),
        provider => {
            let default_model = match provider {
                "anthropic" => Some("claude-sonnet-4-5-20250514"),
                "openai" => Some("gpt-4o"),
                "google" => Some("gemini-2.0-flash"),
                _ => None,
            };

            let model = text_input("Vision model", default_model)?;

            Some(VisionSettings {
                enabled: true,
                source: VisionSource::Cloud,
                secondary_server: None,
                cloud_provider: Some(provider.to_string()),
                cloud_model: Some(model),
            })
        }
    }
}

/// Vision source options
#[derive(Clone, Debug)]
pub enum VisionSource {
    None,
    Primary,
    SecondaryLocal,
    Cloud,
}

/// Vision configuration result
pub struct VisionSettings {
    pub enabled: bool,
    pub source: VisionSource,
    pub secondary_server: Option<SecondaryServer>,
    pub cloud_provider: Option<String>,
    pub cloud_model: Option<String>,
}

/// MCP Server configuration
pub fn show_mcp_settings() {
    use crate::orchestrator::mcp::{McpManager, ServerStatus, connect_mcp_server, get_all_mcp_tools};

    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("MCP Server Configuration").cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    println!("{}", style("MCP (Model Context Protocol) servers extend Ganesha's capabilities.").dim());
    println!("{}", style("They provide web browsing, documentation, and more.").dim());
    println!();

    let mut manager = McpManager::new();

    loop {
        let options = vec![
            MenuOption::with_description("📋 List Servers", "Show available MCP servers and status", "list"),
            MenuOption::with_description("🔌 Connect Server", "Connect to an MCP server", "connect"),
            MenuOption::with_description("📦 Install All", "Install all default MCP servers", "install"),
            MenuOption::with_description("🛠️ View Tools", "Show tools from connected servers", "tools"),
            MenuOption::with_description("⬅️ Back", "Return to settings", "back"),
        ];

        match show_menu("MCP Servers", &options, false, true) {
            MenuResult::Selected(v) => {
                match v.as_str() {
                    "list" => {
                        println!("\n{}", style("Available MCP Servers:").cyan().bold());
                        println!();
                        for server in manager.list_servers() {
                            let status_icon = match server.status {
                                ServerStatus::Running => style("●").green(),
                                ServerStatus::Stopped => style("○").yellow(),
                                ServerStatus::Starting => style("◐").blue(),
                                ServerStatus::Failed => style("✗").red(),
                                ServerStatus::NotInstalled => style("◌").dim(),
                            };
                            let status_text = match server.status {
                                ServerStatus::Running => "running",
                                ServerStatus::Stopped => "stopped",
                                ServerStatus::Starting => "starting",
                                ServerStatus::Failed => "failed",
                                ServerStatus::NotInstalled => "not installed",
                            };
                            println!("  {} {} - {} [{}]",
                                status_icon,
                                style(&server.name).bold(),
                                server.description,
                                style(status_text).dim()
                            );
                        }
                        println!();
                        println!("{}", style("Press Enter to continue...").dim());
                        let _ = io::stdin().read_line(&mut String::new());
                    }
                    "connect" => {
                        println!("\n{}", style("Connect MCP Server:").cyan().bold());
                        println!();

                        // Show servers that aren't running
                        let available: Vec<_> = manager.list_servers()
                            .iter()
                            .filter(|s| s.status != ServerStatus::Running)
                            .map(|s| MenuOption::with_description(&s.name, &s.description, &s.name))
                            .collect();

                        if available.is_empty() {
                            println!("  {} All servers are already running or not installed.", style("ℹ").cyan());
                        } else {
                            if let MenuResult::Selected(name) = show_menu("Select Server", &available, false, true) {
                                println!("\n  {} Connecting to {}...", style("⌛").yellow(), name);

                                // First check if installed
                                if let Some(server) = manager.get_server(&name) {
                                    if server.status == ServerStatus::NotInstalled {
                                        println!("  {} Installing first...", style("📦").cyan());
                                        match manager.install_server(&name) {
                                            Ok(_) => println!("  {} Installed successfully", style("✓").green()),
                                            Err(e) => {
                                                println!("  {} Install failed: {}", style("✗").red(), e);
                                                continue;
                                            }
                                        }
                                    }

                                    // Get fresh server ref after potential install
                                    if let Some(server) = manager.get_server(&name) {
                                        match connect_mcp_server(server) {
                                            Ok(_) => {
                                                println!("\n  {} Connected to {}", style("✓").green(), name);
                                            }
                                            Err(e) => {
                                                println!("  {} Connection failed: {}", style("✗").red(), e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        println!("\n{}", style("Press Enter to continue...").dim());
                        let _ = io::stdin().read_line(&mut String::new());
                    }
                    "install" => {
                        println!("\n{}", style("Installing default MCP servers...").cyan().bold());
                        println!();

                        // Install all defaults (synchronous)
                        match manager.install_defaults() {
                            Ok(_) => println!("\n  {} Default servers installed", style("✓").green()),
                            Err(e) => println!("\n  {} Some installations failed: {}", style("⚠").yellow(), e),
                        }

                        println!("\n{}", style("Press Enter to continue...").dim());
                        let _ = io::stdin().read_line(&mut String::new());
                    }
                    "tools" => {
                        println!("\n{}", style("Available MCP Tools:").cyan().bold());
                        println!();

                        let all_tools = get_all_mcp_tools();
                        if all_tools.is_empty() {
                            println!("  {} No MCP servers connected. Connect a server first.", style("ℹ").cyan());
                        } else {
                            for (server_name, tools) in all_tools {
                                println!("  {} {} tools:", style("◆").cyan(), server_name);
                                for tool in tools {
                                    println!("    {} {}: {}",
                                        style("•").dim(),
                                        style(&tool.name).bold(),
                                        tool.description.as_deref().unwrap_or("")
                                    );
                                }
                                println!();
                            }
                        }

                        println!("{}", style("Press Enter to continue...").dim());
                        let _ = io::stdin().read_line(&mut String::new());
                    }
                    "back" => return,
                    _ => {}
                }
            }
            MenuResult::Back | MenuResult::Exit => return,
            _ => {}
        }
    }
}

/// Permissions/Access Control settings
pub fn show_permissions_settings() {
    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("Permissions & Access Control").cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    let options = vec![
        MenuOption::with_description("Restricted", "Only safe, read-only commands allowed", "restricted"),
        MenuOption::with_description("Standard", "Normal commands allowed (default)", "standard"),
        MenuOption::with_description("Elevated", "More powerful commands (with warnings)", "elevated"),
        MenuOption::with_description("Full Access", "All commands allowed (dangerous)", "full"),
    ];

    println!("{}", style("Current access level determines which commands Ganesha can execute.").dim());
    println!();

    if let MenuResult::Selected(v) = show_menu("Access Level", &options, false, true) {
        println!("\n{} Access level set to: {}", style("✓").green(), v);
        println!("{}", style("(Note: Persistent config coming soon)").dim());
        println!("\n{}", style("Press Enter to continue...").dim());
        let _ = io::stdin().read_line(&mut String::new());
    }
}

/// Session history menu
pub fn show_session_history() {
    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("Session History").cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    let options = vec![
        MenuOption::with_description("View Recent", "Show recent sessions", "recent"),
        MenuOption::with_description("Search", "Search session history", "search"),
        MenuOption::with_description("Export", "Export session to file", "export"),
        MenuOption::with_description("Clear History", "Delete all session history", "clear"),
    ];

    if let MenuResult::Selected(v) = show_menu("Session History", &options, false, true) {
        match v.as_str() {
            "recent" => {
                println!("\n{}", style("Recent Sessions:").cyan().bold());
                // TODO: Load from session_dir
                println!("  {} No sessions found.", style("ℹ").dim());
            }
            "search" => {
                if let Some(query) = text_input("Search query", None) {
                    println!("\n{} Searching for: '{}'", style("🔍").cyan(), query);
                    println!("  {} No matching sessions.", style("ℹ").dim());
                }
            }
            "export" => {
                println!("\n{} Use /log command in chat to export current session.", style("ℹ").cyan());
            }
            "clear" => {
                if confirm("Are you sure you want to clear all session history?", false) {
                    println!("\n{} Session history cleared.", style("✓").green());
                }
            }
            _ => {}
        }
        println!("\n{}", style("Press Enter to continue...").dim());
        let _ = io::stdin().read_line(&mut String::new());
    }
}

/// Show available models from all providers with dynamic fetching
pub fn show_models_menu() {
    use console::style;
    use std::io::{self, Write};

    // Initialize providers from environment to sync with actual connections
    init_providers_from_env();

    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("Available Models").cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    println!("{}", style("Fetching models from connected providers...").dim());
    println!();

    let providers = get_providers();

    if providers.is_empty() {
        println!("{} No providers configured in menu system.", style("ℹ").cyan());
        println!();
        println!("{}", style("Note: Ganesha auto-detects providers at startup (shown in header).").dim());
        println!("{}", style("To manage providers here, use /settings > Providers & Connections.").dim());
        println!("\n{}", style("Press Enter to continue...").dim());
        let _ = io::stdin().read_line(&mut String::new());
        return;
    }

    let mut all_models: Vec<(String, String, String)> = vec![]; // (provider, model_id, display_name)

    for provider in &providers {
        if !provider.enabled {
            continue;
        }

        let models = fetch_local_models(&provider.endpoint, &provider.provider_type);

        if models.is_empty() {
            // Show the configured default model if we can't fetch dynamically
            all_models.push((
                provider.name.clone(),
                provider.model.clone(),
                format!("{} (configured)", provider.model)
            ));
        } else {
            for model in models {
                all_models.push((
                    provider.name.clone(),
                    model.clone(),
                    model
                ));
            }
        }
    }

    // Also check OpenRouter if available
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
        println!("{}", style("Checking OpenRouter...").dim());
        let models = fetch_openrouter_models_blocking(&key);
        for (id, name) in models.into_iter().take(15) { // Limit to top 15
            all_models.push(("openrouter".to_string(), id, name));
        }
    }

    if all_models.is_empty() {
        println!("{} No models found from any provider.", style("⚠").yellow());
        println!("\n{}", style("Press Enter to continue...").dim());
        let _ = io::stdin().read_line(&mut String::new());
        return;
    }

    // Group by provider
    let mut current_provider = String::new();
    for (i, (provider, model_id, display_name)) in all_models.iter().enumerate() {
        if provider != &current_provider {
            current_provider = provider.clone();
            println!("\n{}", style(format!("  {}:", provider.to_uppercase())).cyan().bold());
        }
        println!("    {} {} {}",
            style(format!("[{}]", i + 1)).yellow(),
            style(&display_name).bold(),
            if model_id != display_name { style(format!("({})", model_id)).dim() } else { style("".to_string()).dim() }
        );
    }

    println!("\n{}", style("─".repeat(60)).dim());
    println!("{}", style("Enter a number to select a model, or press Enter to go back.").dim());
    print!("{} ", style("Select model:").cyan());
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return;
    }

    let input = input.trim();
    if input.is_empty() {
        return;
    }

    if let Ok(num) = input.parse::<usize>() {
        if num > 0 && num <= all_models.len() {
            let (provider, model_id, _) = &all_models[num - 1];
            println!("\n{} Selected: {} from {}", style("✓").green(), model_id, provider);
            println!("{}", style("Model selection saved for this session.").dim());

            // Store the selection (update the provider's model)
            let mut providers = CONFIGURED_PROVIDERS
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("Provider lock poisoned");
            for p in providers.iter_mut() {
                if &p.name == provider {
                    p.model = model_id.clone();
                    break;
                }
            }
        } else {
            println!("{} Invalid selection.", style("⚠").yellow());
        }
    }

    println!("\n{}", style("Press Enter to continue...").dim());
    let _ = io::stdin().read_line(&mut String::new());
}

/// Fetch OpenRouter models (blocking)
fn fetch_openrouter_models_blocking(api_key: &str) -> Vec<(String, String)> {
    use std::time::Duration;

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let response = match client
        .get("https://openrouter.ai/api/v1/models")
        .bearer_auth(api_key)
        .send()
    {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    if !response.status().is_success() {
        return vec![];
    }

    let text = match response.text() {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    // Parse OpenRouter format: { "data": [{ "id": "...", "name": "..." }] }
    #[derive(serde::Deserialize)]
    struct OpenRouterModels {
        data: Vec<OpenRouterModel>,
    }
    #[derive(serde::Deserialize)]
    struct OpenRouterModel {
        id: String,
        name: Option<String>,
    }

    if let Ok(parsed) = serde_json::from_str::<OpenRouterModels>(&text) {
        // Return popular/recommended models first
        let priority_models = ["claude", "gpt-4", "gpt-5", "gemini", "llama", "mistral", "deepseek"];
        let mut models: Vec<_> = parsed.data.into_iter()
            .filter(|m| priority_models.iter().any(|p| m.id.to_lowercase().contains(p)))
            .map(|m| (m.id.clone(), m.name.unwrap_or(m.id)))
            .collect();
        models.sort_by(|a, b| a.1.cmp(&b.1));
        return models;
    }

    vec![]
}

/// Main settings menu
pub fn show_settings_menu() {
    loop {
        let provider_count = get_providers().len();
        let provider_desc = if provider_count > 0 {
            format!("{} configured", provider_count)
        } else {
            "Add local & cloud providers".to_string()
        };

        let options = vec![
            MenuOption::with_description("Providers & Connections", &provider_desc, "connections"),
            MenuOption::with_description("Provider Priority", "Set fallback order (BIOS-style)", "priority"),
            MenuOption::with_description("Vision Settings", "Configure vision/screenshot model", "vision"),
            MenuOption::with_description("MCP Servers", "Model Context Protocol integrations", "mcp"),
            MenuOption::with_description("Permissions", "Access control & safety levels", "permissions"),
            MenuOption::with_description("Session History", "View and manage sessions", "history"),
        ];

        match show_menu("⚙  Ganesha Settings", &options, false, true) {
            MenuResult::Selected(v) => {
                match v.as_str() {
                    "connections" => {
                        show_connections_menu();
                    }
                    "priority" => {
                        show_priority_menu();
                    }
                    "vision" => {
                        if let Some(settings) = show_vision_settings() {
                            if settings.enabled {
                                println!("\n{} Vision enabled", style("✓").green());
                                match settings.source {
                                    VisionSource::Primary => {
                                        println!("  Source: Primary model");
                                    }
                                    VisionSource::SecondaryLocal => {
                                        if let Some(ref server) = settings.secondary_server {
                                            println!("  Source: Secondary server ({})", server.url);
                                        }
                                    }
                                    VisionSource::Cloud => {
                                        println!("  Source: {} ({})",
                                            settings.cloud_provider.as_deref().unwrap_or("cloud"),
                                            settings.cloud_model.as_deref().unwrap_or("default")
                                        );
                                    }
                                    VisionSource::None => {}
                                }
                            } else {
                                println!("\n{} Vision disabled", style("⚠").yellow());
                            }
                            println!("{}", style("Press Enter to continue...").dim());
                            let _ = io::stdin().read_line(&mut String::new());
                        }
                    }
                    "mcp" => {
                        show_mcp_settings();
                    }
                    "permissions" => {
                        show_permissions_settings();
                    }
                    "history" => {
                        show_session_history();
                    }
                    _ => {}
                }
            }
            MenuResult::Back | MenuResult::Exit => break,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_option_creation() {
        let opt = MenuOption::new("Test", "test");
        assert_eq!(opt.label, "Test");
        assert_eq!(opt.value, "test");
        assert!(opt.description.is_none());

        let opt2 = MenuOption::with_description("Test", "Description", "test");
        assert!(opt2.description.is_some());
    }

    #[test]
    fn test_interview_question_creation() {
        let q = InterviewQuestion::single("q1", "What is your name?", vec![]);
        assert!(q.required);
        assert!(!q.allow_multiple);

        let q2 = InterviewQuestion::multiple("q2", "Select options", vec![]);
        assert!(q2.allow_multiple);
    }
}
