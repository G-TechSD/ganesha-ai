//! # REPL (Read-Eval-Print Loop)
//!
//! Interactive command-line interface for Ganesha.

use crate::cli::{ChatMode, Cli};
use crate::commands;
use crate::render::{self, Style};
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::{Config, Editor, history::FileHistory};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Slash command definition
struct SlashCommand {
    name: &'static str,
    aliases: &'static [&'static str],
    description: &'static str,
    handler: fn(&str, &mut ReplState) -> anyhow::Result<()>,
}

/// REPL state
pub struct ReplState {
    pub mode: ChatMode,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub history: Vec<(String, String)>,
    pub working_dir: PathBuf,
    pub session_id: Option<String>,
}

impl ReplState {
    pub fn new(cli: &Cli) -> Self {
        Self {
            mode: cli.mode,
            model: cli.model.clone(),
            provider: cli.provider.clone(),
            history: Vec::new(),
            working_dir: cli
                .directory
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
            session_id: None,
        }
    }
}

/// Available slash commands
const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "help",
        aliases: &["h", "?"],
        description: "Show this help message",
        handler: cmd_help,
    },
    SlashCommand {
        name: "mode",
        aliases: &["m"],
        description: "Switch chat mode (code/ask/architect/help)",
        handler: cmd_mode,
    },
    SlashCommand {
        name: "model",
        aliases: &[],
        description: "Switch model",
        handler: cmd_model,
    },
    SlashCommand {
        name: "clear",
        aliases: &["c"],
        description: "Clear conversation history",
        handler: cmd_clear,
    },
    SlashCommand {
        name: "undo",
        aliases: &["u"],
        description: "Undo last file change",
        handler: cmd_undo,
    },
    SlashCommand {
        name: "diff",
        aliases: &["d"],
        description: "Show recent changes",
        handler: cmd_diff,
    },
    SlashCommand {
        name: "git",
        aliases: &["g"],
        description: "Run a git command",
        handler: cmd_git,
    },
    SlashCommand {
        name: "commit",
        aliases: &[],
        description: "Commit changes with AI-generated message",
        handler: cmd_commit,
    },
    SlashCommand {
        name: "add",
        aliases: &["a"],
        description: "Add files to context",
        handler: cmd_add,
    },
    SlashCommand {
        name: "drop",
        aliases: &[],
        description: "Remove files from context",
        handler: cmd_drop,
    },
    SlashCommand {
        name: "ls",
        aliases: &["files"],
        description: "List files in context",
        handler: cmd_ls,
    },
    SlashCommand {
        name: "mcp",
        aliases: &["tools"],
        description: "List MCP tools",
        handler: cmd_mcp,
    },
    SlashCommand {
        name: "session",
        aliases: &["s"],
        description: "Session management",
        handler: cmd_session,
    },
    SlashCommand {
        name: "exit",
        aliases: &["quit", "q"],
        description: "Exit Ganesha",
        handler: cmd_exit,
    },
];

