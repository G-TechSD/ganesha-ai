//! # Flux Capacitor — Time-Boxed Autonomous Work
//!
//! Runs Ganesha in autonomous mode for a specified duration.
//! Think of it as "pair programming on autopilot."
//!
//! Usage: `ganesha flux 2h "build a REST API for todo items"`

use crate::cli::Cli;
use crate::render;
use colored::Colorize;
use ganesha_providers::{GenerateOptions, Message, ProviderManager};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Parse a duration string like "2h", "30m", "1h30m", "90m"
pub fn parse_duration(input: &str) -> Option<Duration> {
    let input = input.trim().to_lowercase();

    // Try hours + minutes (e.g., "1h30m")
    if input.contains('h') && input.contains('m') {
        let parts: Vec<&str> = input.split('h').collect();
        if parts.len() == 2 {
            let hours: u64 = parts[0].parse().ok()?;
            let minutes: u64 = parts[1].trim_end_matches('m').parse().ok()?;
            return Some(Duration::from_secs(hours * 3600 + minutes * 60));
        }
    }

    // Try hours only (e.g., "2h")
    if input.ends_with('h') {
        let hours: u64 = input.trim_end_matches('h').parse().ok()?;
        return Some(Duration::from_secs(hours * 3600));
    }

    // Try minutes only (e.g., "30m")
    if input.ends_with('m') {
        let minutes: u64 = input.trim_end_matches('m').parse().ok()?;
        return Some(Duration::from_secs(minutes * 60));
    }

    // Try seconds (e.g., "300s")
    if input.ends_with('s') {
        let secs: u64 = input.trim_end_matches('s').parse().ok()?;
        return Some(Duration::from_secs(secs));
    }

    // Try bare number as minutes
    if let Ok(minutes) = input.parse::<u64>() {
        return Some(Duration::from_secs(minutes * 60));
    }

    None
}

/// Format a duration for display
pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 && minutes > 0 {
        format!("{}h {}m", hours, minutes)
    } else if hours > 0 {
        format!("{}h", hours)
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        format!("{}s", secs)
    }
}

/// Format remaining time
fn format_remaining(elapsed: Duration, total: Duration) -> String {
    if elapsed >= total {
        return "⏰ TIME'S UP".to_string();
    }
    let remaining = total - elapsed;
    format!("⏱ {} remaining", format_duration(remaining))
}

