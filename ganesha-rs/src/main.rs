//! Ganesha - The Remover of Obstacles
//!
//! AI-powered system control. Local-first, safe by default.
//!
//! ```bash
//! ganesha "install docker"
//! ganesha --auto "update all packages"
//! ganesha --interactive
//! ```

mod cli;
mod core;
mod logging;
mod providers;

use clap::{Parser, Subcommand};
use cli::{print_banner, print_error, print_info, print_result, print_success, AutoConsent, CliConsent};
use core::access_control::{load_policy, AccessLevel};
use core::GaneshaEngine;
use providers::ProviderChain;

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

    // Print banner unless quiet
    if !args.quiet {
        print_banner();
    }

    // Get task
    let task = args.task.join(" ");

    if task.is_empty() && !args.interactive && !args.history && args.rollback.is_none() {
        eprintln!("No task provided. Use --help for usage.");
        std::process::exit(1);
    }

    // Load policy
    let policy = load_policy();

    // Create provider chain
    let chain = ProviderChain::default_chain();
    let available = chain.get_available();

    if available.is_empty() {
        print_error("No LLM providers available");
        print_info("Start LM Studio or Ollama, or set ANTHROPIC_API_KEY / OPENAI_API_KEY");
        std::process::exit(1);
    }

    print_info(&format!("Provider: {}", available.first().unwrap()));

    // Create engine with appropriate consent handler
    if args.auto {
        let mut engine = GaneshaEngine::new(chain, AutoConsent, policy);
        engine.auto_approve = true;
        run_task(&mut engine, &task, args.code).await;
    } else {
        let mut engine = GaneshaEngine::new(chain, CliConsent::new(), policy);
        run_task(&mut engine, &task, args.code).await;
    }
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
                print_result(result.success, &result.output, result.duration_ms);
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