/// Run the interactive REPL
pub async fn run(cli: &Cli) -> anyhow::Result<()> {
    let mut state = ReplState::new(cli);

    // Print welcome message
    print_welcome(&state);

    // Set up readline with history
    let config = Config::builder()
        .history_ignore_space(true)
        .auto_add_history(true)
        .build();

    let history_path = dirs::data_dir()
        .map(|d| d.join("ganesha").join("history.txt"))
        .unwrap_or_else(|| PathBuf::from(".ganesha_history"));

    let mut rl: Editor<(), FileHistory> = Editor::with_config(config)?;
    let _ = rl.load_history(&history_path);

    loop {
        let prompt = get_prompt(&state);
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Check for slash commands
                if line.starts_with('/') {
                    match handle_slash_command(line, &mut state) {
                        Ok(should_exit) => {
                            if should_exit {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("{} {}", "Error:".red().bold(), e);
                        }
                    }
                    continue;
                }

                // Regular message - send to LLM
                match send_message(line, &mut state).await {
                    Ok(response) => {
                        render::print_assistant_message(&response);
                        state.history.push((line.to_string(), response));
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red().bold(), e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    if let Some(parent) = history_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = rl.save_history(&history_path);

    Ok(())
}

/// Print welcome message
fn print_welcome(state: &ReplState) {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "\n{} {} - {}\n",
        "🐘 Ganesha".bright_magenta().bold(),
        format!("v{}", version).dimmed(),
        "AI Coding Assistant".bright_cyan()
    );
    println!(
        "Mode: {}  |  Type {} for commands\n",
        format!("{:?}", state.mode).bright_yellow(),
        "/help".bright_green()
    );
}

/// Get the prompt string based on current mode
fn get_prompt(state: &ReplState) -> String {
    let mode_indicator = match state.mode {
        ChatMode::Code => "code".bright_green(),
        ChatMode::Ask => "ask".bright_blue(),
        ChatMode::Architect => "arch".bright_yellow(),
        ChatMode::Help => "help".bright_magenta(),
    };
    format!("{} {} ", mode_indicator, ">".bright_white())
}

/// Handle a slash command
fn handle_slash_command(line: &str, state: &mut ReplState) -> anyhow::Result<bool> {
    let parts: Vec<&str> = line[1..].splitn(2, ' ').collect();
    let cmd_name = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| *s).unwrap_or("");

    // Find matching command
    for cmd in SLASH_COMMANDS {
        if cmd.name == cmd_name || cmd.aliases.contains(&cmd_name.as_str()) {
            (cmd.handler)(args, state)?;
            return Ok(cmd.name == "exit");
        }
    }

    // Check for custom commands (from .ganesha/commands/)
    // TODO: Load custom TOML commands

    println!("{} Unknown command: /{}", "?".yellow(), cmd_name);
    println!("Type {} for available commands", "/help".bright_green());
    Ok(false)
}

/// Send a message to the LLM
async fn send_message(message: &str, state: &mut ReplState) -> anyhow::Result<String> {
    // TODO: Implement actual LLM call
    // For now, return a placeholder
    debug!("Sending message: {}", message);

    // This would use ganesha_core::GaneshaEngine
    Ok(format!(
        "I received your message in {:?} mode: \"{}\"

This is a placeholder response. The actual implementation will:
- Use the configured provider and model
- Apply the current chat mode ({:?})
- Manage context and conversation history
- Handle tool calls and file modifications",
        state.mode, message, state.mode
    ))
}

// Slash command handlers

fn cmd_help(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    println!("\n{}\n", "Available Commands".bright_cyan().bold());
    for cmd in SLASH_COMMANDS {
        let aliases = if cmd.aliases.is_empty() {
            String::new()
        } else {
            format!(" ({})", cmd.aliases.join(", ")).dimmed().to_string()
        };
        println!(
            "  {}{} - {}",
            format!("/{}", cmd.name).bright_green(),
            aliases,
            cmd.description
        );
    }
    println!();
    Ok(())
}

fn cmd_mode(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    let mode = args.trim().to_lowercase();
    state.mode = match mode.as_str() {
        "code" | "c" => ChatMode::Code,
        "ask" | "a" => ChatMode::Ask,
        "architect" | "arch" => ChatMode::Architect,
        "help" | "h" => ChatMode::Help,
        "" => {
            println!("Current mode: {:?}", state.mode);
            println!("Available: code, ask, architect, help");
            return Ok(());
        }
        _ => {
            println!("Unknown mode: {}", mode);
            println!("Available: code, ask, architect, help");
            return Ok(());
        }
    };
    println!("Switched to {:?} mode", state.mode);
    Ok(())
}

fn cmd_model(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    let model = args.trim();
    if model.is_empty() {
        if let Some(ref m) = state.model {
            println!("Current model: {}", m);
        } else {
            println!("Using default model");
        }
        return Ok(());
    }
    state.model = Some(model.to_string());
    println!("Switched to model: {}", model);
    Ok(())
}

fn cmd_clear(_args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    state.history.clear();
    println!("Conversation cleared");
    Ok(())
}

fn cmd_undo(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    // TODO: Implement rollback
    println!("Undo not yet implemented");
    Ok(())
}

fn cmd_diff(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    // TODO: Show recent file changes
    println!("Diff not yet implemented");
    Ok(())
}

fn cmd_git(args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    // Run git command
    let output = std::process::Command::new("git")
        .args(args.split_whitespace())
        .output()?;

    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(())
}

fn cmd_commit(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    // TODO: Generate commit message with AI
    println!("Commit not yet implemented");
    Ok(())
}

fn cmd_add(args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    let files: Vec<&str> = args.split_whitespace().collect();
    if files.is_empty() {
        println!("Usage: /add <file1> [file2] ...");
        return Ok(());
    }
    println!("Added {} file(s) to context", files.len());
    // TODO: Actually add to context
    Ok(())
}

fn cmd_drop(args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    let files: Vec<&str> = args.split_whitespace().collect();
    if files.is_empty() {
        println!("Usage: /drop <file1> [file2] ...");
        return Ok(());
    }
    println!("Removed {} file(s) from context", files.len());
    // TODO: Actually remove from context
    Ok(())
}

fn cmd_ls(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    // TODO: List files in context
    println!("No files in context");
    Ok(())
}

fn cmd_mcp(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    // TODO: List MCP tools
    println!("MCP tools not yet connected");
    Ok(())
}

fn cmd_session(args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().map(|s| *s).unwrap_or("");

    match action {
        "list" | "ls" => {
            // TODO: List sessions
            println!("No saved sessions");
        }
        "save" => {
            // TODO: Save current session
            println!("Session saved");
        }
        "resume" => {
            if let Some(id) = parts.get(1) {
                // TODO: Resume session
                println!("Resuming session: {}", id);
            } else {
                println!("Usage: /session resume <id>");
            }
        }
        _ => {
            println!("Usage: /session [list|save|resume <id>]");
        }
    }
    Ok(())
}

fn cmd_exit(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    println!("Goodbye! 🐘");
    Ok(())
}