/// Run the flux capacitor mode
pub async fn run(duration_str: String, task: String, cli: &Cli) -> anyhow::Result<()> {
    // Parse duration
    let duration = match parse_duration(&duration_str) {
        Some(d) => d,
        None => {
            eprintln!("{} Invalid duration: '{}'. Examples: 2h, 30m, 1h30m", "Error:".red().bold(), duration_str);
            return Err(anyhow::anyhow!("Invalid duration"));
        }
    };

    // Validate reasonable duration
    if duration < Duration::from_secs(60) {
        eprintln!("{} Duration must be at least 1 minute", "Error:".red().bold());
        return Err(anyhow::anyhow!("Duration too short"));
    }
    if duration > Duration::from_secs(8 * 3600) {
        eprintln!("{} Duration capped at 8 hours for safety", "Warning:".yellow().bold());
    }

    let duration = duration.min(Duration::from_secs(8 * 3600));

    // Initialize provider
    let provider_manager = Arc::new(ProviderManager::new());
    provider_manager.auto_discover().await?;

    if !provider_manager.has_available_provider().await {
        eprintln!("{} No LLM providers available.", "Error:".red().bold());
        return Err(anyhow::anyhow!("No providers"));
    }

    // Show startup banner
    println!();
    println!("{}", "╔══════════════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║         ⚡ FLUX CAPACITOR MODE ⚡               ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════════════════════╝".bright_cyan());
    println!();
    println!("  {} {}", "Task:".bright_white().bold(), task.bright_yellow());
    println!("  {} {}", "Duration:".bright_white().bold(), format_duration(duration).bright_green());
    println!("  {} {}", "Started:".bright_white().bold(), chrono::Local::now().format("%H:%M:%S").to_string().dimmed());
    println!("  {} {}", "Mode:".bright_white().bold(), format!("{:?}", cli.mode).bright_cyan());
    println!();
    println!("  {} Press {} to abort", "⚠".yellow(), "Ctrl+C".bright_red());
    println!();

    let start = Instant::now();
    let working_dir = cli.directory.as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Build system prompt for autonomous work
    let system_prompt = format!(
        "You are Ganesha in Flux Capacitor mode — autonomous time-boxed work. \
         Your task: {task}\n\n\
         RULES:\n\
         - Work autonomously. Don't ask questions — make reasonable assumptions.\n\
         - Execute ONE shell command per response using ```bash blocks.\n\
         - Be efficient. You have {} to complete this task.\n\
         - After each command result, assess progress and continue.\n\
         - Create files, edit code, run tests, commit changes.\n\
         - If stuck, try a different approach.\n\
         - When done, summarize what you accomplished.\n\n\
         Working directory: {}\n\
         OS: {}\n\
         Shell: {}",
        format_duration(duration),
        working_dir.display(),
        std::env::consts::OS,
        if cfg!(windows) { "PowerShell" } else { "sh" },
    );

    let mut messages = vec![
        Message::system(&system_prompt),
        Message::user(&format!("Begin working on: {}\n\nStart by understanding the current state (list files, read code, etc.), then proceed to implement.", task)),
    ];

    let mut iteration = 0;
    let max_iterations = 100;
    let mut commands_run = 0;
    let mut files_changed: Vec<String> = Vec::new();

    loop {
        // Check time
        let elapsed = start.elapsed();
        if elapsed >= duration {
            println!("\n{}", "⏰ Time's up! Flux Capacitor session complete.".bright_yellow().bold());
            break;
        }

        iteration += 1;
        if iteration > max_iterations {
            println!("\n{}", "Reached maximum iterations.".yellow());
            break;
        }

        // Status line
        println!("{}", format!("─── Iteration {} · {} ───", iteration, format_remaining(elapsed, duration)).dimmed());

        // Call AI
        let options = GenerateOptions {
            model: cli.model.clone(),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            ..Default::default()
        };

        let spinner = render::Spinner::new("Thinking...");
        let response = provider_manager.chat(&messages, &options).await;
        spinner.finish();

        match response {
            Ok(resp) => {
                let content = resp.content.clone();

                // Extract command from response
                let cmd = extract_bash_command(&content);

                // Print AI's commentary (without the code block)
                let commentary = strip_code_blocks(&content);
                if !commentary.trim().is_empty() {
                    for line in commentary.lines().take(5) {
                        println!("  {} {}", "🐘".dimmed(), line.dimmed());
                    }
                }

                if let Some(command) = cmd {
                    println!("  {} {}", "→".bright_blue(), command.dimmed());

                    // Execute command
                    let output = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&command)
                        .current_dir(&working_dir)
                        .output();

                    match output {
                        Ok(out) => {
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let stderr = String::from_utf8_lossy(&out.stderr);

                            // Brief output
                            let combined = format!("{}{}", stdout, stderr);
                            for line in combined.lines().take(5) {
                                println!("    {}", line.dimmed());
                            }
                            if combined.lines().count() > 5 {
                                println!("    {} more lines...", "...".dimmed());
                            }

                            commands_run += 1;

                            // Track file changes
                            if command.contains("tee ") || command.contains("> ") || command.contains("cat >") || command.starts_with("echo ") {
                                if let Some(file) = command.split_whitespace().last() {
                                    if !files_changed.contains(&file.to_string()) {
                                        files_changed.push(file.to_string());
                                    }
                                }
                            }

                            // Feed result back
                            messages.push(Message::assistant(&content));
                            messages.push(Message::user(&format!(
                                "Command output ({}):\n```\n{}\n```\n\n{}\nContinue working.",
                                if out.status.success() { "success" } else { "failed" },
                                &combined[..combined.len().min(3000)],
                                format_remaining(elapsed, duration)
                            )));
                        }
                        Err(e) => {
                            println!("  {} Command error: {}", "✗".red(), e);
                            messages.push(Message::assistant(&content));
                            messages.push(Message::user(&format!("Command error: {}. Try a different approach.", e)));
                        }
                    }
                } else {
                    // No command — AI is done or giving commentary
                    messages.push(Message::assistant(&content));

                    // Check if AI thinks it's done
                    let lower = content.to_lowercase();
                    if lower.contains("completed") || lower.contains("all done") || lower.contains("task is finished") || lower.contains("summary of what") {
                        println!("\n{}", "✅ Task completed!".bright_green().bold());
                        println!();
                        // Print the final summary
                        for line in content.lines() {
                            println!("  {}", line);
                        }
                        break;
                    }

                    // Nudge to continue
                    messages.push(Message::user(&format!(
                        "Continue working on the task. {} Execute the next command.",
                        format_remaining(start.elapsed(), duration)
                    )));
                }
            }
            Err(e) => {
                eprintln!("  {} AI error: {}", "✗".red(), e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        // Trim conversation if too long (keep system + last 20 messages)
        if messages.len() > 22 {
            let system = messages[0].clone();
            let recent: Vec<_> = messages[messages.len()-20..].to_vec();
            messages = vec![system];
            messages.extend(recent);
        }
    }

    // Final summary
    let elapsed = start.elapsed();
    println!();
    println!("{}", "══════════════════════════════════════════════════".bright_cyan());
    println!("{}", "  Flux Capacitor Session Summary".bright_cyan().bold());
    println!("{}", "══════════════════════════════════════════════════".bright_cyan());
    println!("  {} {}", "Task:".bright_white(), task);
    println!("  {} {}", "Duration:".bright_white(), format_duration(elapsed));
    println!("  {} {}", "Iterations:".bright_white(), iteration);
    println!("  {} {}", "Commands run:".bright_white(), commands_run);
    if !files_changed.is_empty() {
        println!("  {} {}", "Files touched:".bright_white(), files_changed.join(", "));
    }
    println!();

    Ok(())
}

/// Extract a bash command from AI response
fn extract_bash_command(response: &str) -> Option<String> {
    // Look for ```bash or ```sh blocks
    let re = regex::Regex::new(r"```(?:bash|sh|shell)\n([\s\S]*?)```").ok()?;
    if let Some(cap) = re.captures(response) {
        let block = cap.get(1)?.as_str();
        // Get first non-comment, non-empty line
        for line in block.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                return Some(trimmed.to_string());
            }
        }
    }

    // Fallback: unmarked code blocks
    let re2 = regex::Regex::new(r"```\n([\s\S]*?)```").ok()?;
    if let Some(cap) = re2.captures(response) {
        let block = cap.get(1)?.as_str();
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() <= 3 {
            for line in lines {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

/// Strip code blocks from text for display
fn strip_code_blocks(text: &str) -> String {
    let re = regex::Regex::new(r"```[\s\S]*?```").unwrap();
    re.replace_all(text, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(parse_duration("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
    }

    #[test]
    fn test_parse_duration_minutes() {
        assert_eq!(parse_duration("30m"), Some(Duration::from_secs(1800)));
        assert_eq!(parse_duration("90m"), Some(Duration::from_secs(5400)));
    }

    #[test]
    fn test_parse_duration_combined() {
        assert_eq!(parse_duration("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("2h15m"), Some(Duration::from_secs(8100)));
    }

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(parse_duration("300s"), Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_parse_duration_bare_number() {
        assert_eq!(parse_duration("30"), Some(Duration::from_secs(1800))); // 30 minutes
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("abc").is_none());
        assert!(parse_duration("").is_none());
    }

    #[test]
    fn test_parse_duration_case_insensitive() {
        assert_eq!(parse_duration("2H"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_duration("30M"), Some(Duration::from_secs(1800)));
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(7200)), "2h");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(1800)), "30m");
    }

    #[test]
    fn test_format_duration_combined() {
        assert_eq!(format_duration(Duration::from_secs(5400)), "1h 30m");
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    }

    #[test]
    fn test_extract_bash_command() {
        let response = "Let me check:\n```bash\nls -la\n```";
        assert_eq!(extract_bash_command(response), Some("ls -la".to_string()));
    }

    #[test]
    fn test_extract_bash_command_skip_comments() {
        let response = "```bash\n# check files\nfind . -name '*.rs'\n```";
        assert_eq!(extract_bash_command(response), Some("find . -name '*.rs'".to_string()));
    }

    #[test]
    fn test_extract_bash_command_none() {
        let response = "No code blocks here.";
        assert!(extract_bash_command(response).is_none());
    }

    #[test]
    fn test_strip_code_blocks() {
        let text = "Before\n```bash\nls\n```\nAfter";
        let stripped = strip_code_blocks(text);
        assert!(stripped.contains("Before"));
        assert!(stripped.contains("After"));
        assert!(!stripped.contains("ls"));
    }

    #[test]
    fn test_parse_duration_zero_hours() {
        assert_eq!(parse_duration("0h"), Some(Duration::from_secs(0)));
    }

    #[test]
    fn test_parse_duration_large() {
        assert_eq!(parse_duration("24h"), Some(Duration::from_secs(86400)));
    }

    #[test]
    fn test_parse_duration_whitespace() {
        assert_eq!(parse_duration("  2h  "), Some(Duration::from_secs(7200)));
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
    }

    #[test]
    fn test_format_duration_large() {
        assert_eq!(format_duration(Duration::from_secs(7290)), "2h 1m");
    }

    #[test]
    fn test_extract_bash_command_sh_block() {
        let response = "```sh\necho hello\n```";
        assert_eq!(extract_bash_command(response), Some("echo hello".to_string()));
    }

    #[test]
    fn test_extract_bash_command_multiline() {
        let response = "```bash\n# setup\ncd /tmp\nls\n```";
        assert_eq!(extract_bash_command(response), Some("cd /tmp".to_string()));
    }

    #[test]
    fn test_extract_bash_command_unmarked_block() {
        let response = "Here:\n```\npwd\n```";
        assert_eq!(extract_bash_command(response), Some("pwd".to_string()));
    }

    #[test]
    fn test_strip_code_blocks_multiple() {
        let text = "A```bash\nx\n```B```sh\ny\n```C";
        let stripped = strip_code_blocks(text);
        assert!(stripped.contains("A"));
        assert!(stripped.contains("B"));
        assert!(stripped.contains("C"));
        assert!(!stripped.contains("x"));
        assert!(!stripped.contains("y"));
    }

    #[test]
    fn test_strip_code_blocks_no_blocks() {
        let text = "Just regular text";
        assert_eq!(strip_code_blocks(text), "Just regular text");
    }

    #[test]
    fn test_parse_duration_combined_1h0m() {
        assert_eq!(parse_duration("1h0m"), Some(Duration::from_secs(3600)));
    }

}
