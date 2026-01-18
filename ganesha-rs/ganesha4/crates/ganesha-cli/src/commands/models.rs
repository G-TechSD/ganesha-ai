//! # Models Command
//!
//! List available models.

use crate::render;
use colored::Colorize;

/// Run the models command
pub async fn run(provider: Option<String>) -> anyhow::Result<()> {
    println!("{}", "Available Models".bright_cyan().bold());
    println!();

    if let Some(ref p) = provider {
        println!("Filtering by provider: {}", p.bright_yellow());
        println!();
    }

    // Show tier legend
    println!("Model Tiers:");
    render::print_model_tier("exceptional", "claude-3-5-sonnet");
    render::print_model_tier("capable", "gpt-4o-mini");
    render::print_model_tier("limited", "llama-3.1-8b");
    render::print_model_tier("unsafe", "phi-2");
    println!();

    // TODO: Actually list models from providers
    let headers = &["Provider", "Model", "Tier", "Context", "Vision"];
    let rows = vec![
        vec![
            "anthropic".to_string(),
            "claude-3-5-sonnet".to_string(),
            "🟢 Exceptional".to_string(),
            "200k".to_string(),
            "✓".to_string(),
        ],
        vec![
            "openai".to_string(),
            "gpt-4o".to_string(),
            "🟢 Exceptional".to_string(),
            "128k".to_string(),
            "✓".to_string(),
        ],
        vec![
            "openai".to_string(),
            "gpt-4o-mini".to_string(),
            "🟡 Capable".to_string(),
            "128k".to_string(),
            "✓".to_string(),
        ],
        vec![
            "ollama".to_string(),
            "llama-3.1-70b".to_string(),
            "🟡 Capable".to_string(),
            "128k".to_string(),
            "".to_string(),
        ],
    ];

    render::print_table(headers, &rows);

    println!();
    println!(
        "Use {} to switch models",
        "ganesha --model <name>".bright_green()
    );

    Ok(())
}
