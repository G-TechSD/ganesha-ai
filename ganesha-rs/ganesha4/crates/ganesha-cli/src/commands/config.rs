//! # Config Command
//!
//! View or set configuration values.

use colored::Colorize;
use ganesha_core::config::CoreConfig;

/// Get a nested value from the config by key path
fn get_config_value(config: &CoreConfig, key: &str) -> Option<String> {
    let parts: Vec<&str> = key.split('.').collect();

    match parts.as_slice() {
        // Risk level
        ["risk_level"] => Some(format!("{:?}", config.risk_level)),

        // AI settings
        ["ai", "provider"] => Some(config.ai.provider.clone()),
        ["ai", "model"] => Some(config.ai.model.clone()),
        ["ai", "max_tokens"] => Some(config.ai.max_tokens.to_string()),
        ["ai", "temperature"] => Some(config.ai.temperature.to_string()),
        ["ai", "timeout_secs"] => Some(config.ai.timeout_secs.to_string()),
        ["ai", "streaming"] => Some(config.ai.streaming.to_string()),
        ["ai", "api_base_url"] => config.ai.api_base_url.clone(),

        // Execution settings
        ["execution", "command_timeout_secs"] => {
            Some(config.execution.command_timeout_secs.to_string())
        }
        ["execution", "auto_rollback"] => Some(config.execution.auto_rollback.to_string()),
        ["execution", "max_file_size"] => Some(config.execution.max_file_size.to_string()),
        ["execution", "shell"] => Some(config.execution.shell.clone()),
        ["execution", "dry_run"] => Some(config.execution.dry_run.to_string()),

        // Session settings
        ["session", "storage_dir"] => Some(config.session.storage_dir.display().to_string()),
        ["session", "max_age_days"] => Some(config.session.max_age_days.to_string()),
        ["session", "autosave_interval_secs"] => {
            Some(config.session.autosave_interval_secs.to_string())
        }
        ["session", "auto_checkpoint"] => Some(config.session.auto_checkpoint.to_string()),

        // Display settings
        ["display", "colors"] => Some(config.display.colors.to_string()),
        ["display", "emoji"] => Some(config.display.emoji.to_string()),
        ["display", "verbose"] => Some(config.display.verbose.to_string()),
        ["display", "theme"] => Some(config.display.theme.clone()),

        // Verification settings
        ["verification", "auto_verify"] => Some(config.verification.auto_verify.to_string()),
        ["verification", "run_tests"] => Some(config.verification.run_tests.to_string()),
        ["verification", "timeout_secs"] => Some(config.verification.timeout_secs.to_string()),

        // Custom values
        ["custom", rest @ ..] if !rest.is_empty() => {
            let custom_key = rest.join(".");
            config
                .custom
                .get(&custom_key)
                .map(|v| v.to_string())
        }

        _ => None,
    }
}

/// Set a config value by key path
fn set_config_value(config: &mut CoreConfig, key: &str, value: &str) -> anyhow::Result<()> {
    let parts: Vec<&str> = key.split('.').collect();

    match parts.as_slice() {
        // Risk level
        ["risk_level"] => {
            config.risk_level = match value.to_lowercase().as_str() {
                "safe" => ganesha_core::RiskLevel::Safe,
                "normal" => ganesha_core::RiskLevel::Normal,
                "trusted" => ganesha_core::RiskLevel::Trusted,
                "yolo" => ganesha_core::RiskLevel::Yolo,
                _ => return Err(anyhow::anyhow!("Invalid risk level: {}", value)),
            };
        }

        // AI settings
        ["ai", "provider"] => config.ai.provider = value.to_string(),
        ["ai", "model"] => config.ai.model = value.to_string(),
        ["ai", "max_tokens"] => {
            config.ai.max_tokens = value.parse().map_err(|_| anyhow::anyhow!("Invalid number"))?
        }
        ["ai", "temperature"] => {
            let temp: f32 = value.parse().map_err(|_| anyhow::anyhow!("Invalid number"))?;
            if !(0.0..=2.0).contains(&temp) {
                return Err(anyhow::anyhow!("Temperature must be between 0.0 and 2.0"));
            }
            config.ai.temperature = temp;
        }
        ["ai", "timeout_secs"] => {
            config.ai.timeout_secs = value.parse().map_err(|_| anyhow::anyhow!("Invalid number"))?
        }
        ["ai", "streaming"] => config.ai.streaming = value.parse().unwrap_or(true),
        ["ai", "api_base_url"] => config.ai.api_base_url = Some(value.to_string()),

        // Execution settings
        ["execution", "command_timeout_secs"] => {
            config.execution.command_timeout_secs =
                value.parse().map_err(|_| anyhow::anyhow!("Invalid number"))?
        }
        ["execution", "auto_rollback"] => config.execution.auto_rollback = value.parse().unwrap_or(true),
        ["execution", "shell"] => config.execution.shell = value.to_string(),
        ["execution", "dry_run"] => config.execution.dry_run = value.parse().unwrap_or(false),

        // Session settings
        ["session", "max_age_days"] => {
            config.session.max_age_days =
                value.parse().map_err(|_| anyhow::anyhow!("Invalid number"))?
        }
        ["session", "auto_checkpoint"] => config.session.auto_checkpoint = value.parse().unwrap_or(true),

        // Display settings
        ["display", "colors"] => config.display.colors = value.parse().unwrap_or(true),
        ["display", "emoji"] => config.display.emoji = value.parse().unwrap_or(true),
        ["display", "verbose"] => config.display.verbose = value.parse().unwrap_or(false),
        ["display", "theme"] => config.display.theme = value.to_string(),

        // Verification settings
        ["verification", "auto_verify"] => config.verification.auto_verify = value.parse().unwrap_or(true),
        ["verification", "run_tests"] => config.verification.run_tests = value.parse().unwrap_or(true),

        // Custom values
        ["custom", rest @ ..] if !rest.is_empty() => {
            let custom_key = rest.join(".");
            config.set_custom(&custom_key, value);
        }

        _ => return Err(anyhow::anyhow!("Unknown config key: {}", key)),
    }

    Ok(())
}

