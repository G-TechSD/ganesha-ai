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
mod comprehensive_test;
mod core;
mod flux;
mod logging;
mod menu;
mod providers;
mod orchestrator;
mod tui;
mod workflow;

use clap::{Parser, Subcommand};
use cli::{print_banner, print_error, print_info, print_warning, print_action_summary, print_success, AutoConsent, CliConsent};
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

    /// Run test harness (40 tests per session, default 1 session)
    #[arg(long)]
    test: bool,

    /// Number of test sessions to run (default 1, max 40)
    #[arg(long, value_name = "SESSIONS", default_value = "1")]
    test_sessions: usize,

    /// Wiggum agent mode with verification loop
    #[arg(long)]
    wiggum: bool,

    /// Flux Capacitor: Run for specified duration (e.g., "1h", "30m", "2 hours", "auto")
    #[arg(long, value_name = "DURATION")]
    flux: Option<String>,

    /// Flux Capacitor: Run until specified time (e.g., "11:11", "23:30", "11:11 PM")
    #[arg(long, value_name = "TIME")]
    until: Option<String>,

    /// LLM temperature (0.0-2.0, higher = more creative, default 0.7 for flux)
    #[arg(long, value_name = "TEMP", default_value = "0.7")]
    temp: f32,

    /// Random seed for reproducible outputs
    #[arg(long, value_name = "SEED")]
    seed: Option<i64>,

    /// Resume a previous Flux Capacitor session (session ID or canvas path)
    #[arg(long, value_name = "SESSION")]
    resume: Option<String>,

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

    // Show all available providers with primary/secondary designation
    if available.len() == 1 {
        print_info(&format!("Provider: {}", available[0]));
    } else {
        print_info(&format!("Primary: {} | Secondary: {}",
            available[0],
            available.get(1).map(|s| *s).unwrap_or("none")
        ));
        if available.len() > 2 {
            print_info(&format!("Fallbacks: {}", available[2..].join(", ")));
        }
    }

    // Test mode - run comprehensive tests
    if args.test {
        let num_sessions = args.test_sessions.min(40);  // Cap at 40 sessions

        println!("\n{}", style("═".repeat(60)).dim());
        println!("{}", style("GANESHA COMPREHENSIVE TEST HARNESS").cyan().bold());
        println!("{}", style(format!("40 tests × {} sessions = {} total test runs",
            num_sessions, 40 * num_sessions)).dim());
        println!("{}", style("═".repeat(60)).dim());

        // Run full tests with actual LLM interaction
        let _results = comprehensive_test::run_full_tests(num_sessions).await;
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

    // Flux Capacitor mode - time-boxed autonomous execution
    if args.flux.is_some() || args.until.is_some() {
        let (provider_url, model) = chain.get_first_available_url()
            .unwrap_or_else(|| ("http://192.168.245.155:1234".to_string(), "default".to_string()));

        // Calculate duration
        let duration = if let Some(ref flux_str) = args.flux {
            match flux::parse_duration(flux_str) {
                Some(d) => d,
                None => {
                    print_error(&format!("Invalid duration: '{}'. Try '1h', '30m', '2 hours', or 'auto'", flux_str));
                    std::process::exit(1);
                }
            }
        } else if let Some(ref until_str) = args.until {
            match flux::parse_target_time(until_str) {
                Some(target) => flux::duration_until(target),
                None => {
                    print_error(&format!("Invalid time: '{}'. Try '11:11', '23:30', or '11:11 PM'", until_str));
                    std::process::exit(1);
                }
            }
        } else {
            unreachable!()
        };

        if task.is_empty() {
            print_error("Flux Capacitor requires a task. Example: ganesha --flux 1h \"optimize this code\"");
            std::process::exit(1);
        }

        let config = flux::FluxConfig {
            duration,
            task: task.clone(),
            auto_extend: args.flux.as_ref().map(|f| f == "auto").unwrap_or(false),
            provider_url,
            model,
            auto_approve: args.auto,
            verbose: !args.quiet,
            temperature: args.temp,
            seed: args.seed,
            resume: args.resume.clone(),
        };

        match flux::run_flux_capacitor(config).await {
            Ok(status) => {
                if status.iterations > 0 {
                    println!("{} Flux Capacitor completed {} iterations",
                        style("⚡").cyan(), status.iterations);
                }
            }
            Err(e) => {
                print_error(&format!("Flux Capacitor error: {}", e));
                std::process::exit(1);
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

    // Configure vision from saved config (not hardcoded)
    // Read the ProviderManager's config to get the vision provider setting
    let config_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".ganesha")
        .join("config.json");

    let vision_configured = if config_path.exists() {
        // Try to read vision config from saved settings
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(vision) = config.get("tiers").and_then(|t| t.get("vision")) {
                    let endpoint = vision.get("endpoint").and_then(|e| e.as_str()).unwrap_or("");
                    let model = vision.get("model").and_then(|m| m.as_str()).unwrap_or("");
                    let description = vision.get("description").and_then(|d| d.as_str()).unwrap_or("Vision");

                    if !endpoint.is_empty() && !model.is_empty() {
                        // Check if the vision endpoint is actually available
                        let endpoints = config.get("endpoints");
                        let endpoint_url = endpoints
                            .and_then(|e| e.get(endpoint))
                            .and_then(|e| e.get("base_url"))
                            .and_then(|u| u.as_str())
                            .unwrap_or("");

                        if !endpoint_url.is_empty() {
                            let check_url = format!("{}/v1/models", endpoint_url);
                            let is_online = reqwest::blocking::Client::builder()
                                .timeout(std::time::Duration::from_secs(2))
                                .build()
                                .ok()
                                .and_then(|c| c.get(&check_url).send().ok())
                                .map(|r| r.status().is_success())
                                .unwrap_or(false);

                            if is_online {
                                workflow.configure_vision(false, Some((endpoint.to_string(), model.to_string())));
                                print_info(&format!("Vision: {} ({})", description, model));
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    // Fallback: auto-detect vision providers if not configured
    if !vision_configured {
        // Check available providers for vision capability
        let check_provider = |url: &str| -> bool {
            reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .ok()
                .and_then(|c| c.get(&format!("{}/v1/models", url)).send().ok())
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        };

        // Check secondary/vision servers (these typically have vision models)
        if check_provider("http://192.168.27.182:1234") {
            workflow.configure_vision(false, Some(("bedroom".to_string(), "default".to_string())));
            print_info("Vision: bedroom server (auto-detected)");
        } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            workflow.configure_vision(false, Some(("anthropic".to_string(), "claude-sonnet-4-5-20250514".to_string())));
            print_info("Vision: Anthropic Claude (fallback)");
        }
    }

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

        // Ensure cursor is visible (spinners/animations may hide it)
        print!("\x1b[?25h");
        let _ = std::io::Write::flush(&mut std::io::stdout());

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
                    println!("  cd <path>      Change working directory");
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
                    println!("{}", style("https://github.com/G-TechSD/ganesha-ai").dim());
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

                // Handle cd command - change working directory
                if input.starts_with("cd ") || input == "cd" {
                    let path_str = input.strip_prefix("cd").unwrap_or("").trim();

                    // Handle cd with no args -> go to home
                    let target_path = if path_str.is_empty() {
                        dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"))
                    } else {
                        // Handle ~ expansion
                        let expanded = if path_str.starts_with("~/") {
                            dirs::home_dir()
                                .unwrap_or_else(|| std::path::PathBuf::from("/"))
                                .join(&path_str[2..])
                        } else if path_str == "~" {
                            dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"))
                        } else if path_str.starts_with('/') {
                            std::path::PathBuf::from(path_str)
                        } else {
                            // Relative path
                            engine.working_directory.join(path_str)
                        };
                        expanded
                    };

                    // Canonicalize to resolve .. and .
                    match target_path.canonicalize() {
                        Ok(canonical) => {
                            if canonical.is_dir() {
                                engine.working_directory = canonical.clone();
                                println!("{} {}", style("📁").cyan(), canonical.display());
                            } else {
                                print_error(&format!("Not a directory: {}", target_path.display()));
                            }
                        }
                        Err(_) => {
                            print_error(&format!("No such directory: {}", target_path.display()));
                        }
                    }
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
                // Get vision config for image analysis
                let vision_cfg = if workflow.vision_config.is_available() {
                    workflow.vision_config.cloud_vision_provider.as_ref()
                        .zip(workflow.vision_config.cloud_vision_model.as_ref())
                        .map(|(p, m)| (p.as_str(), m.as_str()))
                } else {
                    None
                };
                let output = run_task_with_log(engine, input, code_mode, vision_cfg).await;
                session_log.push(format!("[{}] GANESHA: {}", Local::now().format("%H:%M:%S"), output));

                // Auto-return to Chat mode if we auto-switched for this task
                if workflow.auto_triggered_mode {
                    workflow.complete_auto_task();
                    println!("{} Returned to Chat mode", style("💬").dim());
                }

                println!(); // Add spacing after task completion
            }
            Err(ReadlineError::Interrupted) => {
                // First Ctrl+C: if in a non-Chat mode, return to Chat
                if workflow.current_mode != GaneshaMode::Chat {
                    workflow.force_transition(GaneshaMode::Chat);
                    println!("\n{} Returned to Chat mode", style("💬").cyan());
                    last_interrupt = None;
                    continue;
                }

                // In Chat mode: check for double Ctrl+C to exit
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

/// Display multiple choice question and get user's answer
fn ask_multiple_choice(question: &core::MultipleChoiceQuestion) -> Option<String> {
    use dialoguer::{theme::ColorfulTheme, Select, Input};

    println!();
    println!("{} {}", style("❓").cyan(), style(&question.question).bold());

    if let Some(ref ctx) = question.context {
        println!("{}", style(ctx).dim());
    }
    println!();

    // Build choices with "Other" option
    let mut choices: Vec<String> = question.options.iter()
        .enumerate()
        .map(|(i, opt)| format!("{}. {}", i + 1, opt))
        .collect();
    choices.push(format!("{}. Other (type your own answer)", choices.len() + 1));

    // Show selection menu
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&choices)
        .default(0)
        .interact_opt();

    match selection {
        Ok(Some(idx)) if idx < question.options.len() => {
            // User selected a predefined option
            Some(question.options[idx].clone())
        }
        Ok(Some(_)) => {
            // User selected "Other" - prompt for custom input
            println!();
            let custom: Result<String, _> = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Your answer")
                .interact_text();

            match custom {
                Ok(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
                _ => None,
            }
        }
        _ => None, // User cancelled
    }
}

/// Check if a file is an image by extension
fn is_image_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
        || lower.ends_with(".gif") || lower.ends_with(".bmp") || lower.ends_with(".webp")
        || lower.ends_with(".tiff") || lower.ends_with(".tif")
}

/// Extract image file paths from user input
fn extract_image_paths(input: &str, working_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // Pattern 1: Quoted paths
    let quoted_re = regex::Regex::new(r#"["']([^"']+\.(png|jpg|jpeg|gif|bmp|webp|tiff|tif))["']"#).ok();
    if let Some(re) = quoted_re {
        for cap in re.captures_iter(input) {
            if let Some(m) = cap.get(1) {
                let path_str = m.as_str();
                let path = if path_str.starts_with('/') {
                    std::path::PathBuf::from(path_str)
                } else if path_str.starts_with("~/") {
                    dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("/"))
                        .join(&path_str[2..])
                } else {
                    working_dir.join(path_str)
                };
                if path.exists() {
                    paths.push(path);
                }
            }
        }
    }

    // Pattern 2: Unquoted file paths (with image extensions)
    let unquoted_re = regex::Regex::new(r"(?:^|\s)([/~]?[a-zA-Z0-9_./-]+\.(png|jpg|jpeg|gif|bmp|webp|tiff|tif))(?:\s|$|[,.])")
        .ok();
    if let Some(re) = unquoted_re {
        for cap in re.captures_iter(input) {
            if let Some(m) = cap.get(1) {
                let path_str = m.as_str();
                let path = if path_str.starts_with('/') {
                    std::path::PathBuf::from(path_str)
                } else if path_str.starts_with("~/") {
                    dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("/"))
                        .join(&path_str[2..])
                } else {
                    working_dir.join(path_str)
                };
                if path.exists() && !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
    }

    paths
}

/// Check if user is asking to analyze/describe an image
fn is_image_analysis_request(input: &str) -> bool {
    let lower = input.to_lowercase();
    let analysis_keywords = ["describe", "analyze", "what's in", "what is in", "tell me about",
                             "show me", "look at", "examine", "what does", "explain"];
    let image_keywords = ["image", "photo", "picture", "screenshot", "png", "jpg", "jpeg",
                          "images in", "photos in", "pictures in"];

    // Check for analysis keywords combined with image keywords or file extensions
    let has_analysis = analysis_keywords.iter().any(|k| lower.contains(k));
    let has_image_ref = image_keywords.iter().any(|k| lower.contains(k))
        || lower.contains(".png") || lower.contains(".jpg") || lower.contains(".jpeg")
        || lower.contains(".gif") || lower.contains(".webp");

    has_analysis && has_image_ref
}

/// Find all image files in a directory
fn find_images_in_directory(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut images = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_lower.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif") {
                        images.push(path);
                    }
                }
            }
        }
    }

    images
}

/// Analyze an image using the vision model
async fn analyze_image_with_vision(
    image_path: &std::path::Path,
    query: &str,
    vision_provider: &str,
    vision_model: &str,
) -> Result<String, String> {
    use base64_lib::Engine;
    use crate::orchestrator::vision::{VisionAnalyzer, VisionConfig};

    // Read and encode the image
    let image_data = std::fs::read(image_path)
        .map_err(|e| format!("Failed to read image: {}", e))?;
    let base64_image = base64_lib::engine::general_purpose::STANDARD.encode(&image_data);

    // Determine the endpoint URL from provider name
    let endpoint = match vision_provider {
        "bedroom" => "http://192.168.27.182:1234/v1/chat/completions",
        "beast" => "http://192.168.245.155:1234/v1/chat/completions",
        "anthropic" => "https://api.anthropic.com/v1/messages",
        "openai" => "https://api.openai.com/v1/chat/completions",
        _ if vision_provider.starts_with("http") => vision_provider,
        _ => "http://192.168.27.182:1234/v1/chat/completions", // Default to bedroom
    };

    // For Anthropic, we need special handling
    if vision_provider == "anthropic" {
        return analyze_image_anthropic(&base64_image, query, vision_model).await;
    }

    // Use VisionAnalyzer for OpenAI-compatible endpoints
    let config = VisionConfig {
        endpoint: endpoint.to_string(),
        model: vision_model.to_string(),
        timeout: std::time::Duration::from_secs(60),
    };

    let analyzer = VisionAnalyzer::new(config);

    // Query the image
    analyzer.query_screen(&base64_image, query).await
        .map_err(|e| format!("Vision analysis failed: {}", e))
}

/// Analyze image using Anthropic API (different format)
async fn analyze_image_anthropic(
    base64_image: &str,
    query: &str,
    model: &str,
) -> Result<String, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY not set")?;

    let client = reqwest::Client::new();

    let request = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": base64_image
                    }
                },
                {
                    "type": "text",
                    "text": query
                }
            ]
        }]
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error {}: {}", status, body));
    }

    let json: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(json["content"][0]["text"]
        .as_str()
        .unwrap_or("Unable to analyze image")
        .to_string())
}

