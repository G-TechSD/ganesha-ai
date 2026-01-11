//! Ganesha - The Remover of Obstacles
//!
//! AI-powered system control. Local-first, safe by default.
//!
//! ```bash
//! ganesha "install docker"
//! ganesha --auto "update all packages"
//! ganesha --interactive
//! ```

mod agent;
mod cli;
mod core;
mod logging;
mod providers;
mod orchestrator;
mod tui;

use clap::{Parser, Subcommand};
use cli::{print_banner, print_error, print_info, print_action_summary, print_success, AutoConsent, CliConsent};
use console::style;
use core::access_control::{load_policy, AccessLevel};
use core::GaneshaEngine;
use providers::ProviderChain;
use orchestrator::providers::ProviderManager;
use chrono::Local;

#[derive(Parser)]
#[command(name = "ganesha")]
#[command(author = "G-Tech SD")]
#[command(version = "3.0.0")]
#[command(about = "The Remover of Obstacles - AI-Powered System Control")]
#[command(long_about = r#"
Ganesha translates natural language into safe, executable system commands.

Examples:
  ganesha "install docker"
  ganesha --auto "update all packages"
  ganesha --code "create a React login form"
  ganesha --rollback
  ganesha --interactive

The first AI-powered system control tool.
Predates Claude Code & OpenAI Codex CLI.
"#)]
struct Args {
    /// Task in plain English
    #[arg(trailing_var_arg = true)]
    task: Vec<String>,

    /// Auto-approve all commands (DANGEROUS)
    #[arg(short = 'A', long)]
    auto: bool,

    /// Code generation mode
    #[arg(long)]
    code: bool,

    /// Interactive REPL mode
    #[arg(short, long)]
    interactive: bool,

    /// Agent mode - full coding assistant with tool use
    #[arg(long)]
    agent: bool,

    /// Rollback session
    #[arg(short, long)]
    rollback: Option<Option<String>>,

    /// Show session history
    #[arg(long)]
    history: bool,

    /// LLM provider
    #[arg(long, value_parser = ["local", "anthropic", "openai"])]
    provider: Option<String>,

    /// Show debug output
    #[arg(long)]
    debug: bool,

    /// Minimal output
    #[arg(short, long)]
    quiet: bool,