/// Print a config section
fn print_section(name: &str, items: &[(&str, String)]) {
    println!("{}:", name.bright_cyan().bold());
    for (key, value) in items {
        println!("  {} = {}", key.bright_yellow(), value.bright_white());
    }
    println!();
}

/// Run the config command
pub async fn run(key: Option<String>, value: Option<String>) -> anyhow::Result<()> {
    // Load config
    let mut config = CoreConfig::load().unwrap_or_default();

    match (key, value) {
        (None, None) => {
            // Show all config
            println!("{}\n", "Current Configuration".bright_cyan().bold());

            // Show config file locations
            if let Some(global_path) = CoreConfig::global_config_path() {
                let exists = global_path.exists();
                println!(
                    "{} {} {}",
                    "Global:".dimmed(),
                    global_path.display(),
                    if exists {
                        "(found)".green()
                    } else {
                        "(not found)".dimmed()
                    }
                );
            }
            if let Some(project_path) = CoreConfig::project_config_path() {
                let exists = project_path.exists();
                println!(
                    "{} {} {}",
                    "Project:".dimmed(),
                    project_path.display(),
                    if exists {
                        "(found)".green()
                    } else {
                        "(not found)".dimmed()
                    }
                );
            }
            println!();

            // Risk level
            println!(
                "{} = {:?}\n",
                "risk_level".bright_yellow(),
                config.risk_level
            );

            // AI section
            print_section(
                "ai",
                &[
                    ("provider", config.ai.provider.clone()),
                    ("model", config.ai.model.clone()),
                    ("max_tokens", config.ai.max_tokens.to_string()),
                    ("temperature", config.ai.temperature.to_string()),
                    ("timeout_secs", config.ai.timeout_secs.to_string()),
                    ("streaming", config.ai.streaming.to_string()),
                ],
            );

            // Execution section
            print_section(
                "execution",
                &[
                    (
                        "command_timeout_secs",
                        config.execution.command_timeout_secs.to_string(),
                    ),
                    ("auto_rollback", config.execution.auto_rollback.to_string()),
                    ("shell", config.execution.shell.clone()),
                    ("dry_run", config.execution.dry_run.to_string()),
                ],
            );

            // Session section
            print_section(
                "session",
                &[
                    (
                        "storage_dir",
                        config.session.storage_dir.display().to_string(),
                    ),
                    ("max_age_days", config.session.max_age_days.to_string()),
                    (
                        "auto_checkpoint",
                        config.session.auto_checkpoint.to_string(),
                    ),
                ],
            );

            // Display section
            print_section(
                "display",
                &[
                    ("colors", config.display.colors.to_string()),
                    ("emoji", config.display.emoji.to_string()),
                    ("verbose", config.display.verbose.to_string()),
                    ("theme", config.display.theme.clone()),
                ],
            );

            // Verification section
            print_section(
                "verification",
                &[
                    ("auto_verify", config.verification.auto_verify.to_string()),
                    ("run_tests", config.verification.run_tests.to_string()),
                    (
                        "timeout_secs",
                        config.verification.timeout_secs.to_string(),
                    ),
                ],
            );

            // Custom section (if any)
            if !config.custom.is_empty() {
                let items: Vec<(&str, String)> = config
                    .custom
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.to_string()))
                    .collect();
                print_section("custom", &items);
            }

            println!(
                "Use {} to set a value",
                "ganesha config <key> <value>".bright_green()
            );
        }

        (Some(key), None) => {
            // Show specific key
            match get_config_value(&config, &key) {
                Some(value) => {
                    println!("{} = {}", key.bright_yellow(), value.bright_white());
                }
                None => {
                    println!("{} Unknown config key: {}", "Error:".red(), key);
                    println!("\nAvailable keys:");
                    println!("  {} - Risk level (safe/normal/trusted/yolo)", "risk_level".bright_yellow());
                    println!("  {} - AI provider", "ai.provider".bright_yellow());
                    println!("  {} - AI model", "ai.model".bright_yellow());
                    println!("  {} - Maximum response tokens", "ai.max_tokens".bright_yellow());
                    println!("  {} - Generation temperature", "ai.temperature".bright_yellow());
                    println!("  {} - Dry run mode", "execution.dry_run".bright_yellow());
                    println!("  {} - Color output", "display.colors".bright_yellow());
                    println!("  ... and more");
                }
            }
        }

        (Some(key), Some(value)) => {
            // Set value
            match set_config_value(&mut config, &key, &value) {
                Ok(()) => {
                    // Save config
                    let save_path = CoreConfig::project_config_path()
                        .unwrap_or_else(|| std::path::PathBuf::from(".ganesha/config.toml"));

                    if let Err(e) = config.save_to_file(&save_path) {
                        println!(
                            "{} Failed to save config: {}",
                            "Warning:".yellow(),
                            e
                        );
                        println!(
                            "Value was set for this session only. Create {} manually to persist.",
                            save_path.display()
                        );
                    } else {
                        println!(
                            "{} {} = {}",
                            "✓".green(),
                            key.bright_yellow(),
                            value.bright_green()
                        );
                        println!("Saved to {}", save_path.display().to_string().dimmed());
                    }
                }
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                }
            }
        }

        (None, Some(_)) => {
            println!("{} Must specify a key to set a value", "Error:".red());
            println!("Usage: ganesha config <key> <value>");
        }
    }

    Ok(())
}
