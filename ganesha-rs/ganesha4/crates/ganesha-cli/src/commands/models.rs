//! # Models Command
//!
//! List available models from connected providers.

use crate::render;
use colored::Colorize;
use ganesha_providers::{ModelTier, ProviderManager};
use std::sync::Arc;

/// Format context length for display
fn format_context(ctx: Option<u32>) -> String {
    match ctx {
        Some(n) if n >= 1_000_000 => format!("{}M", n / 1_000_000),
        Some(n) if n >= 1_000 => format!("{}k", n / 1_000),
        Some(n) => format!("{}", n),
        None => "-".to_string(),
    }
}

/// Format tier for display
fn format_tier(tier: &ModelTier) -> String {
    match tier {
        ModelTier::Exceptional => "🟢 Exceptional".to_string(),
        ModelTier::Capable => "🟡 Capable".to_string(),
        ModelTier::Limited => "🟠 Limited".to_string(),
        ModelTier::Unsafe => "🔴 Unsafe".to_string(),
        ModelTier::Unknown => "⚪ Unknown".to_string(),
    }
}

/// Run the models command
pub async fn run(provider: Option<String>) -> anyhow::Result<()> {
    // Initialize provider manager
    let provider_manager = Arc::new(ProviderManager::new());

    println!("{}", "Discovering providers...".dimmed());
    if let Err(e) = provider_manager.auto_discover().await {
        println!("{} Failed to discover providers: {}", "Warning:".yellow(), e);
    }

    // Get all providers
    let providers = provider_manager.list_providers().await;

    if providers.is_empty() {
        println!();
        println!("{}", "No LLM providers found.".yellow());
        println!();
        println!("Set one of the following environment variables:");
        println!("  - {} for Claude", "ANTHROPIC_API_KEY".cyan());
        println!("  - {} for GPT-4", "OPENAI_API_KEY".cyan());
        println!("  - {} for OpenRouter", "OPENROUTER_API_KEY".cyan());
        println!("\nOr start a local server (Ollama, LM Studio, etc.)");
        return Ok(());
    }

    println!();
    println!("{}", "Available Models".bright_cyan().bold());
    println!();

    if let Some(ref p) = provider {
        println!("Filtering by provider: {}", p.bright_yellow());
        println!();
    }

    // Fetch models from all providers
    let all_models = provider_manager.list_all_models().await?;

    if all_models.is_empty() {
        println!("{}", "No models found from connected providers.".yellow());
        return Ok(());
    }

    // Filter by provider if specified
    let models: Vec<_> = if let Some(ref filter) = provider {
        all_models
            .into_iter()
            .filter(|m| m.provider.to_lowercase().contains(&filter.to_lowercase()))
            .collect()
    } else {
        all_models
    };

    // Build table rows
    let headers = &["Provider", "Model", "Tier", "Context", "Vision"];
    let mut rows = Vec::new();

    for model in &models {
        rows.push(vec![
            model.provider.clone(),
            model.id.clone(),
            format_tier(&model.tier),
            format_context(model.context_length),
            if model.supports_vision { "✓".to_string() } else { "".to_string() },
        ]);
    }

    render::print_table(headers, &rows);

    println!();
    println!(
        "{} model(s) from {} provider(s)",
        models.len().to_string().bright_green(),
        providers.len().to_string().bright_green()
    );
    println!();
    println!(
        "Use {} to switch models",
        "ganesha --model <name>".bright_green()
    );

    Ok(())
}
