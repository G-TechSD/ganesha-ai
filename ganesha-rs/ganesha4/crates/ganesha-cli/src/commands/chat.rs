//! # Chat Command
//!
//! Send a single message and exit.

use crate::cli::Cli;
use crate::render;
use colored::Colorize;
use ganesha_providers::{GenerateOptions, Message, ProviderManager};

/// Run the chat command
pub async fn run(message: String, model: Option<String>, cli: &Cli) -> anyhow::Result<()> {
    // Initialize provider manager
    let provider_manager = ProviderManager::new();

    // Auto-discover available providers
    provider_manager.auto_discover().await?;

    // Check if we have a provider
    if !provider_manager.has_available_provider().await {
        eprintln!(
            "{} No LLM providers available.\n",
            "Error:".red().bold()
        );
        eprintln!("Set one of the following environment variables:");
        eprintln!("  - {} for Claude", "ANTHROPIC_API_KEY".cyan());
        eprintln!("  - {} for GPT-4", "OPENAI_API_KEY".cyan());
        eprintln!("  - {} for OpenRouter", "OPENROUTER_API_KEY".cyan());
        eprintln!("\nOr start a local server (LM Studio, Ollama, etc.)");
        return Err(anyhow::anyhow!("No providers available"));
    }

    let spinner = render::Spinner::new("Thinking...");

    // Build system prompt based on mode
    let system_prompt = match cli.mode {
        crate::cli::ChatMode::Code => {
            "You are Ganesha, an AI coding assistant. Be concise and provide working code."
        }
        crate::cli::ChatMode::Ask => {
            "You are Ganesha, an AI assistant. Answer questions clearly and concisely."
        }
        crate::cli::ChatMode::Architect => {
            "You are Ganesha, a software architect. Help plan and design software systems."
        }
        crate::cli::ChatMode::Help => {
            "You are Ganesha's help system. Explain features, commands, and capabilities."
        }
    };

    // Build messages
    let messages = vec![
        Message::system(system_prompt),
        Message::user(&message),
    ];

    // Set up options
    let options = GenerateOptions {
        model: model.or_else(|| cli.model.clone()),
        temperature: Some(0.7),
        max_tokens: Some(4096),
        ..Default::default()
    };

    // Call the provider
    let response = provider_manager.chat(&messages, &options).await;

    spinner.finish();

    match response {
        Ok(resp) => {
            render::print_assistant_message(&resp.content);

            // Show model info in verbose mode
            if cli.verbose {
                eprintln!("\n{}", format!("Model: {}", resp.model).dimmed());
                if let Some(usage) = resp.usage {
                    eprintln!(
                        "{}",
                        format!(
                            "Tokens: {} prompt + {} completion = {} total",
                            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                        )
                        .dimmed()
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            Err(e.into())
        }
    }
}