    /// Configure providers and tiers
    #[arg(long)]
    configure: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure access control
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set access level
    SetLevel {
        #[arg(value_parser = ["restricted", "standard", "elevated", "full_access", "whitelist", "blacklist"])]
        level: String,
    },
    /// Test if a command would be allowed
    Test {
        command: String,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize tracing for debug output
    if args.debug {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    // Handle subcommands
    if let Some(cmd) = args.command {
        match cmd {
            Commands::Config { action } => {
                handle_config(action);
                return;
            }
        }
    }

    // Check for first-run setup or explicit --configure
    let mut provider_manager = ProviderManager::new();

    if args.configure {
        // Explicit configuration request
        if let Err(e) = provider_manager.first_run_setup().await {
            print_error(&format!("Setup failed: {}", e));
            std::process::exit(1);
        }
        return;
    }

    if provider_manager.needs_setup() {
        // First run - do setup
        println!("\x1b[33mFirst run detected - starting setup...\x1b[0m\n");
        if let Err(e) = provider_manager.first_run_setup().await {
            print_error(&format!("Setup failed: {}", e));
            std::process::exit(1);
        }

        // If user just ran setup with no task, exit
        if args.task.is_empty() && !args.interactive {
            return;
        }
    }

    // Print banner unless quiet
    if !args.quiet {
        print_banner();
    }

    // Get task
    let task = args.task.join(" ");

    // Load policy
    let policy = load_policy();

    // Create provider chain (TODO: migrate to ProviderManager)
    let chain = ProviderChain::default_chain();
    let available = chain.get_available();

    if available.is_empty() {
        print_error("No LLM providers available");
        print_info("Run: ganesha --configure");
        std::process::exit(1);
    }

    print_info(&format!("Provider: {}", available.first().unwrap()));

    // Agent mode - full coding assistant with tool use
    if args.agent {
        let (provider_url, model) = chain.get_first_available_url()
            .unwrap_or_else(|| ("http://192.168.245.155:1234".to_string(), "default".to_string()));

        println!("\n{}", style("─".repeat(60)).dim());
        println!("{}", style("Starting Agent Mode...").cyan().bold());
        println!("{}", style("─".repeat(60)).dim());

        if let Err(e) = agent::run_agent(&provider_url, &model, if task.is_empty() { None } else { Some(&task) }, args.auto).await {
            print_error(&format!("Agent error: {}", e));
            std::process::exit(1);
        }
        return;
    }

    // Create engine with appropriate consent handler
    if args.auto {
        let mut engine = GaneshaEngine::new(chain, AutoConsent, policy);
        engine.auto_approve = true;

        // Process initial task if provided
        if !task.is_empty() {
            run_task(&mut engine, &task, args.code).await;
        }

        // Enter REPL if interactive mode or no task provided
        if args.interactive || task.is_empty() {
            run_repl(&mut engine, args.code).await;
        }
    } else {
        let mut engine = GaneshaEngine::new(chain, CliConsent::new(), policy);

        // Process initial task if provided
        if !task.is_empty() {
            run_task(&mut engine, &task, args.code).await;
        }

        // Always enter REPL for interactive experience
        run_repl(&mut engine, args.code).await;
    }
}

/// Interactive REPL loop with proper line editing
async fn run_repl<C: core::ConsentHandler>(
    engine: &mut GaneshaEngine<ProviderChain, C>,
    code_mode: bool,
) {
    use rustyline::error::ReadlineError;
    use rustyline::{DefaultEditor, Config, EditMode};
    use std::time::Instant;

    // Session log for /log command
    let mut session_log: Vec<String> = vec![
        format!("=== Ganesha Session Started: {} ===", Local::now().format("%Y-%m-%d %H:%M:%S")),
    ];

    // Track Ctrl+C for double-press exit
    let mut last_interrupt: Option<Instant> = None;

    // Configure rustyline with emacs-style editing (arrow keys, etc.)
    let config = Config::builder()
        .edit_mode(EditMode::Emacs)
        .build();

    let mut rl = match DefaultEditor::with_config(config) {
        Ok(editor) => editor,
        Err(e) => {
            print_error(&format!("Failed to initialize readline: {}", e));
            return;
        }
    };

    // Load history if exists
    let history_path = dirs::data_dir()
        .map(|p| p.join("ganesha").join("history.txt"))
        .unwrap_or_else(|| std::path::PathBuf::from(".ganesha_history"));

    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    println!("\n{}", style("─".repeat(60)).dim());
    println!("{}", style("Interactive mode. Ctrl+C twice or 'exit' to leave.").dim());
    println!("{}", style("Commands: /1: /2: /3: (tiers) | /vision: | /log | /help").dim());
    println!("{}\n", style("─".repeat(60)).dim());

    loop {
        let prompt = format!("{} ", style("ganesha>").cyan().bold());

        match rl.readline(&prompt) {
            Ok(line) => {
                // Reset interrupt tracking on successful input
                last_interrupt = None;

                let input = line.trim();

                if input.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(input);

                // Log user input
                session_log.push(format!("[{}] USER: {}", Local::now().format("%H:%M:%S"), input));

                // Handle exit commands
                if matches!(input.to_lowercase().as_str(), "exit" | "quit" | "q" | ":q") {
                    println!("{}", style("Namaste 🙏").yellow());
                    break;
                }

                // Handle help
                if input == "/help" || input == "help" {
                    println!("\n{}", style("Ganesha Commands:").bold());
                    println!("  /1: <task>     - Use fast tier (Haiku)");
                    println!("  /2: <task>     - Use balanced tier (Sonnet)");
                    println!("  /3: <task>     - Use premium tier (Opus)");
                    println!("  /vision: <task> - Use vision model");
                    println!("  /log [file]    - Save session to file");
                    println!("  /config        - Reconfigure providers");
                    println!("  Ctrl+C twice   - Exit Ganesha");
                    println!("  exit, quit     - Exit Ganesha\n");
                    continue;
                }

                // Handle /log command
                if input.to_lowercase().starts_with("/log") {
                    let filename = input.strip_prefix("/log").map(|s| s.trim()).filter(|s| !s.is_empty());
                    let log_file = filename.map(|f| f.to_string()).unwrap_or_else(|| {
                        format!("ganesha-session-{}.log", Local::now().format("%Y%m%d-%H%M%S"))
                    });

                    match std::fs::write(&log_file, session_log.join("\n")) {
                        Ok(_) => {
                            println!("{} Session saved to: {}", style("✓").green(), log_file);
                            session_log.push(format!("[{}] SYSTEM: Session saved to {}", Local::now().format("%H:%M:%S"), log_file));
                        }
                        Err(e) => {
                            println!("{} Failed to save log: {}", style("✗").red(), e);
                        }
                    }
                    continue;
                }

                // Handle config
                if input == "/config" {
                    println!("{}", style("Run: ganesha --configure").dim());
                    continue;
                }

                // Process the task and capture output
                let output = run_task_with_log(engine, input, code_mode).await;
                session_log.push(format!("[{}] GANESHA: {}", Local::now().format("%H:%M:%S"), output));
                println!(); // Add spacing after task completion
            }
            Err(ReadlineError::Interrupted) => {
                // Check for double Ctrl+C
                if let Some(last) = last_interrupt {
                    if last.elapsed().as_secs() < 2 {
                        println!("\n{}", style("Namaste 🙏").yellow());
                        break;
                    }
                }
                last_interrupt = Some(Instant::now());
                println!("{}", style("(Press Ctrl+C again to exit)").dim());
            }
            Err(ReadlineError::Eof) => {
                println!("{}", style("Namaste 🙏").yellow());
                break;
            }
            Err(err) => {
                print_error(&format!("Input error: {}", err));
                break;
            }
        }
    }

    // Save history
    if let Some(parent) = history_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = rl.save_history(&history_path);

    // Add session end
    session_log.push(format!("=== Session Ended: {} ===", Local::now().format("%Y-%m-%d %H:%M:%S")));
}

/// Fun spinner messages for the AI thinking phase
const THINKING_MESSAGES: &[&str] = &[
    "🐘 Ganesha is contemplating...",
    "🔮 Consulting the cosmic trunk...",
    "✨ Removing obstacles from your path...",
    "🧠 Processing with elephant-sized wisdom...",
    "🌟 Channeling divine intelligence...",
    "🎯 Focusing the third eye...",
    "💭 Meditating on your request...",
    "🔥 Igniting the inner flame of knowledge...",
];

/// Fun spinner messages for execution phase
const EXECUTING_MESSAGES: &[&str] = &[
    "⚡ Executing with trunk precision...",
    "🛠️ Ganesha's trunk is at work...",
    "🎪 Performing digital magic...",
    "🚀 Launching your commands...",
    "⚙️ Turning the cosmic gears...",
];

/// Create an entertaining spinner
fn create_spinner(msg: &str) -> indicatif::ProgressBar {
    use indicatif::{ProgressBar, ProgressStyle};

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("🕐🕑🕒🕓🕔🕕🕖🕗🕘🕙🕚🕛")
            .template("{spinner:.cyan} {msg}")
            .unwrap()
    );
    spinner.set_message(msg.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

/// Run a task and return the output for logging
async fn run_task_with_log<C: core::ConsentHandler>(
    engine: &mut GaneshaEngine<ProviderChain, C>,
    task: &str,
    code_mode: bool,
) -> String {
    use rand::seq::SliceRandom;

    let task = if code_mode {
        format!("[CODE MODE] {}", task)
    } else {
        task.to_string()
    };

    // Pick a random thinking message
    let thinking_msg = THINKING_MESSAGES
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"🐘 Thinking...");

    // Show spinner while planning
    let spinner = create_spinner(thinking_msg);

    // Plan
    let plan = match engine.plan(&task).await {
        Ok(p) => {
            spinner.finish_and_clear();
            p
        }
        Err(e) => {
            spinner.finish_and_clear();
            let msg = format!("Planning failed: {}", e);
            print_error(&msg);
            return msg;
        }
    };

    // Execute with different spinner
    let exec_msg = EXECUTING_MESSAGES
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"⚡ Executing...");

    let spinner = create_spinner(exec_msg);

    let mut outputs = vec![];
    match engine.execute(&plan).await {
        Ok(results) => {
            spinner.finish_and_clear();
            for result in results {
                // Check if this is a conversational Response action (no command)
                if result.command.is_empty() && !result.explanation.is_empty() {
                    // This is a Response action - show the actual response nicely
                    println!("\n{}", style(&result.explanation).cyan());
                    outputs.push(result.explanation.clone());
                } else {
                    // This is a command execution - show friendly summary
                    print_action_summary(&result.command, result.success, &result.output, result.duration_ms);
                    outputs.push(result.output.clone());
                }

                if let Some(ref err) = result.error {
                    print_error(err);
                    outputs.push(format!("Error: {}", err));
                }
            }
        }
        Err(e) => {
            spinner.finish_and_clear();
            let msg = format!("{}", e);
            print_error(&msg);
            outputs.push(msg);
        }
    }

    outputs.join("\n")
}

async fn run_task<C: core::ConsentHandler>(
    engine: &mut GaneshaEngine<ProviderChain, C>,
    task: &str,
    code_mode: bool,
) {
    let task = if code_mode {
        format!("[CODE MODE] {}", task)
    } else {
        task.to_string()
    };

    // Plan
    let plan = match engine.plan(&task).await {
        Ok(p) => p,
        Err(e) => {
            print_error(&format!("Planning failed: {}", e));
            return;
        }
    };

    // Execute
    match engine.execute(&plan).await {
        Ok(results) => {
            for result in results {
                // Check if this is a conversational Response action (no command)
                if result.command.is_empty() && !result.explanation.is_empty() {
                    // This is a Response action - show the actual response nicely
                    println!("\n{}", console::style(&result.explanation).cyan());
                } else {
                    // This is a command execution - show friendly summary
                    print_action_summary(&result.command, result.success, &result.output, result.duration_ms);
                }

                if let Some(ref err) = result.error {
                    print_error(err);
                }
            }
        }
        Err(e) => {
            print_error(&format!("{}", e));
        }
    }
}

fn handle_config(action: ConfigAction) {
    use core::access_control::AccessController;

    let policy = load_policy();
    let controller = AccessController::new(policy.clone());

    match action {
        ConfigAction::Show => {
            println!();
            println!("╔═══════════════════════════════════════════════════════════════╗");
            println!("║           GANESHA ACCESS CONTROL CONFIGURATION                ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
            println!();
            println!("  Access Level: {:?}", policy.level);
            println!("  Require approval for high risk: {}", policy.require_approval_for_high_risk);
            println!("  Audit all commands: {}", policy.audit_all_commands);
            println!("  Max execution time: {}s", policy.max_execution_time_secs);
            println!();
            if !policy.whitelist.is_empty() {
                println!("  Whitelist patterns:");
                for p in &policy.whitelist {
                    println!("    + {}", p);
                }
            }
            if !policy.blacklist.is_empty() {
                println!("  Blacklist patterns:");
                for p in &policy.blacklist {
                    println!("    - {}", p);
                }
            }
        }

        ConfigAction::SetLevel { level } => {
            println!("Access level set to: {}", level);
            println!("(Config persistence not yet implemented in Rust version)");
        }

        ConfigAction::Test { command } => {
            println!();
            println!("Testing command: {}", command);
            println!("Current policy: {:?}", policy.level);
            println!();

            let result = controller.check_command(&command);

            if result.allowed {
                println!(
                    "{} [{}]",
                    console::style("✓ ALLOWED").green().bold(),
                    result.risk_level
                );
            } else {
                println!("{}", console::style("✗ DENIED").red().bold());
            }
            println!("Reason: {}", result.reason);
        }
    }
}