/// Run a task autonomously - execute commands, analyze results, continue until done
async fn run_task_with_log<C: core::ConsentHandler>(
    engine: &mut GaneshaEngine<ProviderChain, C>,
    task: &str,
    code_mode: bool,
    vision_config: Option<(&str, &str)>, // (provider, model)
) -> String {
    use rand::seq::SliceRandom;

    let task = if code_mode {
        format!("[CODE MODE] {}", task)
    } else {
        task.to_string()
    };

    // Check if this is an image analysis request
    if is_image_analysis_request(&task) {
        let mut image_paths = extract_image_paths(&task, &engine.working_directory);

        // If no specific images found but user mentions "images in this folder" etc,
        // find images in the current directory
        let lower_task = task.to_lowercase();
        if image_paths.is_empty() &&
            (lower_task.contains("images in") || lower_task.contains("photos in")
             || lower_task.contains("pictures in") || lower_task.contains("this folder")
             || lower_task.contains("this directory") || lower_task.contains("current folder"))
        {
            image_paths = find_images_in_directory(&engine.working_directory);

            if image_paths.is_empty() {
                println!("{} No image files found in {}",
                    style("ℹ").cyan(), engine.working_directory.display());
                return format!("No image files found in {}", engine.working_directory.display());
            } else {
                println!("{} Found {} image(s) in {}",
                    style("ℹ").cyan(), image_paths.len(), engine.working_directory.display());
            }
        }

        if !image_paths.is_empty() {
            // We have image files to analyze
            if let Some((provider, model)) = vision_config {
                println!("{} Analyzing {} image(s) with vision model...",
                    style("👁️").cyan(), image_paths.len());

                let mut results = Vec::new();
                for (i, path) in image_paths.iter().enumerate() {
                    let query = format!("Describe this image in detail. What do you see?");
                    println!("{} [{}/{}] Analyzing {}...",
                        style("→").dim(), i + 1, image_paths.len(), path.file_name().unwrap_or_default().to_string_lossy());

                    match analyze_image_with_vision(path, &query, provider, model).await {
                        Ok(analysis) => {
                            println!("\n{} {}", style("Image:").bold(), path.display());
                            println!("{}", style(&analysis).cyan());
                            results.push(format!("{}: {}", path.display(), analysis));
                        }
                        Err(e) => {
                            println!("{} Vision analysis failed for {}: {}",
                                style("⚠").yellow(), path.display(), e);
                            results.push(format!("{}: Error - {}", path.display(), e));
                        }
                    }
                }

                if !results.is_empty() {
                    return results.join("\n\n");
                }
            } else {
                // No vision config - let user know
                println!("{} No vision model configured. Use /settings to configure vision.",
                    style("ℹ").cyan());
                return "Vision not configured. Run /settings to set up a vision model.".to_string();
            }
        }
    }

    let mut all_outputs: Vec<String> = vec![];
    let mut actions_taken: Vec<String> = vec![];
    let max_iterations = 10;  // Allow more iterations for complex tasks

    // Initial planning
    let thinking_msg = THINKING_MESSAGES
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"🐘 Thinking...");
    let spinner = create_spinner(thinking_msg);

    let mut current_task = task.clone();

    let mut current_plan = match engine.plan(&current_task).await {
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

    // Check for question - LLM wants to ask user something
    if let Some(action) = current_plan.actions.first() {
        if matches!(action.action_type, core::ActionType::Question) {
            if let Some(ref q) = action.question {
                // Display question and get user's answer
                if let Some(answer) = ask_multiple_choice(q) {
                    // Re-plan with user's answer
                    current_task = format!("{} [User selected: {}]", current_task, answer);
                    println!("{} Got it! Let me proceed with: {}", style("✓").green(), style(&answer).cyan());

                    let spinner = create_spinner("🐘 Thinking...");
                    current_plan = match engine.plan(&current_task).await {
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
                } else {
                    return "User cancelled".to_string();
                }
            }
        }
    }

    // Check for response-only (no commands)
    if current_plan.actions.iter().all(|a| a.command.is_empty()) {
        for action in &current_plan.actions {
            if !action.explanation.is_empty() && !matches!(action.action_type, core::ActionType::Question) {
                println!("\n{}", style(&action.explanation).cyan());
                all_outputs.push(action.explanation.clone());
            }
        }
        return all_outputs.join("\n");
    }

    // AGENTIC LOOP: Execute → Analyze → Continue until done
    for iteration in 0..max_iterations {
        let has_actions = current_plan.actions.iter().any(|a| !a.command.is_empty());
        if !has_actions {
            break;
        }

        // Execute each command in the plan
        let results = match engine.execute(&current_plan).await {
            Ok(r) => r,
            Err(e) => {
                if matches!(e, core::GaneshaError::UserCancelled) {
                    return "User cancelled".to_string();
                }
                print_error(&format!("{}", e));
                break;
            }
        };

        // Display results with clear format
        for result in &results {
            if result.command.is_empty() {
                continue;
            }

            // Show what we're running
            println!("{} {}", style("Running:").dim(), style(&result.command).white());

            // Show output (truncated if very long)
            let output = result.output.trim();
            if !output.is_empty() {
                let lines: Vec<&str> = output.lines().collect();
                if lines.len() > 20 {
                    for line in lines.iter().take(15) {
                        println!("  {}", style(line).dim());
                    }
                    println!("  {} ({} more lines)", style("...").dim(), lines.len() - 15);
                } else {
                    for line in lines {
                        println!("  {}", style(line).dim());
                    }
                }
            }

            // Show result status
            let status = if result.success {
                style("Command finished (SUCCESS)").green()
            } else {
                style("Command finished (FAILED)").red()
            };
            println!("{}", status);

            if let Some(ref err) = result.error {
                println!("  {}: {}", style("Error").red(), err);
            }

            println!();  // Blank line between commands
            actions_taken.push(result.command.clone());
            all_outputs.push(result.output.clone());
        }

        // Analyze results and decide next steps
        let spinner = create_spinner("🔍 Analyzing...");
        match engine.analyze_results(&current_task, &results).await {
            Ok((response, next_plan)) => {
                spinner.finish_and_clear();

                // If LLM returns more actions, continue the loop
                if let Some(plan) = next_plan {
                    if plan.actions.iter().any(|a| !a.command.is_empty()) {
                        current_plan = plan;
                        continue;  // Go back and execute new commands
                    }
                }

                // No more actions - show final response and exit
                if !response.is_empty() {
                    println!("\n{}", style(&response).cyan());
                    all_outputs.push(response);
                } else if !actions_taken.is_empty() {
                    // No response from LLM, generate a simple summary
                    println!("\n{}", style(format!("Completed {} action(s).", actions_taken.len())).green());
                }
                break;
            }
            Err(e) => {
                spinner.finish_and_clear();
                if std::env::var("GANESHA_DEBUG").is_ok() {
                    print_warning(&format!("Analysis error: {}", e));
                }
                // Even on error, try to give a summary
                if !actions_taken.is_empty() {
                    println!("\n{}", style(format!("Executed {} command(s).", actions_taken.len())).dim());
                }
                break;
            }
        }
    }

    all_outputs.join("\n")
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

    // Agentic loop - plan, execute, analyze, repeat if needed
    let max_iterations = 5;  // Safety limit
    let mut current_task = task.clone();

    for iteration in 0..max_iterations {
        // Plan
        let plan = match engine.plan(&current_task).await {
            Ok(p) => p,
            Err(e) => {
                print_error(&format!("Planning failed: {}", e));
                return;
            }
        };

        // Check if this is a response-only plan (no commands)
        let has_commands = plan.actions.iter().any(|a| !a.command.is_empty());

        // Execute
        let results = match engine.execute(&plan).await {
            Ok(r) => r,
            Err(e) => {
                // User cancelled is not an error to report
                if !matches!(e, core::GaneshaError::UserCancelled) {
                    print_error(&format!("{}", e));
                }
                return;
            }
        };

        // Show execution summaries for commands
        for result in &results {
            if result.command.is_empty() && !result.explanation.is_empty() {
                // Response action - show the response
                println!("\n{}", console::style(&result.explanation).cyan());
            } else if !result.command.is_empty() {
                // Command execution - show friendly summary
                print_action_summary(&result.command, result.success, &result.output, result.duration_ms);
            }

            if let Some(ref err) = result.error {
                print_error(err);
            }
        }

        // If no commands were run, we're done (pure response)
        if !has_commands {
            return;
        }

        // Analyze results and determine next steps
        if std::env::var("GANESHA_DEBUG").is_ok() {
            eprintln!("[DEBUG] Starting result analysis...");
        }
        match engine.analyze_results(&current_task, &results).await {
            Ok((response, next_plan)) => {
                if std::env::var("GANESHA_DEBUG").is_ok() {
                    eprintln!("[DEBUG] Analysis response: '{}' (has_plan: {})",
                        if response.len() > 100 { &response[..100] } else { &response },
                        next_plan.is_some());
                }
                // Show the analysis response
                if !response.is_empty() {
                    println!("\n{}", console::style(&response).cyan());
                }

                // If there are more actions needed, continue the loop
                if let Some(plan) = next_plan {
                    if iteration < max_iterations - 1 {
                        print_info("Continuing with additional actions...");
                        // Execute the follow-up plan directly
                        match engine.execute(&plan).await {
                            Ok(follow_results) => {
                                for result in &follow_results {
                                    if !result.command.is_empty() {
                                        print_action_summary(&result.command, result.success, &result.output, result.duration_ms);
                                    }
                                    if let Some(ref err) = result.error {
                                        print_error(err);
                                    }
                                }
                                // Update task context for next iteration if needed
                                current_task = format!("{} [continuing from previous results]", task);
                            }
                            Err(e) => {
                                if !matches!(e, core::GaneshaError::UserCancelled) {
                                    print_error(&format!("{}", e));
                                }
                                return;
                            }
                        }
                    }
                } else {
                    // No more actions needed, we're done
                    return;
                }
            }
            Err(e) => {
                // Analysis failed, but execution succeeded - just continue
                if std::env::var("GANESHA_DEBUG").is_ok() {
                    print_warning(&format!("Analysis error: {}", e));
                }
                return;
            }
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
