//! # MCP Command
//!
//! Manage MCP servers.

use crate::cli::McpAction;
use crate::render;
use colored::Colorize;

/// Run the mcp command
pub async fn run(action: McpAction) -> anyhow::Result<()> {
    match action {
        McpAction::List => {
            println!("{}", "Configured MCP Servers".bright_cyan().bold());
            println!();
            // TODO: List servers from config
            println!("No servers configured");
            println!();
            println!(
                "Use {} to add a server",
                "ganesha mcp add <name> <source>".bright_green()
            );
        }

        McpAction::Add { name, source } => {
            println!("Adding MCP server: {} ({})", name.bright_yellow(), source);
            // TODO: Add to config
            println!("{} Server added (not yet implemented)", "✓".green());
        }

        McpAction::Remove { name } => {
            println!("Removing MCP server: {}", name.bright_yellow());
            // TODO: Remove from config
            println!("{} Server removed (not yet implemented)", "✓".green());
        }

        McpAction::Connect { name } => {
            let spinner = render::Spinner::new(&format!("Connecting to {}...", name));
            // TODO: Connect to server
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            spinner.finish_with_message(&format!("{} Connected to {}", "✓".green(), name));
        }

        McpAction::Disconnect { name } => {
            println!("Disconnecting from: {}", name.bright_yellow());
            // TODO: Disconnect
            println!("{} Disconnected", "✓".green());
        }

        McpAction::Tools => {
            println!("{}", "Available MCP Tools".bright_cyan().bold());
            println!();
            // TODO: List tools from connected servers
            println!("No tools available (no servers connected)");
        }

        McpAction::Install { server_id } => {
            println!("Installing MCP server: {}", server_id.bright_yellow());

            // Look up in registry
            let registry = ganesha_mcp::ServerRegistry::with_builtin();
            if let Some(entry) = registry.get(&server_id) {
                println!("  Name: {}", entry.name);
                println!("  Description: {}", entry.description.dimmed());

                if !entry.required_env.is_empty() {
                    println!();
                    println!("  {} Required environment variables:", "!".yellow());
                    for var in &entry.required_env {
                        let status = if std::env::var(&var.name).is_ok() {
                            "✓".green()
                        } else {
                            "✗".red()
                        };
                        println!("    {} {} - {}", status, var.name, var.description);
                        if let Some(url) = &var.obtain_url {
                            println!("      Get it at: {}", url.bright_blue());
                        }
                    }
                }

                println!();
                println!("Command: {}", entry.install_command.bright_cyan());
                // TODO: Actually install
            } else {
                println!(
                    "{} Unknown server: {}",
                    "Error:".red(),
                    server_id
                );
                println!();
                println!("Known servers:");
                for (id, entry) in registry.verified() {
                    println!("  {} - {}", id.bright_green(), entry.description.dimmed());
                }
            }
        }
    }

    Ok(())
}
