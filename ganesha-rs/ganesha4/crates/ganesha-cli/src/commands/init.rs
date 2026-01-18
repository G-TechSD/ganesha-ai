//! # Init Command
//!
//! Initialize Ganesha in a directory.

use colored::Colorize;
use std::path::Path;
use tokio::fs;

/// Run the init command
pub async fn run(force: bool) -> anyhow::Result<()> {
    let ganesha_dir = Path::new(".ganesha");

    if ganesha_dir.exists() && !force {
        println!(
            "{} Ganesha already initialized in this directory.",
            "!".yellow()
        );
        println!("Use {} to re-initialize.", "--force".bright_cyan());
        return Ok(());
    }

    // Create .ganesha directory
    fs::create_dir_all(ganesha_dir).await?;

    // Create default config
    let config_path = ganesha_dir.join("config.toml");
    let default_config = r#"# Ganesha Configuration
# See https://ganesha.dev/docs/config for all options

[general]
# Default risk level: safe, normal, trusted, yolo
risk_level = "normal"

# Default chat mode: code, ask, architect
mode = "code"

[provider]
# Preferred provider order (local-first by default)
# priority = ["lmstudio", "ollama", "anthropic", "openrouter"]

# Default model (optional)
# model = "claude-3-5-sonnet"

[mcp]
# Auto-connect MCP servers on startup
auto_connect = true

# Trusted servers (auto-approve tool calls)
# trusted = ["filesystem", "fetch"]

[session]
# Auto-save sessions
auto_save = true

# Session location
# path = "~/.local/share/ganesha/sessions"
"#;

    fs::write(&config_path, default_config).await?;

    // Create commands directory for custom slash commands
    fs::create_dir_all(ganesha_dir.join("commands")).await?;

    // Create example custom command
    let example_command = r#"# Example custom command
# This command will be available as /example

prompt = "Please analyze the following and provide suggestions."
description = "Analyze code and provide suggestions"

# Optional: pass arguments with {{args}}
# prompt = "Analyze {{args}} and suggest improvements"
"#;
    fs::write(
        ganesha_dir.join("commands").join("example.toml"),
        example_command,
    )
    .await?;

    // Create .gitignore for Ganesha directory
    let gitignore = r#"# Ganesha local files
sessions/
*.log
credentials.toml
"#;
    fs::write(ganesha_dir.join(".gitignore"), gitignore).await?;

    println!("{} Ganesha initialized!", "✓".bright_green());
    println!();
    println!("Created:");
    println!("  {} - Configuration file", ".ganesha/config.toml".bright_cyan());
    println!("  {} - Custom commands", ".ganesha/commands/".bright_cyan());
    println!();
    println!(
        "Run {} to start the interactive assistant.",
        "ganesha".bright_green()
    );

    Ok(())
}
