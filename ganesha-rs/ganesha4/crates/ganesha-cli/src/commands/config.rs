//! # Config Command
//!
//! View or set configuration values.

use colored::Colorize;

/// Run the config command
pub async fn run(key: Option<String>, value: Option<String>) -> anyhow::Result<()> {
    match (key, value) {
        (None, None) => {
            // Show all config
            println!("{}", "Current Configuration".bright_cyan().bold());
            println!();
            // TODO: Load and display config
            println!("Configuration display not yet implemented");
        }
        (Some(key), None) => {
            // Show specific key
            println!("Config key: {}", key.bright_yellow());
            // TODO: Look up and display value
            println!("Value lookup not yet implemented");
        }
        (Some(key), Some(value)) => {
            // Set value
            println!(
                "Setting {} = {}",
                key.bright_yellow(),
                value.bright_green()
            );
            // TODO: Update config
            println!("Config update not yet implemented");
        }
        (None, Some(_)) => {
            println!("{} Must specify a key to set a value", "Error:".red());
        }
    }

    Ok(())
}
