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
