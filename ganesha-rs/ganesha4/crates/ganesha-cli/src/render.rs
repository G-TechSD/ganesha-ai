//! # Terminal Rendering
//!
//! Utilities for rendering formatted output to the terminal.

use colored::Colorize;
use textwrap::{wrap, Options};
use unicode_width::UnicodeWidthStr;

/// Output style
pub enum Style {
    Assistant,
    User,
    System,
    Error,
    Warning,
    Info,
    Success,
    Code,
}

/// Print a message with a style
pub fn print_styled(message: &str, style: Style) {
    let prefix = match style {
        Style::Assistant => "🐘".to_string(),
        Style::User => "You:".bright_blue().bold().to_string(),
        Style::System => "System:".bright_yellow().bold().to_string(),
        Style::Error => "Error:".bright_red().bold().to_string(),
        Style::Warning => "Warning:".bright_yellow().bold().to_string(),
        Style::Info => "Info:".bright_cyan().bold().to_string(),
        Style::Success => "✓".bright_green().bold().to_string(),
        Style::Code => "".to_string(),
    };

    if !prefix.is_empty() {
        print!("{} ", prefix);
    }

    println!("{}", message);
}

/// Print an assistant message with markdown rendering
#[allow(unused_assignments)]
pub fn print_assistant_message(message: &str) {
    println!();

    // Simple markdown rendering
    // For full markdown, we'd use termimad
    let lines: Vec<&str> = message.lines().collect();
    let mut in_code_block = false;
    let mut code_lang;

    for line in lines {
        if line.starts_with("```") {
            if in_code_block {
                // End code block
                println!("{}", "─".repeat(40).dimmed());
                in_code_block = false;
                code_lang = String::new(); // Reset for next block
            } else {
                // Start code block
                code_lang = line[3..].trim().to_string();
                println!("{} {}", "─".repeat(40).dimmed(), code_lang.dimmed());
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            println!("  {}", line.bright_cyan());
        } else if line.starts_with("# ") {
            println!("\n{}\n", line[2..].bright_magenta().bold());
        } else if line.starts_with("## ") {
            println!("\n{}\n", line[3..].bright_blue().bold());
        } else if line.starts_with("### ") {
            println!("{}", line[4..].bright_cyan().bold());
        } else if line.starts_with("- ") || line.starts_with("* ") {
            println!("  {} {}", "•".bright_green(), &line[2..]);
        } else if line.starts_with("> ") {
            println!("  {} {}", "│".dimmed(), line[2..].italic());
        } else if line.contains("**") {
            // Bold text
            let rendered = render_bold(line);
            println!("{}", rendered);
        } else if line.contains("`") && !line.contains("```") {
            // Inline code
            let rendered = render_inline_code(line);
            println!("{}", rendered);
        } else {
            println!("{}", line);
        }
    }
    println!();
}

/// Render bold text (**text**)
fn render_bold(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    let mut in_bold = false;

    while let Some(c) = chars.next() {
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            in_bold = !in_bold;
        } else if in_bold {
            result.push_str(&c.to_string().bold().to_string());
        } else {
            result.push(c);
        }
    }
    result
}

/// Render inline code (`code`)
fn render_inline_code(text: &str) -> String {
    let mut result = String::new();
    let mut in_code = false;

    for c in text.chars() {
        if c == '`' {
            in_code = !in_code;
        } else if in_code {
            result.push_str(&c.to_string().bright_cyan().to_string());
        } else {
            result.push(c);
        }
    }
    result
}

/// Print a risk level indicator
pub fn print_risk_level(level: &str, description: &str) {
    let (icon, color_fn): (&str, fn(&str) -> colored::ColoredString) = match level.to_lowercase().as_str() {
        "safe" => ("🟢", |s: &str| s.bright_green()),
        "normal" => ("🟡", |s: &str| s.bright_yellow()),
        "trusted" => ("🟠", |s: &str| s.truecolor(255, 165, 0)), // orange
        "yolo" => ("🔴", |s: &str| s.bright_red()),
        _ => ("⚪", |s: &str| s.white()),
    };

    println!("{} {} - {}", icon, color_fn(level), description.dimmed());
}

/// Print a model tier indicator
pub fn print_model_tier(tier: &str, model: &str) {
    let (icon, description) = match tier.to_lowercase().as_str() {
        "exceptional" => ("🟢", "Excellent for complex tasks"),
        "capable" => ("🟡", "Good for most tasks"),
        "limited" => ("🟠", "Simple tasks only"),
        "unsafe" => ("🔴", "May produce errors"),
        _ => ("⚪", "Unknown capability"),
    };

    println!("{} {} - {} ({})", icon, model, description.dimmed(), tier.dimmed());
}

/// Print a progress spinner
pub struct Spinner {
    message: String,
    pb: indicatif::ProgressBar,
}

impl Spinner {
    pub fn new(message: &str) -> Self {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        Self {
            message: message.to_string(),
            pb,
        }
    }

    pub fn update(&self, message: &str) {
        self.pb.set_message(message.to_string());
    }

    pub fn finish(&self) {
        self.pb.finish_and_clear();
    }

    pub fn finish_with_message(&self, message: &str) {
        self.pb.finish_with_message(message.to_string());
    }
}

/// Print a table
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.width()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.width());
            }
        }
    }

    // Print headers
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:width$}", h, width = widths[i]))
        .collect();
    println!("{}", header_line.join(" │ ").bright_cyan().bold());

    // Print separator
    let separator: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    println!("{}", separator.join("─┼─").dimmed());

    // Print rows
    for row in rows {
        let row_line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let width = widths.get(i).copied().unwrap_or(0);
                format!("{:width$}", cell, width = width)
            })
            .collect();
        println!("{}", row_line.join(" │ "));
    }
}

/// Get terminal width
pub fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
}

/// Wrap text to terminal width
pub fn wrap_text(text: &str, indent: usize) -> String {
    let width = terminal_width().saturating_sub(indent);
    let options = Options::new(width).initial_indent("").subsequent_indent("");

    wrap(text, &options).join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_variants() {
        let _ = Style::Assistant;
        let _ = Style::User;
        let _ = Style::System;
        let _ = Style::Error;
        let _ = Style::Warning;
        let _ = Style::Info;
        let _ = Style::Success;
        let _ = Style::Code;
    }

    #[test]
    fn test_terminal_width_positive() {
        let w = terminal_width();
        assert!(w > 0);
    }

    #[test]
    fn test_wrap_text_short() {
        let result = wrap_text("hello", 0);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_wrap_text_with_indent() {
        let result = wrap_text("hello world", 4);
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_wrap_text_long_line() {
        let long = "word ".repeat(100);
        let result = wrap_text(&long, 0);
        assert!(result.contains("word"));
    }

    #[test]
    fn test_wrap_text_empty() {
        let result = wrap_text("", 0);
        assert_eq!(result, "");
    }

    #[test]
    fn test_spinner_new() {
        let spinner = Spinner::new("loading");
        spinner.finish();
    }

    #[test]
    fn test_spinner_update_and_finish_with_message() {
        let spinner = Spinner::new("starting");
        spinner.update("updating");
        spinner.finish_with_message("done");
    }
}
