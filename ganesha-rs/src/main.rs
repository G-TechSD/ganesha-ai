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
mod agent_wiggum;
mod cli;
mod core;
mod logging;
mod menu;
mod providers;
mod orchestrator;
mod tui;
mod workflow;

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

The original AI-powered system control tool.
Predates Claude Code, OpenAI Codex CLI, and Gemini CLI.
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

    /// Interactive REPL mode (default when no task given)
    #[arg(short, long, default_value_t = true)]
    interactive: bool,

    /// Non-interactive mode (run task and exit)
    #[arg(long)]
    no_interactive: bool,

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

    /// Run test harness with 200 edge cases
    #[arg(long)]
    test: bool,

    /// Wiggum agent mode with verification loop
    #[arg(long)]
    wiggum: bool,

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

    // Check if ganesha is available system-wide
    check_and_install_system_wide();

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

    // Test mode - run 200 edge case tests
    if args.test {
        let (provider_url, model) = chain.get_first_available_url()
            .unwrap_or_else(|| ("http://192.168.245.155:1234".to_string(), "default".to_string()));

        println!("\n{}", style("═".repeat(60)).dim());
        println!("{}", style("Starting Test Harness...").cyan().bold());
        println!("{}", style("═".repeat(60)).dim());

        let mut harness = agent_wiggum::TestHarness::new(&provider_url, &model);
        let _results = harness.run_all_tests().await;
        return;
    }

    // Wiggum agent mode - with verification loop
    if args.wiggum {
        let (provider_url, model) = chain.get_first_available_url()
            .unwrap_or_else(|| ("http://192.168.245.155:1234".to_string(), "default".to_string()));

        let config = agent_wiggum::AgentConfig {
            provider_url,
            model,
            auto_approve: args.auto,
            verify_actions: true,
            verbose: !args.quiet,
            ..Default::default()
        };

        let mut agent = agent_wiggum::WiggumAgent::new(config);

        if !task.is_empty() {
            match agent.run_task(&task).await {
                Ok(result) => {
                    println!("\n{}", style(&result.final_response).cyan());
                    if !args.quiet {
                        println!("\n{}", style(format!(
                            "Completed {} actions in {:?}",
                            result.actions.len(),
                            result.duration
                        )).dim());
                    }
                }
                Err(e) => {
                    print_error(&format!("Agent error: {}", e));
                }
            }
        } else {
            // Interactive wiggum mode
            println!("\n{}", style("Wiggum Agent Mode - with verification").cyan().bold());
            println!("{}", style("Enter tasks or 'exit' to quit").dim());

            let config_for_repl = rustyline::Config::builder()
                .edit_mode(rustyline::EditMode::Emacs)
                .build();
            let mut rl = rustyline::DefaultEditor::with_config(config_for_repl).unwrap();

            loop {
                match rl.readline("wiggum> ") {
                    Ok(line) => {
                        let input = line.trim();
                        if input.is_empty() { continue; }
                        if input == "exit" || input == "quit" {
                            println!("{}", style("Namaste 🙏").yellow());
                            break;
                        }

                        match agent.run_task(input).await {
                            Ok(result) => {
                                println!("\n{}", style(&result.final_response).cyan());
                            }
                            Err(e) => {
                                print_error(&format!("Error: {}", e));
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        return;
    }

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

    // Determine if we should enter interactive mode
    let should_be_interactive = !args.no_interactive && (args.interactive || task.is_empty());

    // Create engine with appropriate consent handler
    if args.auto {
        let mut engine = GaneshaEngine::new(chain, AutoConsent, policy);
        engine.auto_approve = true;

        // Process initial task if provided
        if !task.is_empty() {
            run_task(&mut engine, &task, args.code).await;
        }

        // Enter REPL if interactive
        if should_be_interactive {
            run_repl(&mut engine, args.code).await;
        }
    } else {
        let mut engine = GaneshaEngine::new(chain, CliConsent::new(), policy);

        // Process initial task if provided
        if !task.is_empty() {
            run_task(&mut engine, &task, args.code).await;
            // If task was provided and --no-interactive, exit
            if args.no_interactive {
                return;
            }
        }

        // Enter REPL for interactive experience
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
    use workflow::{WorkflowEngine, GaneshaMode};

    // Initialize workflow engine
    let mut workflow = WorkflowEngine::new();

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
    println!("{}", style("Interactive mode. Type /menu for commands, Ctrl+C twice to exit.").dim());
    println!("{}\n", style("─".repeat(60)).dim());

    loop {
        // Show current mode in prompt
        let mode_indicator = format!("[{}]", workflow.current_mode.display_name());
        let prompt = format!("{} {} ",
            style(mode_indicator).dim(),
            style("ganesha>").cyan().bold()
        );

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

                // Handle menu
                if input == "/menu" || input == "/help" || input == "help" {
                    println!("\n{}", style("╔══════════════════════════════════════════════════════════╗").cyan());
                    println!("{}", style("║                    GANESHA MENU                          ║").cyan().bold());
                    println!("{}", style("╚══════════════════════════════════════════════════════════╝").cyan());

                    println!("\n{}", style("WORKFLOW MODES:").yellow().bold());
                    println!("  /chat          Switch to Chat mode (Q&A, discussion)");
                    println!("  /plan          Switch to Planning mode (careful analysis)");
                    println!("  /dev           Switch to Development mode (Wiggum loop)");
                    println!("  /test          Switch to Testing mode");
                    println!("  /fix           Switch to Fix/Refine mode");
                    println!("  /eval          Switch to Evaluation mode");
                    println!("  /sysadmin      Switch to SysAdmin mode (system tasks)");

                    println!("\n{}", style("MEMORY & SESSION:").yellow().bold());
                    println!("  /recall        Show conversation history");
                    println!("  /clear         Clear conversation history");
                    println!("  /session-status Show full session & workflow status");
                    println!("  /log [file]    Save session transcript to file");

                    println!("\n{}", style("SETTINGS & CONFIGURATION:").yellow().bold());
                    println!("  /settings      Open settings menu:");
                    println!("                 • Providers & Models");
                    println!("                 • Vision settings");
                    println!("                 • MCP Servers");
                    println!("                 • Permissions");

                    println!("\n{}", style("CONTEXT:").yellow().bold());
                    println!("  /pwd           Show current working directory");
                    println!("  /mode          Show current mode only");

                    println!("\n{}", style("INFO & FEEDBACK:").yellow().bold());
                    println!("  /about         About Ganesha and its history");
                    println!("  /feedback      Send feedback to G-Tech SD");

                    println!("\n{}", style("EXIT:").yellow().bold());
                    println!("  exit, quit     Exit Ganesha");
                    println!("  Ctrl+C twice   Force exit\n");
                    continue;
                }

                // Handle about command
                if input == "/about" {
                    println!("\n{}", style("═".repeat(70)).cyan());
                    println!("{}", style("                    ABOUT GANESHA").cyan().bold());
                    println!("{}", style("         The Original AI-Powered System Control Tool").dim());
                    println!("{}\n", style("═".repeat(70)).cyan());

                    println!("{}", style("ORIGIN STORY").yellow().bold());
                    println!("{}", style("─".repeat(70)).dim());
                    println!(r#"
Ganesha was first developed and working in August 2024 by Bill Griffith
of G-Tech SD in California. Built at his home, it started as a tool for
developing robotic software and configuring Raspberry Pi computers.

As an IT services provider, Bill wanted to make system administration
easier and more automated. He realized AI had finally reached the point
where it could handle these tasks - but the workflow was painful. Copying
code and terminal commands from ChatGPT, then copying errors back, meant
thousands of manual actions that slowed progress to a snail's pace.

Bill had been "vibe coding" since ChatGPT 3.5 - before anyone called it
that, and before most people dared to try or believed it was possible.
Using Ganesha, he managed to write several complete robot operation tools
that worked better than expected, dramatically speeding up deployment of
new code and features.
"#);

                    println!("{}", style("THE BREAKTHROUGH").yellow().bold());
                    println!("{}", style("─".repeat(70)).dim());
                    println!(r#"
Ganesha wasn't just faster - it started doing everything Bill had always
dreamed of, without the constant roadblocks and research for every little
task. Unable to find anything like it anywhere, he posted it on GitHub.

It got little attention at first. But six months later, Bill saw a YouTube
video about OpenAI Codex CLI being released. Then Claude Code. Then Gemini
CLI. They all worked exactly like Ganesha - the same consent flows, agentic
tool use with dynamic scripting, feedback loops from command outputs back
into prompts for troubleshooting, planning long-horizon installs... all
controlled by plain English instead of esoteric commands that can break
everything if used improperly.

Ganesha wasn't perfect at first, but it was reliable and harmless enough
to officially earn its name: The Remover of Obstacles.
"#);

                    println!("{}", style("GANESHA 2.0 - JANUARY 2026").yellow().bold());
                    println!("{}", style("─".repeat(70)).dim());
                    println!(r#"
Ganesha 2 is a complete rewrite in Rust - blazingly fast, memory-safe,
and way easier to get up and running. Single binary, no dependencies,
no Python environment headaches. Just download and run.

Feature set that rivals (and often exceeds) the big players:

  • Multi-provider support (local + cloud with priority fallback)
  • Conversation memory and context awareness
  • Workflow modes (Planning, Development, Testing, Evaluation)
  • Wiggum verification loop for autonomous task completion
  • Vision model support for screenshot analysis
  • MCP server integration
  • Git expertise built-in
  • BIOS-style provider priority configuration
  • Auto-detection of available models from local servers

Coming soon: Claudia Coder - an autonomous development platform that
harnesses all the best cloud providers and Ganesha 2 itself to create
long-horizon app development projects, completing them end-to-end
including testing and UX iteration, producing clean, polished apps
with real GitLab repositories and documentation.
"#);

                    println!("{}", style("─".repeat(70)).dim());
                    println!("{}", style("Bill hopes you enjoy using these tools and that you make things").italic());
                    println!("{}", style("that improve your life or career while making the world better.").italic());
                    println!("\n{}", style("© 2024-2026 G-Tech SD, California").dim());
                    println!("{}", style("https://github.com/gtechsd/ganesha").dim());
                    println!("{}\n", style("═".repeat(70)).cyan());
                    continue;
                }

                // Handle feedback command
                if input == "/feedback" {
                    println!("\n{}", style("═".repeat(60)).cyan());
                    println!("{}", style("        SEND FEEDBACK TO G-TECH SD").cyan().bold());
                    println!("{}\n", style("═".repeat(60)).cyan());

                    println!("{}", style("We'd love to hear from you! Your feedback helps make Ganesha better.").dim());
                    println!();

                    // Get feedback type
                    println!("{}", style("What type of feedback?").bold());
                    println!("  [1] Bug report");
                    println!("  [2] Feature request");
                    println!("  [3] General feedback");
                    println!("  [4] Success story");
                    println!();

                    print!("{} ", style("Select (1-4):").cyan());
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    let mut feedback_type = String::new();
                    if std::io::stdin().read_line(&mut feedback_type).is_err() {
                        continue;
                    }

                    let feedback_type = match feedback_type.trim() {
                        "1" => "bug",
                        "2" => "feature",
                        "3" => "general",
                        "4" => "success",
                        _ => {
                            println!("{} Cancelled.", style("⚠").yellow());
                            continue;
                        }
                    };

                    println!("\n{}", style("Enter your feedback (press Enter twice to submit):").bold());
                    let mut feedback_text = String::new();
                    let mut empty_lines = 0;

                    loop {
                        let mut line = String::new();
                        if std::io::stdin().read_line(&mut line).is_err() {
                            break;
                        }
                        if line.trim().is_empty() {
                            empty_lines += 1;
                            if empty_lines >= 2 {
                                break;
                            }
                        } else {
                            empty_lines = 0;
                        }
                        feedback_text.push_str(&line);
                    }

                    let feedback_text = feedback_text.trim();
                    if feedback_text.is_empty() {
                        println!("{} No feedback entered. Cancelled.", style("⚠").yellow());
                        continue;
                    }

                    // Optional email
                    print!("{} ", style("Your email (optional, for follow-up):").cyan());
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    let mut email = String::new();
                    let _ = std::io::stdin().read_line(&mut email);
                    let email = email.trim();

                    // Send feedback
                    println!("\n{} Sending feedback...", style("📤").cyan());

                    let feedback_data = serde_json::json!({
                        "type": feedback_type,
                        "message": feedback_text,
                        "email": if email.is_empty() { None } else { Some(email) },
                        "version": "3.0.0",
                        "platform": std::env::consts::OS,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });

                    // Try to send to G-Tech SD feedback endpoint
                    let client = reqwest::blocking::Client::builder()
                        .timeout(std::time::Duration::from_secs(10))
                        .build();

                    let sent = if let Ok(client) = client {
                        // Primary endpoint
                        let result = client
                            .post("https://api.gtechsd.com/ganesha/feedback")
                            .json(&feedback_data)
                            .send();

                        match result {
                            Ok(resp) if resp.status().is_success() => true,
                            _ => {
                                // Fallback: try alternative endpoint
                                let fallback = client
                                    .post("https://ganesha-feedback.gtechsd.workers.dev")
                                    .json(&feedback_data)
                                    .send();
                                matches!(fallback, Ok(r) if r.status().is_success())
                            }
                        }
                    } else {
                        false
                    };

                    if sent {
                        println!("{} Feedback sent successfully! Thank you!", style("✓").green());
                    } else {
                        // Save locally if can't send
                        let feedback_file = format!("ganesha-feedback-{}.json",
                            chrono::Local::now().format("%Y%m%d-%H%M%S"));
                        if let Ok(_) = std::fs::write(&feedback_file, feedback_data.to_string()) {
                            println!("{} Could not connect to server.", style("⚠").yellow());
                            println!("  Feedback saved to: {}", feedback_file);
                            println!("  Please email to: feedback@gtechsd.com");
                        } else {
                            println!("{} Could not send feedback. Please email:", style("⚠").yellow());
                            println!("  feedback@gtechsd.com");
                        }
                    }
                    println!();
                    continue;
                }

                // Handle mode command - just show current mode
                if input == "/mode" {
                    println!("{} Current mode: {}", style("→").cyan(), style(workflow.current_mode.display_name()).bold());
                    continue;
                }

                // Handle session-status - full workflow status
                if input == "/session-status" || input == "/status" {
                    println!("\n{}", workflow.status());
                    println!("\n{}", style("Conversation:").bold());
                    println!("  Messages: {}", engine.conversation_history.len());
                    println!("  Working dir: {}", engine.working_directory.display());
                    continue;
                }

                if input == "/chat" {
                    workflow.force_transition(GaneshaMode::Chat);
                    println!("{} Switched to Chat mode", style("💬").cyan());
                    continue;
                }

                if input == "/plan" {
                    if let Err(e) = workflow.transition(GaneshaMode::Planning) {
                        println!("{} {}", style("⚠").yellow(), e);
                    } else {
                        println!("{} Switched to Planning mode - careful analysis before development", style("📋").cyan());
                    }
                    continue;
                }

                if input == "/dev" {
                    if let Err(e) = workflow.transition(GaneshaMode::Development) {
                        println!("{} {}", style("⚠").yellow(), e);
                    } else {
                        println!("{} Switched to Development mode - Wiggum verification active", style("🔨").cyan());
                    }
                    continue;
                }

                if input == "/test" {
                    if let Err(e) = workflow.transition(GaneshaMode::Testing) {
                        println!("{} {}", style("⚠").yellow(), e);
                    } else {
                        println!("{} Switched to Testing mode - thorough validation", style("🧪").cyan());
                    }
                    continue;
                }

                if input == "/fix" {
                    if let Err(e) = workflow.transition(GaneshaMode::FixRefine) {
                        println!("{} {}", style("⚠").yellow(), e);
                    } else {
                        println!("{} Switched to Fix/Refine mode - fixing test failures", style("🔧").cyan());
                    }
                    continue;
                }

                if input == "/eval" {
                    if let Err(e) = workflow.transition(GaneshaMode::Evaluation) {
                        println!("{} {}", style("⚠").yellow(), e);
                    } else {
                        println!("{} Switched to Evaluation mode - final quality check", style("✅").cyan());
                    }
                    continue;
                }

                if input == "/sysadmin" {
                    if let Err(e) = workflow.transition(GaneshaMode::SysAdmin) {
                        println!("{} {}", style("⚠").yellow(), e);
                    } else {
                        println!("{} Switched to SysAdmin mode - system configuration", style("⚙️").cyan());
                    }
                    continue;
                }

                // Auto-detect and switch mode from input
                if let Some(detected_mode) = workflow.detect_mode(input) {
                    if detected_mode != workflow.current_mode {
                        if workflow.auto_transition(detected_mode) {
                            println!("{} {} Auto-switched to {} mode",
                                detected_mode.emoji(),
                                style("→").dim(),
                                style(detected_mode.display_name()).cyan().bold()
                            );
                        }
                    }
                }

                // Handle conversation memory commands
                if input == "/recall" {
                    println!("\n{}", engine.get_conversation_summary());
                    continue;
                }

                if input == "/clear" {
                    engine.clear_history();
                    println!("{} Conversation history cleared", style("✓").green());
                    continue;
                }

                if input == "/pwd" {
                    println!("{} Working directory: {}", style("📁").cyan(), engine.working_directory.display());
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

                // Handle config/settings
                if input == "/config" || input == "/settings" {
                    menu::show_settings_menu();
                    // Redraw header after settings menu
                    println!("\n{}", style("─".repeat(60)).dim());
                    println!("{}", style("Back to interactive mode.").dim());
                    println!("{}\n", style("─".repeat(60)).dim());
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

/// Check if ganesha is available system-wide and offer to install if not
fn check_and_install_system_wide() {
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Check if 'ganesha' is already in PATH
    if let Ok(output) = std::process::Command::new("which").arg("ganesha").output() {
        if output.status.success() {
            // Already installed
            return;
        }
    }

    // Get current executable path
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(_) => return, // Can't determine current exe
    };

    // Check if we're already in a system location
    let exe_str = current_exe.to_string_lossy();
    if exe_str.contains("/usr/") || exe_str.contains("/bin/") {
        return; // Already in a system location
    }

    // Create marker file to track if we've asked before
    let marker_path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ganesha")
        .join(".install_offered");

    if marker_path.exists() {
        return; // Already offered
    }

    // Create the marker directory
    if let Some(parent) = marker_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    println!("\n{}", style("═".repeat(60)).dim());
    println!("{}", style("Ganesha First Run Setup").cyan().bold());
    println!("{}\n", style("═".repeat(60)).dim());

    println!("{}", style("Would you like to install 'ganesha' command system-wide?").yellow());
    println!("{}", style("This will copy the binary to /usr/local/bin/").dim());
    println!();

    print!("{} ", style("[Y/n]:").cyan());
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        let _ = fs::write(&marker_path, "skipped");
        return;
    }

    let input = input.trim().to_lowercase();
    if input.is_empty() || input == "y" || input == "yes" {
        // Try to install to /usr/local/bin
        let install_path = PathBuf::from("/usr/local/bin/ganesha");

        // Check if we need sudo
        let needs_sudo = !fs::metadata("/usr/local/bin")
            .map(|m| m.permissions().mode() & 0o200 != 0)
            .unwrap_or(false);

        let result = if needs_sudo {
            // Use sudo to copy
            std::process::Command::new("sudo")
                .args(["cp", &current_exe.to_string_lossy(), "/usr/local/bin/ganesha"])
                .status()
                .and_then(|_| {
                    std::process::Command::new("sudo")
                        .args(["chmod", "+x", "/usr/local/bin/ganesha"])
                        .status()
                })
        } else {
            // Direct copy
            fs::copy(&current_exe, &install_path)
                .map(|_| std::process::ExitStatus::default())
        };

        match result {
            Ok(status) if status.success() || status.code().is_none() => {
                println!("\n{} Installed to /usr/local/bin/ganesha", style("✓").green());
                println!("{}", style("You can now run 'ganesha' from anywhere!").dim());
            }
            Ok(_) | Err(_) => {
                println!("\n{} Installation failed. You can manually copy:", style("⚠").yellow());
                println!("  sudo cp {} /usr/local/bin/ganesha", current_exe.display());
            }
        }
    } else {
        println!("\n{} Skipped installation.", style("ℹ").cyan());
        println!("{}", style("You can install later with:").dim());
        println!("  sudo cp {} /usr/local/bin/ganesha", current_exe.display());
    }

    // Mark as offered
    let _ = fs::write(&marker_path, "offered");
    println!();
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
