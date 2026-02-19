//! # Ganesha CLI
//!
//! The main entry point for the Ganesha AI coding assistant.

mod cli;
mod repl;
mod commands;
mod tui;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod render;
#[allow(dead_code)]
mod history;
mod setup;
#[allow(dead_code)]
mod voice_input;

use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Enable ANSI colors on Windows
    #[cfg(windows)]
    let _ = colored::control::set_virtual_terminal(true);

    // Initialize logging - default to warn to keep output clean
    // Use RUST_LOG=info or RUST_LOG=debug for verbose output
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time() // Don't show timestamps in output
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
        Some(Commands::Voice { action }) => {
            commands::voice::run(action).await?;
        }
        Some(Commands::Flux { ref duration, ref task }) => {
            commands::flux::run(duration.clone(), task.clone(), &cli).await?;
        }
        Some(Commands::Setup) => {
            setup::run_setup_wizard()?;
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
