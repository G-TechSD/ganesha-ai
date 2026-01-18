//! # Ganesha CLI
//!
//! The main entry point for the Ganesha AI coding assistant.

mod cli;
mod repl;
mod commands;
mod tui;
mod config;
mod render;
mod history;

use clap::Parser;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Handle commands
    match cli.command {
        Some(Commands::Chat { ref message, ref model }) => {
            commands::chat::run(message.clone(), model.clone(), &cli).await?;
        }
        Some(Commands::Init { force }) => {
            commands::init::run(force).await?;
        }
        Some(Commands::Config { key, value }) => {
            commands::config::run(key, value).await?;
        }
        Some(Commands::Mcp { action }) => {
            commands::mcp::run(action).await?;
        }
        Some(Commands::Models { provider }) => {
            commands::models::run(provider).await?;
        }
        Some(Commands::Session { action }) => {
            commands::session::run(action).await?;
        }
        Some(Commands::Tui) => {
            tui::run().await?;
        }
        Some(Commands::Completions { shell }) => {
            cli::generate_completions(shell);
        }
        None => {
            // Default: start interactive REPL
            if cli.tui {
                tui::run().await?;
            } else {
                repl::run(&cli).await?;
            }
        }
    }

    Ok(())
}
