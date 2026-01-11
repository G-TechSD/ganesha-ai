//! Ganesha CLI
//!
//! ASCII art, colors, and interactive prompts.

use crate::core::{Action, ConsentHandler, ConsentResult, ExecutionPlan, RiskLevel};
use console::{style, Style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, Select};

/// ASCII banner - Ganesha the Elephant God
pub const BANNER_ART: &str = r#"
                     _.!._
                   /O*@*O\
                  <\@(_)@/>
         ,;,   .--;`     `;--.   ,
         O@O_ /   |d     b|   \ _hnn
         | `/ \   |       |   / \` |
         &&&&  :##;\     /;##;  &&&&
         |  \ / `##/|   |##'  \ /  |
         \   %%%%`</|   |#'`%%%%   /
          '._|_ \   |   |'  / _|_.'
            _/  /   \   \   \  \
           / (\(     '.  '-._&&&&
          (  ()##,    o'--.._`\-)
           '-():`##########'()()()
            /:::::/()`Y`()\:::::\
            \::::( () | () )::::/
             `"""`\().'.()/'"""`
"#;

pub const BANNER_TEXT: &str = r#"
  ██████   █████  ███    ██ ███████ ███████ ██   ██  █████
 ██       ██   ██ ████   ██ ██      ██      ██   ██ ██   ██
 ██   ███ ███████ ██ ██  ██ █████   ███████ ███████ ███████
 ██    ██ ██   ██ ██  ██ ██ ██           ██ ██   ██ ██   ██
  ██████  ██   ██ ██   ████ ███████ ███████ ██   ██ ██   ██
"#;

pub fn print_banner() {
    println!("{}", style(BANNER_ART).magenta());
    println!("{}", style(BANNER_TEXT).cyan().bold());
    println!(
        "{}",
        style("           ✦  R E M O V E R   O F   O B S T A C L E S  ✦")
            .yellow()
            .bold()
    );
    println!(
        "{}",
        style(format!("                        Version {}", env!("CARGO_PKG_VERSION")))
            .dim()
    );
    println!();
}

pub fn print_info(msg: &str) {
    println!("{} {}", style("ℹ").cyan(), msg);
}

pub fn print_success(msg: &str) {
    println!("{} {}", style("✓").green().bold(), msg);
}

pub fn print_error(msg: &str) {
    println!("{} {}", style("✗").red().bold(), msg);
}

pub fn print_warning(msg: &str) {
    println!("{} {}", style("⚠").yellow().bold(), msg);
}

fn risk_style(risk: &RiskLevel) -> Style {
    match risk {
        RiskLevel::Low => Style::new().green(),
        RiskLevel::Medium => Style::new().yellow(),
        RiskLevel::High => Style::new().red(),
        RiskLevel::Critical => Style::new().red().bold().on_black(),
    }
}

pub fn print_plan(plan: &ExecutionPlan) {
    println!();
    println!(
        "{}",
        style("════════════════════════════════════════════════════════════")
            .dim()
    );
    println!("{}", style("EXECUTION PLAN").cyan().bold());
    println!("Task: {}", plan.task);
    println!("Actions: {}", plan.total_actions());

    let high_risk = plan.high_risk_count();
    if high_risk > 0 {
        println!(
            "{}",
            style(format!("⚠ {} HIGH RISK action(s)", high_risk))
                .red()
                .bold()
        );
    }

    println!(
        "{}",
        style("────────────────────────────────────────────────────────────")
            .dim()
    );
    println!();

    for (i, action) in plan.actions.iter().enumerate() {
        let risk_badge = format!("[{}]", action.risk_level.to_string().to_uppercase());
        let risk_styled = risk_style(&action.risk_level).apply_to(&risk_badge);

        println!(
            "{} {}",
            style(format!("[{}/{}]", i + 1, plan.total_actions())).dim(),
            risk_styled
        );
        println!("Command: {}", style(&action.command).white().bold());
        println!("Explanation: {}", style(&action.explanation).dim());
        println!();
    }
}

pub fn print_result(success: bool, output: &str, duration_ms: u64) {
    if success {
        print_success(&format!("Completed in {}ms", duration_ms));
        if !output.trim().is_empty() {
            // Truncate long output
            let lines: Vec<&str> = output.lines().collect();
            let display_lines = if lines.len() > 10 {
                let shown: Vec<&str> = lines.iter().take(10).copied().collect();
                format!(
                    "{}\n... ({} more lines)",
                    shown.join("\n"),
                    lines.len() - 10
                )
            } else {
                output.to_string()
            };
            println!("{}", style(display_lines).dim());
        }
    } else {
        print_error(&format!("Failed after {}ms", duration_ms));
    }
}

/// Describe what an action did in a friendly way
pub fn describe_action(command: &str, success: bool) -> String {
    let cmd = command.trim();

    // File creation patterns
    if cmd.contains('>') && !cmd.contains(">>") {
        if let Some(file) = extract_redirect_target(cmd) {
            return if success {
                format!("📄 Created file: {}", style(&file).cyan())
            } else {
                format!("Failed to create file: {}", file)
            };
        }
    }

    // Append to file
    if cmd.contains(">>") {
        if let Some(file) = extract_redirect_target(cmd) {
            return if success {
                format!("📝 Appended to file: {}", style(&file).cyan())
            } else {
                format!("Failed to append to file: {}", file)
            };
        }
    }

    // Directory creation
    if cmd.starts_with("mkdir ") {
        let dir = cmd.strip_prefix("mkdir ").unwrap_or("").trim();
        let dir = dir.trim_start_matches("-p ").trim();
        return if success {
            format!("📁 Created directory: {}", style(dir).cyan())
        } else {
            format!("Failed to create directory: {}", dir)
        };
    }

    // File copy
    if cmd.starts_with("cp ") {
        return if success {
            "📋 Copied file(s)".to_string()
        } else {
            "Failed to copy file(s)".to_string()
        };
    }

    // File move/rename
    if cmd.starts_with("mv ") {
        return if success {
            "📦 Moved/renamed file(s)".to_string()
        } else {
            "Failed to move file(s)".to_string()
        };
    }

    // File deletion
    if cmd.starts_with("rm ") {
        return if success {
            "🗑️ Deleted file(s)".to_string()
        } else {
            "Failed to delete file(s)".to_string()
        };
    }

    // Git operations
    if cmd.starts_with("git ") {
        let subcmd = cmd.strip_prefix("git ").unwrap_or("").split_whitespace().next().unwrap_or("");
        return match subcmd {
            "add" => "📥 Staged files for commit".to_string(),
            "commit" => "💾 Created commit".to_string(),
            "push" => "🚀 Pushed to remote".to_string(),
            "pull" => "📥 Pulled from remote".to_string(),
            "clone" => "📦 Cloned repository".to_string(),
            _ => format!("Git: {}", subcmd),
        };
    }

    // Package installation
    if cmd.starts_with("npm install") || cmd.starts_with("yarn add") || cmd.starts_with("pip install") {
        return if success {
            "📦 Installed package(s)".to_string()
        } else {
            "Failed to install package(s)".to_string()
        };
    }

    // Default - just show the command was executed
    if success {
        "✨ Command executed successfully".to_string()
    } else {
        "Command failed".to_string()
    }
}

/// Extract the target file from a redirect command
fn extract_redirect_target(cmd: &str) -> Option<String> {
    // Handle both > and >> redirects
    let parts: Vec<&str> = if cmd.contains(">>") {
        cmd.split(">>").collect()
    } else {
        cmd.split('>').collect()
    };

    if parts.len() >= 2 {
        let target = parts.last()?.trim();
        // Remove any trailing quotes
        let target = target.trim_matches('"').trim_matches('\'');
        if !target.is_empty() {
            return Some(target.to_string());
        }
    }
    None
}

/// Print a friendly action summary
pub fn print_action_summary(command: &str, success: bool, output: &str, duration_ms: u64) {
    let description = describe_action(command, success);

    if success {
        println!("{} {}", style("✓").green().bold(), description);

        // Show output if there is any meaningful content
        let trimmed = output.trim();
        if !trimmed.is_empty() && trimmed.len() > 1 {
            // Truncate long output
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() > 8 {
                let shown: Vec<&str> = lines.iter().take(6).copied().collect();
                println!("{}", style(shown.join("\n")).dim());
                println!("{}", style(format!("... ({} more lines)", lines.len() - 6)).dim());
            } else {
                println!("{}", style(trimmed).dim());
            }
        }

        // Show timing for longer operations
        if duration_ms > 100 {
            println!("{}", style(format!("  ({}ms)", duration_ms)).dim());
        }
    } else {
        println!("{} {}", style("✗").red().bold(), description);
    }
}

/// CLI Consent Handler
pub struct CliConsent {
    term: Term,
}

impl CliConsent {
    pub fn new() -> Self {
        Self {
            term: Term::stdout(),
        }
    }
}

impl Default for CliConsent {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsentHandler for CliConsent {
    fn request_consent(&self, action: &Action) -> bool {
        let risk_badge = format!("[{}]", action.risk_level.to_string().to_uppercase());
        let risk_styled = risk_style(&action.risk_level).apply_to(&risk_badge);

        println!();
        println!("{} Command: {}", risk_styled, style(&action.command).bold());
        println!("  {}", style(&action.explanation).dim());

        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Execute?")
            .default(false)
            .interact()
            .unwrap_or(false)
    }

    fn request_batch_consent(&self, plan: &ExecutionPlan) -> ConsentResult {
        print_plan(plan);

        let choices = vec!["Yes - Execute all", "No - Cancel", "Review individually"];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Execute?")
            .items(&choices)
            .default(1) // Default to Cancel for safety
            .interact_opt();

        match selection {
            Ok(Some(0)) => ConsentResult::ApproveAll,
            Ok(Some(2)) => ConsentResult::ApproveSingle,
            _ => ConsentResult::Cancel,
        }
    }
}

/// Auto-approve consent handler (for --auto flag)
pub struct AutoConsent;

impl ConsentHandler for AutoConsent {
    fn request_consent(&self, _action: &Action) -> bool {
        true
    }

    fn request_batch_consent(&self, _plan: &ExecutionPlan) -> ConsentResult {
        ConsentResult::ApproveAll
    }
}
