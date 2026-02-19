//! # REPL (Read-Eval-Print Loop)
//!
//! Interactive command-line interface for Ganesha.
//! Uses an emergent agentic architecture where the AI decides what commands to run.

use crate::cli::{ChatMode, Cli};
use crate::setup::{self, ProvidersConfig, ProviderType};
use colored::Colorize;
use ganesha_mcp::{McpManager, config::presets as mcp_presets, Tool as McpTool};
use ganesha_providers::{GenerateOptions, LocalProvider, LocalProviderType, Message, ProviderManager, ProviderPriority};
use rustyline::error::ReadlineError;
use rustyline::{Config, Editor, history::FileHistory};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;
use regex::Regex;
use tracing::{debug, info, warn};

/// Session logger that writes text logs to ~/.ganesha/sessions/
pub struct SessionLogger {
    /// Path to the current session log file
    log_path: PathBuf,
    /// File handle for writing
    file: Option<File>,
    /// Whether logging is enabled
    pub enabled: bool,
    /// Maximum total log size in bytes
    pub max_total_size: u64,
    /// Sessions directory
    pub sessions_dir: PathBuf,
}

impl SessionLogger {
    /// Create a new session logger
    pub fn new(enabled: bool, max_total_size: u64) -> Self {
        let sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ganesha")
            .join("sessions");

        Self {
            log_path: PathBuf::new(),
            file: None,
            enabled,
            max_total_size,
            sessions_dir,
        }
    }

    /// Start a new session log with the given title
    pub fn start_session(&mut self, title: Option<&str>) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Create sessions directory if needed
        if !self.sessions_dir.exists() {
            fs::create_dir_all(&self.sessions_dir)?;
        }

        // Check and enforce size limits
        self.enforce_size_limit()?;

        // Generate filename: <timestamp>-<title>.txt
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let safe_title = title
            .map(|t| {
                t.chars()
                    .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
                    .take(50)
                    .collect::<String>()
                    .replace(' ', "_")
            })
            .unwrap_or_else(|| "session".to_string());

        let filename = format!("{}-{}.txt", timestamp, safe_title);
        self.log_path = self.sessions_dir.join(&filename);

        // Open file for writing
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.log_path)?;

        self.file = Some(file);

        // Write session header
        self.write_line(&format!(
            "=== Ganesha Session: {} ===",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ))?;
        if let Some(t) = title {
            self.write_line(&format!("Title: {}", t))?;
        }
        self.write_line("")?;

        info!("Session log started: {:?}", self.log_path);
        Ok(())
    }

    /// Log a user message
    pub fn log_user(&mut self, message: &str) -> anyhow::Result<()> {
        if !self.enabled || self.file.is_none() {
            return Ok(());
        }

        self.write_line(&format!(
            "[{}] USER:",
            chrono::Local::now().format("%H:%M:%S")
        ))?;
        self.write_line(message)?;
        self.write_line("")?;
        self.flush()?;
        Ok(())
    }

    /// Log a command execution
    pub fn log_command(&mut self, command: &str, output: &str, success: bool) -> anyhow::Result<()> {
        if !self.enabled || self.file.is_none() {
            return Ok(());
        }

        let status = if success { "OK" } else { "FAILED" };
        self.write_line(&format!(
            "[{}] COMMAND [{}]:",
            chrono::Local::now().format("%H:%M:%S"),
            status
        ))?;
        self.write_line(&format!("$ {}", command))?;
        if !output.is_empty() {
            self.write_line("--- Output ---")?;
            // Limit output to prevent huge logs
            let truncated = if output.len() > 10000 {
                format!("{}... (truncated)", &output[..10000])
            } else {
                output.to_string()
            };
            self.write_line(&truncated)?;
            self.write_line("--- End Output ---")?;
        }
        self.write_line("")?;
        self.flush()?;
        Ok(())
    }

    /// Log an assistant response
    pub fn log_assistant(&mut self, response: &str) -> anyhow::Result<()> {
        if !self.enabled || self.file.is_none() {
            return Ok(());
        }

        self.write_line(&format!(
            "[{}] GANESHA:",
            chrono::Local::now().format("%H:%M:%S")
        ))?;
        self.write_line(response)?;
        self.write_line("")?;
        self.flush()?;
        Ok(())
    }

    /// End the session
    pub fn end_session(&mut self) -> anyhow::Result<()> {
        if !self.enabled || self.file.is_none() {
            return Ok(());
        }

        self.write_line(&format!(
            "\n=== Session ended: {} ===",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ))?;
        self.flush()?;
        self.file = None;
        Ok(())
    }

    /// Write a line to the log
    fn write_line(&mut self, line: &str) -> anyhow::Result<()> {
        if let Some(ref mut file) = self.file {
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    /// Flush the log file
    pub fn flush(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut file) = self.file {
            file.flush()?;
        }
        Ok(())
    }

    /// Enforce size limit by deleting oldest logs
    fn enforce_size_limit(&self) -> anyhow::Result<()> {
        if !self.sessions_dir.exists() {
            return Ok(());
        }

        // Get all log files with their sizes and times
        let mut logs: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total_size: u64 = 0;

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "txt") {
                if let Ok(metadata) = entry.metadata() {
                    let size = metadata.len();
                    let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
                    logs.push((path, size, modified));
                    total_size += size;
                }
            }
        }

        // If under limit, nothing to do
        if total_size <= self.max_total_size {
            return Ok(());
        }

        // Sort by modification time (oldest first)
        logs.sort_by(|a, b| a.2.cmp(&b.2));

        // Delete oldest files until under limit
        let target_size = (self.max_total_size as f64 * 0.8) as u64; // Leave 20% headroom
        for (path, size, _) in logs {
            if total_size <= target_size {
                break;
            }
            if let Err(e) = fs::remove_file(&path) {
                warn!("Failed to delete old log {:?}: {}", path, e);
            } else {
                info!("Deleted old session log: {:?}", path);
                total_size -= size;
            }
        }

        Ok(())
    }

    /// Get the path to the current log file
    pub fn log_path(&self) -> Option<&PathBuf> {
        if self.enabled && self.file.is_some() {
            Some(&self.log_path)
        } else {
            None
        }
    }

    /// List all session logs
    pub fn list_sessions(&self) -> anyhow::Result<Vec<(PathBuf, u64, chrono::DateTime<chrono::Local>)>> {
        let mut sessions = Vec::new();

        if !self.sessions_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "txt") {
                if let Ok(metadata) = entry.metadata() {
                    let size = metadata.len();
                    let modified: chrono::DateTime<chrono::Local> = metadata
                        .modified()
                        .unwrap_or(std::time::UNIX_EPOCH)
                        .into();
                    sessions.push((path, size, modified));
                }
            }
        }

        // Sort by modification time (newest first)
        sessions.sort_by(|a, b| b.2.cmp(&a.2));
        Ok(sessions)
    }

    /// Get the total size of all session logs
    pub fn total_size(&self) -> anyhow::Result<u64> {
        let sessions = self.list_sessions()?;
        Ok(sessions.iter().map(|(_, size, _)| size).sum())
    }
}

/// Parse and detect multiple choice options from AI response
/// Only returns options if the AI is EXPLICITLY presenting numbered choices for user selection
/// This is very strict to avoid false positives on informational numbered lists
/// Returns detected options if any, or None
fn detect_options(_text: &str) -> Option<Vec<String>> {
    // DISABLED: This feature was causing too many false positives
    // (e.g., detecting ingredient lists as choices when AI asks "would you like to know more?")
    //
    // The multiple choice UI should only appear when the AI explicitly formats choices like:
    // "Please select an option:
    //  1. Option A
    //  2. Option B"
    //
    // But distinguishing this from informational lists like:
    // "Here are the ingredients:
    //  1. Malt - provides color
    //  2. Hops - adds bitterness
    //  Would you like to learn more?"
    //
    // Is too error-prone. Disabling until we can implement a more robust solution
    // (e.g., having the AI explicitly mark choices with a special format)
    None
}

/// Display interactive multiple choice prompt
/// Returns the selected option, custom text, or None if declined
fn prompt_multiple_choice(options: &[String]) -> Option<String> {
    use std::io::{self, Write};

    println!();
    println!("{}", "┌─ Select an option: ─────────────────────────".bright_cyan());
    println!("{}", "│".bright_cyan());
    for (i, opt) in options.iter().enumerate() {
        println!("{}  {} {}", "│".bright_cyan(), format!("[{}]", i + 1).bright_yellow(), opt);
    }
    println!("{}", "│".bright_cyan());
    println!("{}  {} No thanks / Skip", "│".bright_cyan(), "[n]".bright_red());
    println!("{}  {} Type your own response", "│".bright_cyan(), "[o]".dimmed());
    println!("{}", "│".bright_cyan());
    println!("{}", "└──────────────────────────────────────────────".bright_cyan());
    println!();

    print!("{} ", "Choice (1-{}, n, or o):".replace("{}", &options.len().to_string()).bright_white());
    io::stdout().flush().ok()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return None;
    }

    // Check for decline
    if input == "n" || input == "no" || input == "skip" || input == "none" {
        return None;
    }

    // Check for custom response
    if input == "o" || input == "other" || input == "0" {
        print!("{} ", "Your response:".bright_white());
        io::stdout().flush().ok()?;
        let mut custom = String::new();
        io::stdin().read_line(&mut custom).ok()?;
        let custom = custom.trim();
        if custom.is_empty() {
            return None;
        }
        return Some(custom.to_string());
    }

    // Check if it's a number
    if let Ok(num) = input.parse::<usize>() {
        if num > 0 && num <= options.len() {
            return Some(options[num - 1].clone());
        }
    }

    // Treat as custom input
    Some(input.to_string())
}

/// Execute a shell command in the current working directory
/// Returns (stdout, stderr, success)
fn run_shell_command(command: &str, working_dir: &PathBuf) -> (String, String, bool) {
    debug!("Executing: {}", command);

    #[cfg(windows)]
    let output = {
        use base64::Engine;

        // For commands with quotes, use -EncodedCommand to avoid escaping issues
        // This encodes the command as base64 UTF-16LE which PowerShell decodes
        let needs_encoding = command.contains('\'') && command.contains('"')
            || command.matches('\'').count() > 2
            || command.matches('"').count() > 2;

        if needs_encoding {
            // Encode as UTF-16LE base64 for PowerShell -EncodedCommand
            let utf16: Vec<u8> = command.encode_utf16()
                .flat_map(|c| c.to_le_bytes())
                .collect();
            let encoded = base64::engine::general_purpose::STANDARD.encode(&utf16);

            std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
                .current_dir(working_dir)
                .output()
        } else {
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", command])
                .current_dir(working_dir)
                .output()
        }
    };

    #[cfg(not(windows))]
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            (stdout, stderr, output.status.success())
        }
        Err(e) => {
            (String::new(), format!("Error: {}", e), false)
        }
    }
}

/// Check if a string looks like a valid shell command (basic heuristic)
fn looks_like_shell_command(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Reject obvious non-commands
    let first_word = trimmed.split_whitespace().next().unwrap_or("");

    // CRITICAL: Reject ls -la output (permission strings like drwxrwxr-x, -rw-r--r--)
    if Regex::new(r"^[d\-][rwx\-]{9}").unwrap().is_match(first_word) {
        return false;
    }
    // Reject "total NNN" from ls -la output
    if first_word == "total" {
        return false;
    }

    // Must start with a valid command character (letter, dot, slash, or parenthesis for PowerShell expressions)
    let first_char = first_word.chars().next().unwrap_or(' ');
    if !first_char.is_ascii_alphabetic() && first_char != '.' && first_char != '/' && first_char != '(' && first_char != '$' {
        return false;
    }

    // Reject things that look like config file content
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return false;  // [section] headers
    }
    if trimmed.contains(" = ") && !trimmed.contains("export ") && !trimmed.contains("&&") {
        return false;  // key = value (config lines)
    }

    // Reject markdown-style content
    if first_word.starts_with('#') && first_word.len() <= 3 {
        return false;  // ## headers
    }
    if trimmed.starts_with("---") || trimmed.starts_with("```") {
        return false;
    }

    // Reject obvious prose
    let prose_starts = ["If ", "When ", "You ", "The ", "This ", "From ", "On ", "It ", "Use ", "Great", "What ", "Just ", "Once "];
    for prefix in prose_starts {
        if trimmed.starts_with(prefix) {
            return false;
        }
    }

    // Reject date/time outputs (e.g., "Tuesday, January 20, 2026 11:36:58 AM")
    let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    for day in days {
        if first_word == day || first_word == &format!("{},", day) {
            return false;
        }
    }

    // Reject numbered list items that aren't commands
    if Regex::new(r"^\d+\.?\s+[A-Z]").unwrap().is_match(trimmed) {
        return false;  // "1. Add the user" style
    }

    // Reject lines that look like bullet points or list items
    if trimmed.starts_with("- ") && trimmed.len() > 2 && trimmed.chars().nth(2).map(|c| c.is_uppercase()).unwrap_or(false) {
        return false;  // "- Explore a sub-folder" style
    }

    // Reject code examples (not actual commands to execute)
    // Rust code patterns
    if trimmed.starts_with("///") || trimmed.starts_with("//!") {
        return false;  // Rust doc comments
    }
    if first_word == "fn" || first_word == "pub" || first_word == "impl" || first_word == "struct" || first_word == "enum" {
        return false;  // Rust declarations
    }
    if trimmed.starts_with("let ") && trimmed.contains(" = ") && !trimmed.contains("$(") {
        return false;  // Rust/JS variable assignment (not shell export)
    }

    // Python code patterns
    if first_word == "def" || first_word == "class" || first_word == "import" || first_word == "from" {
        return false;  // Python declarations
    }
    if trimmed.starts_with("print(") || trimmed.starts_with("return ") || trimmed.starts_with("raise ") {
        return false;  // Python statements
    }
    if trimmed.starts_with(">>> ") || trimmed.starts_with("... ") {
        return false;  // Python REPL examples
    }

    // JavaScript/TypeScript code patterns
    if first_word == "const" || first_word == "function" || first_word == "async" || first_word == "await" {
        return false;  // JS declarations
    }
    if trimmed.starts_with("console.log(") || trimmed.starts_with("module.exports") {
        return false;  // JS statements
    }

    // Generic code patterns
    if trimmed.contains("->") && trimmed.contains("(") && !trimmed.contains("|") {
        return false;  // Type annotations
    }
    if trimmed.ends_with(";") && !first_word.chars().all(|c| c.is_ascii_alphabetic() || c == '-' || c == '_') {
        return false;  // Code statements ending in semicolon
    }

    // Reject lines that look like error messages from the model
    if first_word == "error:" || first_word == "Error:" || first_word == "warning:" {
        return false;
    }

    // Reject assert statements (test code)
    if first_word == "assert" || trimmed.starts_with("assert_") {
        return false;
    }

    // Reject lines that are clearly examples in documentation
    if trimmed.starts_with("$") && trimmed.len() > 1 && trimmed.chars().nth(1) == Some(' ') {
        // "$ command" style examples - extract the actual command
        return false;  // We'll handle these differently
    }

    true
}

/// Extract command from various JSON formats that models output
/// Handles: {"cmd":[...]}, {"command":"..."}, {"tool":"shell","arguments":{...}}, etc.
fn extract_command_from_json(response: &str) -> Option<String> {
    // First, try to find complete JSON objects by brace matching
    // This handles multi-line JSON better than regex
    let response_clean = response.replace('\n', " ").replace('\r', " ");

    // Find JSON objects by brace matching
    let mut json_candidates: Vec<String> = Vec::new();
    let chars: Vec<char> = response_clean.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '{' {
            let start = i;
            let mut depth = 1;
            i += 1;

            while i < chars.len() && depth > 0 {
                match chars[i] {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }

            if depth == 0 {
                let json_str: String = chars[start..i].iter().collect();
                json_candidates.push(json_str);
            }
        } else {
            i += 1;
        }
    }

    // Also try with regex for simpler cases
    let json_re = Regex::new(r"\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}").unwrap();
    for cap in json_re.find_iter(&response_clean) {
        let json_str = cap.as_str().to_string();
        if !json_candidates.contains(&json_str) {
            json_candidates.push(json_str);
        }
    }

    for json_str in json_candidates {
        // Try to parse as JSON
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
            // Format 1: {"cmd": ["powershell", "-Command", "Get-Date"]} or {"cmd": ["command"]}
            if let Some(cmd_array) = value.get("cmd").and_then(|v| v.as_array()) {
                let parts: Vec<&str> = cmd_array.iter()
                    .filter_map(|v| v.as_str())
                    .collect();

                if !parts.is_empty() {
                    // If it's a shell invocation, extract the actual command
                    let first = parts[0].to_lowercase();
                    if ["bash", "sh", "powershell", "pwsh", "cmd"].contains(&first.as_str()) {
                        // Find the actual command after -c, -Command, etc.
                        for (i, part) in parts.iter().enumerate() {
                            if *part == "-c" || *part == "-lc" || part.eq_ignore_ascii_case("-Command") {
                                if i + 1 < parts.len() {
                                    return Some(parts[i + 1].to_string());
                                }
                            }
                        }
                        // If no flag found, join remaining parts
                        if parts.len() > 1 {
                            return Some(parts[1..].join(" "));
                        }
                    } else {
                        // Direct command array
                        return Some(parts.join(" "));
                    }
                }
            }

            // Format 2: {"command": "ls -la"} or {"command": [...]}
            if let Some(cmd) = value.get("command") {
                if let Some(s) = cmd.as_str() {
                    return Some(s.to_string());
                }
                if let Some(arr) = cmd.as_array() {
                    let parts: Vec<&str> = arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect();
                    if !parts.is_empty() {
                        return Some(parts.join(" "));
                    }
                }
            }

            // Format 3: {"tool": "shell", "arguments": {"command": "..."}}
            if let Some(tool) = value.get("tool").or(value.get("name")).and_then(|v| v.as_str()) {
                if ["shell", "powershell", "bash", "execute", "run"].contains(&tool.to_lowercase().as_str()) {
                    if let Some(args) = value.get("arguments") {
                        if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                            return Some(cmd.to_string());
                        }
                        if let Some(cmd) = args.get("cmd").and_then(|v| v.as_str()) {
                            return Some(cmd.to_string());
                        }
                    }
                }
            }

            // Format 4: {"tool_name": "shell", "arguments": {"command": "..."}}
            if let Some(tool) = value.get("tool_name").and_then(|v| v.as_str()) {
                if ["shell", "powershell", "bash", "execute", "run"].contains(&tool.to_lowercase().as_str()) {
                    if let Some(args) = value.get("arguments") {
                        if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                            return Some(cmd.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

/// Extract bash/shell code blocks from AI response
/// CONSERVATIVE: Limits to first command only, validates command-like structure
fn extract_commands(response: &str) -> Vec<String> {
    let mut commands = Vec::new();

    // Method 1: Standard markdown code blocks with explicit language tag
    // Support bash/sh/shell for Unix, powershell/pwsh/cmd for Windows
    let re = Regex::new(r"```(?:bash|sh|shell|powershell|pwsh|cmd)\n([\s\S]*?)```").unwrap();

    for cap in re.captures_iter(response) {
        if let Some(m) = cap.get(1) {
            let block_content = m.as_str();

            // Process each line in the code block
            for line in block_content.lines() {
                let trimmed = line.trim();

                // Skip empty lines and comment-only lines
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                // Strip inline comments
                let cmd = strip_shell_comment(trimmed).trim();

                // Validate it looks like a command
                if !cmd.is_empty() && looks_like_shell_command(cmd) {
                    commands.push(cmd.to_string());
                    // LIMIT: Only take the FIRST valid command
                    return commands;
                }
            }
        }
    }

    // Method 2: Local model JSON format - use proper JSON parsing for flexibility
    // Handles: {"cmd":[...]}, {"command":"..."}, {"tool":"shell","arguments":{"command":"..."}}
    if commands.is_empty() {
        if let Some(cmd) = extract_command_from_json(response) {
            if looks_like_shell_command(&cmd) {
                commands.push(cmd);
                return commands;
            }
        }
    }

    // Method 3: Unmarked code blocks (no language tag) - common with local models
    // Only try if no commands found yet
    if commands.is_empty() {
        let unmarked_re = Regex::new(r"```\n([\s\S]*?)```").unwrap();
        for cap in unmarked_re.captures_iter(response) {
            if let Some(m) = cap.get(1) {
                let block_content = m.as_str().trim();
                // Only consider single-line blocks that look like commands
                let lines: Vec<&str> = block_content.lines().collect();
                if lines.len() <= 3 {
                    for line in lines {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with('#') && looks_like_shell_command(trimmed) {
                            commands.push(trimmed.to_string());
                            return commands;
                        }
                    }
                }
            }
        }
    }

    commands
}

/// Extract MCP tool calls from AI response
/// Format: ```tool\ntool_name: server:tool\narguments:\n  key: value\n```
fn extract_tool_calls(response: &str) -> Vec<(String, serde_json::Value)> {
    let mut calls = Vec::new();

    // Format 1: Local LLM channel format
    // <|channel|>commentary to=tool_id <|constrain|>json<|message|>{json}
    let channel_re = Regex::new(r"<\|channel\|>commentary to=([^\s<]+)\s*<\|constrain\|>json<\|message\|>(\{.+)").unwrap();
    if let Some(cap) = channel_re.captures(response) {
        if let (Some(tool_match), Some(json_start)) = (cap.get(1), cap.get(2)) {
            // Remove common prefixes the AI might add
            let raw_name = tool_match.as_str();
            let tool_name = raw_name
                .strip_prefix("tool:")
                .or_else(|| raw_name.strip_prefix("tool."))
                .or_else(|| raw_name.strip_prefix("tool_name:"))
                .unwrap_or(raw_name)
                .to_string();
            // Try to parse JSON, finding the correct end by trying progressively longer strings
            let json_str = json_start.as_str();
            for end in (1..=json_str.len()).rev() {
                if let Ok(args) = serde_json::from_str::<serde_json::Value>(&json_str[..end]) {
                    debug!("Extracted tool call (channel format): {} with {:?}", tool_name, args);
                    return vec![(tool_name, args)];
                }
            }
        }
    }

    // Format 2: ```tool code blocks (standard markdown format)
    let re = Regex::new(r"```tool\n([\s\S]*?)```").unwrap();

    for cap in re.captures_iter(response) {
        if let Some(m) = cap.get(1) {
            let content = m.as_str();

            // Parse the YAML-like format
            let mut tool_name = String::new();
            let mut args = serde_json::Map::new();
            let mut in_arguments = false;

            for line in content.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with("tool_name:") {
                    tool_name = trimmed.strip_prefix("tool_name:").unwrap().trim().to_string();
                } else if trimmed == "arguments:" {
                    in_arguments = true;
                } else if in_arguments && trimmed.contains(':') {
                    // Parse argument line (key: value)
                    if let Some((key, value)) = trimmed.split_once(':') {
                        let key = key.trim().to_string();
                        let value = value.trim();

                        // Try to parse as JSON value, otherwise use as string
                        let json_value = if value.starts_with('"') && value.ends_with('"') {
                            serde_json::Value::String(value[1..value.len()-1].to_string())
                        } else if value == "true" {
                            serde_json::Value::Bool(true)
                        } else if value == "false" {
                            serde_json::Value::Bool(false)
                        } else if let Ok(n) = value.parse::<i64>() {
                            serde_json::Value::Number(n.into())
                        } else if let Ok(n) = value.parse::<f64>() {
                            serde_json::json!(n)
                        } else {
                            serde_json::Value::String(value.to_string())
                        };

                        args.insert(key, json_value);
                    }
                }
            }

            if !tool_name.is_empty() {
                calls.push((tool_name, serde_json::Value::Object(args)));
                // Only process first tool call per response
                return calls;
            }
        }
    }

    // Format 3: Raw JSON tool call (OpenAI function call style)
    // {"name":"tool_name","arguments":{...}} or {"name":"tool_name","arguments":"{...}"}
    let json_re = Regex::new(r#"\{[^{}]*"name"\s*:\s*"([^"]+)"[^{}]*"arguments"\s*:\s*(\{[^{}]*\}|\{[^{}]*\{[^{}]*\}[^{}]*\}|"[^"]*")[^{}]*\}"#).unwrap();
    if let Some(cap) = json_re.captures(response) {
        if let (Some(name_match), Some(args_match)) = (cap.get(1), cap.get(2)) {
            let tool_name = name_match.as_str().to_string();
            let args_str = args_match.as_str();

            // Handle both JSON object and stringified JSON
            let args: serde_json::Value = if args_str.starts_with('"') && args_str.ends_with('"') {
                // Stringified JSON - parse the inner string
                let inner = &args_str[1..args_str.len()-1];
                serde_json::from_str(inner).unwrap_or(serde_json::json!({}))
            } else {
                // Direct JSON object
                serde_json::from_str(args_str).unwrap_or(serde_json::json!({}))
            };

            debug!("Extracted tool call (JSON format): {} with {:?}", tool_name, args);
            return vec![(tool_name, args)];
        }
    }

    // Format 4: Simple JSON on its own line
    // Try to find any line that looks like a complete JSON tool call
    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.contains("\"name\"") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let (Some(name), Some(args)) = (json.get("name"), json.get("arguments")) {
                    if let Some(name_str) = name.as_str() {
                        let arguments = args.clone();
                        debug!("Extracted tool call (line JSON format): {} with {:?}", name_str, arguments);
                        return vec![(name_str.to_string(), arguments)];
                    }
                }
            }
        }
    }

    calls
}

/// Execute an MCP tool call
async fn execute_tool_call(
    tool_id: &str,
    arguments: serde_json::Value,
    state: &ReplState
) -> anyhow::Result<String> {
    // Fix tool ID if it doesn't have proper format (server:tool)
    let fixed_tool_id = if !tool_id.contains(':') {
        // Try to find matching tool in available tools
        let matching: Vec<_> = state.mcp_tools.iter()
            .map(|(k, _)| k)
            .filter(|k| k.ends_with(&format!(":{}", tool_id)) || k.ends_with(&format!("_{}", tool_id)))
            .collect();

        if matching.len() == 1 {
            matching[0].clone()
        } else if tool_id.starts_with("puppeteer") {
            // Common case: puppeteer tools
            format!("puppeteer:{}", tool_id)
        } else {
            tool_id.to_string()
        }
    } else {
        // Clean up any remaining issues (e.g., tool_puppeteer -> puppeteer)
        tool_id.replace("tool_", "").replace("tool.", "")
    };

    info!("Executing MCP tool: {} with args: {:?}", fixed_tool_id, arguments);

    match state.mcp_manager.call_tool(&fixed_tool_id, arguments).await {
        Ok(response) => {
            // Format the response content
            let mut result = String::new();
            debug!("Tool response: {:?}", response);
            if let Some(content_blocks) = &response.content {
                for content in content_blocks {
                    match content {
                        ganesha_mcp::types::ContentBlock::Text { text } => {
                            result.push_str(text);
                            result.push('\n');
                        }
                        ganesha_mcp::types::ContentBlock::Resource { text, .. } => {
                            if let Some(t) = text {
                                result.push_str(t);
                                result.push('\n');
                            }
                        }
                        ganesha_mcp::types::ContentBlock::Image { data, .. } => {
                            result.push_str(&format!("[Image: {} bytes]\n", data.len()));
                        }
                    }
                }
            }
            Ok(result.trim().to_string())
        }
        Err(e) => {
            Err(anyhow::anyhow!("Tool call failed: {}", e))
        }
    }
}

/// Check if a command requires interactive terminal input
fn is_interactive_command(cmd: &str) -> Option<&'static str> {
    let first_word = cmd.split_whitespace()
        .find(|w| *w != "sudo" && *w != "env")
        .unwrap_or("");

    match first_word {
        "nano" | "vim" | "vi" | "emacs" | "pico" | "joe" => {
            Some("Use 'tee' or 'cat <<EOF' to write files non-interactively")
        }
        "passwd" => {
            Some("Use 'chpasswd' for non-interactive password changes")
        }
        "smbpasswd" if !cmd.contains("-a") && !cmd.contains("-e") && !cmd.contains("-x") => {
            Some("Use 'echo -e \"pass\\npass\" | sudo smbpasswd -s -a user' for non-interactive")
        }
        "mysql" | "psql" | "sqlite3" if !cmd.contains("-e") && !cmd.contains("-c") => {
            Some("Use -e or -c flag to run queries non-interactively")
        }
        "ssh" if !cmd.contains("-t") && !cmd.contains("'") && !cmd.contains("\"") => {
            Some("SSH requires a command argument for non-interactive use")
        }
        _ => None
    }
}

/// Clean AI response by removing control tokens and formatting markers
fn clean_response(response: &str) -> String {
    let mut cleaned = response.to_string();

    // Remove common control tokens from local models
    let patterns = [
        r"<\|channel\|>[^<]*<\|message\|>",
        r"<\|[a-z_]+\|>",
        r#"\{"cmd"\s*:\s*\["bash"[^\}]+\}"#,
        r#"\\n"#,  // Literal \n in output
        r#"\\"timeout"\s*:\s*\d+"#,  // JSON timeout field
        r"commentary to=[a-z._]+ [a-z]+",  // "commentary to=container.exec json"
        r"to=container\.[a-z]+ [a-z]+",  // fallback for container tokens
    ];

    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            cleaned = re.replace_all(&cleaned, "").to_string();
        }
    }

    // Replace escaped newlines with actual newlines
    cleaned = cleaned.replace("\\n", "\n");

    // Remove JSON artifacts
    cleaned = cleaned.replace("\"], \"timeout\":", "");
    cleaned = cleaned.replace("EOF\"], ", "");

    // Clean up multiple spaces and whitespace
    let space_re = Regex::new(r"  +").unwrap();
    cleaned = space_re.replace_all(&cleaned, " ").to_string();

    // Remove trailing incomplete sentences that look like protocol leakage
    let lines: Vec<&str> = cleaned.lines().collect();
    let cleaned_lines: Vec<&str> = lines.into_iter()
        .filter(|line| {
            let trimmed = line.trim().to_lowercase();
            !trimmed.starts_with("commentary") &&
            !trimmed.starts_with("to=") &&
            !trimmed.contains("container.exec")
        })
        .collect();

    cleaned_lines.join("\n").trim().to_string()
}

/// Strip inline shell comments from a command
fn strip_shell_comment(cmd: &str) -> &str {
    // Find # that's not inside quotes
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for (i, c) in cmd.char_indices() {
        match c {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '#' if !in_single_quote && !in_double_quote => {
                return cmd[..i].trim_end();
            }
            _ => {}
        }
    }
    cmd
}

/// Check if a command is a cd command and handle directory change
/// Returns (result_message, remaining_command) if cd was found
fn handle_cd_command(command: &str, state: &mut ReplState) -> Option<(String, Option<String>)> {
    // Strip inline comments first
    let trimmed = strip_shell_comment(command.trim());

    // Check for compound commands (cd path && other_cmd or cd path; other_cmd)
    let (cd_part, remaining) = if let Some(idx) = trimmed.find("&&") {
        (&trimmed[..idx], Some(trimmed[idx+2..].trim().to_string()))
    } else if let Some(idx) = trimmed.find(';') {
        (&trimmed[..idx], Some(trimmed[idx+1..].trim().to_string()))
    } else {
        (trimmed, None)
    };

    let cd_part = cd_part.trim();

    if cd_part == "cd" || cd_part.starts_with("cd ") {
        let parts: Vec<&str> = cd_part.splitn(2, ' ').collect();
        let target = if parts.len() > 1 {
            let path = strip_shell_comment(parts[1].trim());
            if path.starts_with("~/") {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/"))
                    .join(&path[2..])
            } else if path == "~" {
                dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
            } else if path == "-" {
                return Some(("cd - not yet implemented".to_string(), remaining));
            } else {
                let p = PathBuf::from(path);
                if p.is_absolute() { p } else { state.working_dir.join(p) }
            }
        } else {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        };

        match target.canonicalize() {
            Ok(canonical) if canonical.is_dir() => {
                state.working_dir = canonical.clone();
                Some((format!("Changed directory to: {}", canonical.display()), remaining))
            }
            Ok(_) => Some((format!("Not a directory: {}", target.display()), None)),
            Err(_) => Some((format!("No such directory: {}", target.display()), None)),
        }
    } else {
        None
    }
}

/// Agentic chat - sends message to AI and handles command execution loop
async fn agentic_chat(user_message: &str, state: &mut ReplState) -> anyhow::Result<String> {
    // Allow many iterations for complex tasks (coding, research, web scraping)
    const MAX_ITERATIONS: usize = 50;
    const MAX_CONSECUTIVE_FAILURES: usize = 3;

    // Check if we have a provider
    if !state.provider_manager.has_available_provider().await {
        return Err(anyhow::anyhow!(
            "No LLM providers available. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or start a local server."
        ));
    }

    // Session logging is started at startup, just mark first message done
    if state.is_first_message {
        state.is_first_message = false;
    }

    // Log user message
    if let Err(e) = state.session_logger.log_user(user_message) {
        eprintln!("{} Failed to log: {}", "Warning:".yellow(), e);
    }

    // Add user message
    state.messages.push(Message::user(user_message));

    let mut iteration = 0;
    let mut consecutive_failures = 0;
    let mut last_command: Option<String> = None;

    loop {
        iteration += 1;
        if iteration > MAX_ITERATIONS {
            return Ok("Reached maximum iterations. Please ask a follow-up question if you need more.".to_string());
        }

        // Build messages with agentic system prompt
        let system = agentic_system_prompt(state);
        let mut messages = vec![Message::system(&system)];
        messages.extend(state.messages.clone());

        let options = GenerateOptions {
            model: state.model.clone(),
            temperature: Some(0.7),
            max_tokens: Some(2048),  // Reduced to encourage concise responses
            ..Default::default()
        };

        // Show spinner while waiting for AI response
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_style(
            indicatif::ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .unwrap()
        );
        spinner.set_message("Thinking...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        // Get AI response
        let response = state.provider_manager.chat(&messages, &options).await?;
        spinner.finish_and_clear();
        let content = response.content.clone();

        // Extract any commands or tool calls from the response
        let commands = extract_commands(&content);
        let tool_calls = extract_tool_calls(&content);

        // Handle tool calls first (if any)
        if !tool_calls.is_empty() {
            let (tool_id, args) = &tool_calls[0];

            // Special handling for container.exec (used by some fine-tuned models)
            // Convert to shell command execution
            if tool_id == "container.exec" || tool_id.starts_with("container.") {
                if let Some(cmd) = args.get("cmd").or_else(|| args.get("command")) {
                    let command = if let Some(arr) = cmd.as_array() {
                        // Format: {"cmd": ["bash", "-c", "ls -la"]}
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else if let Some(s) = cmd.as_str() {
                        s.to_string()
                    } else {
                        format!("{}", cmd)
                    };

                    // Extract actual command (skip bash -c or sh -c prefix)
                    let actual_cmd = if command.starts_with("bash -c ") || command.starts_with("sh -c ") {
                        command.splitn(3, ' ').nth(2).unwrap_or(&command).trim_matches('"').to_string()
                    } else if command.starts_with("bash -lc ") || command.starts_with("sh -lc ") {
                        command.splitn(3, ' ').nth(2).unwrap_or(&command).trim_matches('"').to_string()
                    } else {
                        command
                    };

                    println!("{} {}", "→".bright_blue(), actual_cmd.dimmed());
                    let (stdout, stderr, success) = run_shell_command(&actual_cmd, &state.working_dir);

                    let output = if !stdout.is_empty() && !stderr.is_empty() {
                        format!("{}\n{}", stdout, stderr)
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        stderr
                    };

                    // Print brief output
                    if !output.is_empty() {
                        for line in output.lines().take(10) {
                            println!("  {}", line);
                        }
                        if output.lines().count() > 10 {
                            println!("  ... {} more lines", output.lines().count() - 10);
                        }
                    }

                    if let Err(e) = state.session_logger.log_command(&actual_cmd, &output, success) {
                        debug!("Failed to log command: {}", e);
                    }

                    state.messages.push(Message::assistant(&content));
                    state.messages.push(Message::user(&format!(
                        "Command output:\n```\n{}\n```\n\nBriefly describe what you found, then continue if needed.",
                        output
                    )));
                    consecutive_failures = 0;
                    continue;
                }
            }

            // Special handling for generic shell tools (shell, shell_commands, execute, run, etc.)
            // Models sometimes call these as if they were MCP tools
            let shell_tool_names = ["shell", "shell_commands", "execute", "run", "bash", "powershell", "terminal"];
            let tool_lower = tool_id.to_lowercase();
            if shell_tool_names.iter().any(|&name| tool_lower == name || tool_lower.ends_with(&format!(":{}", name))) {
                // Try to extract command from arguments
                let command = args.get("command")
                    .or(args.get("cmd"))
                    .or(args.get("script"))
                    .or(args.get("code"))
                    .and_then(|v| v.as_str());

                if let Some(cmd) = command {
                    println!("{} {} → {}", "⚡".bright_cyan(), tool_id.bright_white(), cmd.dimmed());
                    let (stdout, stderr, success) = run_shell_command(cmd, &state.working_dir);

                    let output = if !stdout.is_empty() && !stderr.is_empty() {
                        format!("{}\n{}", stdout, stderr)
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        stderr
                    };

                    // Print brief output
                    if !output.is_empty() {
                        for line in output.lines().take(10) {
                            println!("  {}", line);
                        }
                        if output.lines().count() > 10 {
                            println!("  ... {} more lines", output.lines().count() - 10);
                        }
                    }

                    state.messages.push(Message::assistant(&content));
                    state.messages.push(Message::user(&format!(
                        "Command output:\n```\n{}\n```\n\nBriefly describe what you found, then continue if needed.",
                        output
                    )));
                    if success { consecutive_failures = 0; } else { consecutive_failures += 1; }
                    continue;
                } else {
                    // No command found in arguments
                    state.messages.push(Message::assistant(&content));
                    state.messages.push(Message::user(
                        "Tool error: shell tool requires a 'command' argument. Please provide the command to execute."
                    ));
                    consecutive_failures += 1;
                    continue;
                }
            }

            // Special handling for repo_browser.* tools (used by some fine-tuned models)
            // These aren't real MCP tools - convert to shell commands
            if tool_id.starts_with("repo_browser.") || tool_id.starts_with("repo_browser_") {
                let tool_name = tool_id.split('.').last().unwrap_or(tool_id);

                let shell_cmd = match tool_name {
                    "print_tree" | "tree" | "list_files" => {
                        let path = args.get("path")
                            .or(args.get("directory"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(".");
                        #[cfg(windows)]
                        { format!("Get-ChildItem -Path '{}' -Recurse -Depth 2 | Select-Object FullName", path) }
                        #[cfg(not(windows))]
                        { format!("find {} -maxdepth 3 -type f 2>/dev/null | head -50", path) }
                    }
                    "get_file" | "read_file" | "open_file" => {
                        if let Some(path) = args.get("path").or(args.get("file")).and_then(|v| v.as_str()) {
                            #[cfg(windows)]
                            { format!("Get-Content -Path '{}'", path) }
                            #[cfg(not(windows))]
                            { format!("cat '{}'", path) }
                        } else {
                            // Can't execute without a path
                            state.messages.push(Message::assistant(&content));
                            state.messages.push(Message::user(
                                "Tool error: repo_browser requires a 'path' argument. Try a different approach."
                            ));
                            consecutive_failures += 1;
                            continue;
                        }
                    }
                    "search" | "grep" | "find" => {
                        let pattern = args.get("pattern")
                            .or(args.get("query"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("*");
                        let path = args.get("path")
                            .or(args.get("directory"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(".");
                        #[cfg(windows)]
                        { format!("Get-ChildItem -Path '{}' -Recurse -File | Select-String -Pattern '{}'", path, pattern) }
                        #[cfg(not(windows))]
                        { format!("grep -r '{}' {} 2>/dev/null | head -20", pattern, path) }
                    }
                    _ => {
                        // Unknown repo_browser tool - tell the model it's not available
                        state.messages.push(Message::assistant(&content));
                        state.messages.push(Message::user(&format!(
                            "Tool '{}' is not available. Use shell commands instead (Get-ChildItem, Get-Content, Select-String on Windows).",
                            tool_id
                        )));
                        consecutive_failures += 1;
                        continue;
                    }
                };

                println!("{} {} → {}", "⚡".bright_cyan(), tool_name.bright_white(), shell_cmd.dimmed());
                let (stdout, stderr, success) = run_shell_command(&shell_cmd, &state.working_dir);

                let output = if !stdout.is_empty() && !stderr.is_empty() {
                    format!("{}\n{}", stdout, stderr)
                } else if !stdout.is_empty() {
                    stdout
                } else {
                    stderr
                };

                // Print brief output
                if !output.is_empty() {
                    for line in output.lines().take(10) {
                        println!("  {}", line);
                    }
                    if output.lines().count() > 10 {
                        println!("  ... {} more lines", output.lines().count() - 10);
                    }
                }

                state.messages.push(Message::assistant(&content));
                state.messages.push(Message::user(&format!(
                    "Command output:\n```\n{}\n```\n\nBriefly describe what you found, then continue if needed.",
                    output
                )));
                if success { consecutive_failures = 0; } else { consecutive_failures += 1; }
                continue;
            }

            // Show brief tool name (remove server prefix for display)
            let short_name = tool_id.split(':').last().unwrap_or(tool_id);
            print!("{} {}", "⚡".bright_cyan(), short_name.bright_white());

            match execute_tool_call(tool_id, args.clone(), state).await {
                Ok(result) => {
                    // Show brief success indicator - just first line or truncated
                    let first_line = result.lines().next().unwrap_or("").trim();
                    if !first_line.is_empty() && first_line.len() < 80 {
                        println!(" → {}", first_line.dimmed());
                    } else if result.len() > 0 {
                        println!(" → {} bytes", result.len());
                    } else {
                        println!(" → done");
                    }

                    // Log the tool call
                    if let Err(e) = state.session_logger.log_command(
                        &format!("tool:{}", tool_id),
                        &result,
                        true
                    ) {
                        debug!("Failed to log tool call: {}", e);
                    }

                    state.messages.push(Message::assistant(&content));
                    state.messages.push(Message::user(&format!(
                        "Tool result for {}:\n```\n{}\n```\n\nDescribe the result and continue if needed.",
                        tool_id, result
                    )));
                    consecutive_failures = 0;
                }
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);

                    state.messages.push(Message::assistant(&content));
                    state.messages.push(Message::user(&format!(
                        "Tool error for {}: {}\nTry a different approach.",
                        tool_id, e
                    )));
                    consecutive_failures += 1;

                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        return Ok(format!("Tool calls failing repeatedly. Last error: {}", e));
                    }
                }
            }
            continue;
        }

        if commands.is_empty() {
            // No commands or tools - this is the final response
            // Clean the response to remove any control tokens from local models
            let cleaned = clean_response(&content);
            state.messages.push(Message::assistant(&cleaned));

            // Log assistant response
            if let Err(e) = state.session_logger.log_assistant(&cleaned) {
                debug!("Failed to log assistant response: {}", e);
            }

            return Ok(cleaned);
        }

        // Only execute the first command (extract_commands already limits this)
        let cmd = &commands[0];

        // Check for interactive commands that won't work
        if let Some(hint) = is_interactive_command(cmd) {
            println!("{} {} {}", "⚠".yellow(), "Skipping interactive command:".yellow(), cmd.dimmed());
            println!("  {}", hint.dimmed());

            // Tell the AI the command was skipped
            state.messages.push(Message::assistant(&content));
            state.messages.push(Message::user(&format!(
                "Command skipped (requires interactive terminal): {}\nHint: {}\nPlease use a non-interactive alternative.",
                cmd, hint
            )));
            continue;
        }

        // Check for repeated command (AI might be stuck in a loop)
        if let Some(ref last) = last_command {
            if last == cmd {
                state.messages.push(Message::assistant(&content));
                return Ok(format!("Stopping: repeated command detected. Last output shown above."));
            }
        }
        last_command = Some(cmd.clone());

        // Print the command being executed
        println!("{} {}", "→".bright_blue(), cmd.dimmed());

        // Check for cd commands (need special handling)
        if let Some((cd_result, remaining)) = handle_cd_command(cmd, state) {
            println!("{}", cd_result.dimmed());
            let mut output = format!("$ {}\n{}", cmd, cd_result);

            // If there's a remaining command after cd, execute it
            if let Some(rest) = remaining {
                if !rest.is_empty() {
                    println!("{} {}", "→".bright_blue(), rest.dimmed());
                    let (stdout, stderr, success) = run_shell_command(&rest, &state.working_dir);
                    if !stdout.is_empty() {
                        print!("{}", stdout);
                    }
                    if !stderr.is_empty() {
                        eprint!("{}", stderr.red());
                    }
                    output.push_str(&format!("\n\n$ {}\n{}{}", rest, stdout, stderr));

                    if !success {
                        consecutive_failures += 1;
                    } else {
                        consecutive_failures = 0;
                    }
                }
            }

            state.messages.push(Message::assistant(&content));
            state.messages.push(Message::user(&format!("Command output:\n```\n{}\n```\n\nBriefly describe the result.", output)));
            continue;
        }

        // Execute the command
        let (stdout, stderr, success) = run_shell_command(cmd, &state.working_dir);

        // Print brief output summary (not the full content for readability)
        let combined_len = stdout.len() + stderr.len();
        if combined_len > 200 {
            // Show brief summary for long output
            let first_lines: Vec<&str> = stdout.lines().take(3).collect();
            if !first_lines.is_empty() {
                for line in &first_lines {
                    println!("  {}", line.dimmed());
                }
                if stdout.lines().count() > 3 {
                    println!("  {} more lines...", "...".dimmed());
                }
            }
            if !stderr.is_empty() {
                let err_first = stderr.lines().next().unwrap_or("");
                eprintln!("  {} {}", "⚠".yellow(), err_first.red());
            }
        } else {
            // Short output - show all
            if !stdout.is_empty() {
                for line in stdout.lines() {
                    println!("  {}", line.dimmed());
                }
            }
            if !stderr.is_empty() {
                for line in stderr.lines() {
                    eprintln!("  {}", line.red());
                }
            }
        }

        // Log the command execution
        let combined_output = format!("{}{}", stdout, stderr);
        if let Err(e) = state.session_logger.log_command(cmd, &combined_output, success) {
            debug!("Failed to log command: {}", e);
        }

        // Track failures
        if !success {
            consecutive_failures += 1;
            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                let result = format!("$ {} (failed)\n{}{}", cmd, stdout, stderr);
                state.messages.push(Message::assistant(&content));
                state.messages.push(Message::user(&format!("Command output:\n```\n{}\n```", result)));
                return Ok(format!("Stopping after {} consecutive failures. Please check the errors above.", MAX_CONSECUTIVE_FAILURES));
            }
        } else {
            consecutive_failures = 0;
        }

        // Collect result for feeding back to AI
        let result = if success {
            format!("$ {}\n{}", cmd, stdout)
        } else {
            format!("$ {} (failed)\n{}{}", cmd, stdout, stderr)
        };

        // Add AI response and command output to conversation
        state.messages.push(Message::assistant(&content));
        state.messages.push(Message::user(&format!("Command output:\n```\n{}\n```\n\nBriefly describe the result.", result)));
    }
}

/// Generate the agentic system prompt
fn agentic_system_prompt(state: &ReplState) -> String {
    use sysinfo::System;

    let mode_context = match state.mode {
        ChatMode::Code => "You are Ganesha, the Remover of Obstacles. You help users overcome coding challenges.",
        ChatMode::Ask => "You are Ganesha, the Remover of Obstacles. You help users overcome any challenge.",
        ChatMode::Architect => "You are Ganesha, the Remover of Obstacles. You help design systems and remove architectural blockers.",
        ChatMode::Help => "You are Ganesha's help system.",
    };

    let context_files = if state.context_files.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nContext files:\n{}",
            state.context_files
                .iter()
                .map(|p| format!("- {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    // Get MCP tools if available
    let tools_prompt = state.get_mcp_tools_prompt();

    // Platform-specific shell info
    let (os_name, shell_type, code_block_lang, list_cmd, list_example) = if cfg!(windows) {
        ("Windows", "PowerShell", "powershell", "Get-ChildItem", "Get-ChildItem -Force")
    } else if cfg!(target_os = "macos") {
        ("macOS", "sh", "shell", "ls", "ls -la")
    } else {
        ("Linux", "sh", "shell", "ls", "ls -la")
    };

    // Get system info
    let mut sys = System::new_all();
    sys.refresh_all();

    let os_version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
    let cpu_name = sys.cpus().first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_count = sys.cpus().len();
    let total_memory_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let available_memory_gb = sys.available_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let current_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();

    format!(
        r#"{mode_context}

SYSTEM:
- Date/Time: {current_time}
- OS: {os_version}
- CPU: {cpu_name} ({cpu_count} cores)
- Memory: {available_memory_gb:.1} GB available / {total_memory_gb:.1} GB total
- Shell: {shell_type}
- Working directory: {cwd}
{context_files}

CAPABILITIES:

1. SHELL COMMANDS - Execute ONE command at a time using {shell_type} syntax:
```{code_block_lang}
{list_example}
```
On {os_name}, use {shell_type} commands. For listing files: `{list_cmd}`. For changing directories: `cd`.

2. MCP TOOLS - Call external tools for web browsing, data gathering, etc:
```tool
tool_name: tool_name_here
arguments:
  key: value
```
{tools_prompt}

BEHAVIOR RULES:
- Think step-by-step to accomplish tasks
- Use {os_name}/{shell_type} compatible commands ONLY
- For shell commands: ONE per response, NON-INTERACTIVE only (no editors/password prompts)
- After results: briefly describe what you found, then continue if needed
- Be resourceful: if one approach fails, try another
- NEVER ask "what would you like to do" - just observe and report

The key is to be intelligent and use the right tool for each task."#,
        mode_context = mode_context,
        current_time = current_time,
        os_version = os_version,
        cpu_name = cpu_name,
        cpu_count = cpu_count,
        total_memory_gb = total_memory_gb,
        available_memory_gb = available_memory_gb,
        os_name = os_name,
        shell_type = shell_type,
        code_block_lang = code_block_lang,
        list_cmd = list_cmd,
        list_example = list_example,
        cwd = state.working_dir.display(),
        context_files = context_files,
        tools_prompt = tools_prompt
    )
}

/// Sanitize content by replacing problematic Unicode characters
fn sanitize_output(content: &str) -> String {
    content
        // Replace smart quotes with regular quotes
        .replace('\u{2018}', "'")  // Left single quote
        .replace('\u{2019}', "'")  // Right single quote (apostrophe)
        .replace('\u{201C}', "\"") // Left double quote
        .replace('\u{201D}', "\"") // Right double quote
        // Replace other problematic characters
        .replace('\u{2013}', "-")  // En dash
        .replace('\u{2014}', "--") // Em dash
        .replace('\u{2026}', "...") // Ellipsis
        .replace('\u{00A0}', " ")  // Non-breaking space
        // Replace any remaining non-ASCII that might cause issues
        .chars()
        .map(|c| {
            if c.is_ascii() || c == '│' || c == '─' || c == '┌' || c == '┐' || c == '└' || c == '┘' || c == '•' || c == '→' || c == '✓' || c == '✗' || c == '⚠' || c == '🐘' {
                c
            } else if !c.is_control() && c.is_alphanumeric() {
                c
            } else if c == '`' || c == '*' || c == '#' || c == '-' || c == '>' || c == '\n' || c == ' ' || c == '.' || c == ',' || c == ':' || c == ';' || c == '!' || c == '?' || c == '(' || c == ')' || c == '[' || c == ']' || c == '/' || c == '\\' || c == '=' || c == '+' || c == '_' || c == '@' || c == '$' || c == '%' || c == '^' || c == '&' || c == '<' || c == '>' || c == '|' || c == '{' || c == '}' || c == '~' {
                c
            } else {
                ' ' // Replace unknown chars with space
            }
        })
        .collect()
}

/// Print content in a styled Ganesha box with title and timestamp
/// Renders markdown for better readability with proper box borders and word wrap
fn print_ganesha_box(content: &str) {
    use unicode_width::UnicodeWidthStr;

    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();

    // Sanitize content first
    let content = sanitize_output(content);

    // Fixed box width for consistent appearance
    const BOX_WIDTH: usize = 72;
    const CONTENT_WIDTH: usize = 68; // BOX_WIDTH - 4 for "│ " and " │"

    // Helper to pad a string to exact width
    let pad_to_width = |s: &str, width: usize| -> String {
        let visible_len = UnicodeWidthStr::width(s);
        if visible_len >= width {
            s.to_string()
        } else {
            format!("{}{}", s, " ".repeat(width - visible_len))
        }
    };

    // Helper to print a bordered line
    let print_line = |content: &str| {
        let padded = pad_to_width(content, CONTENT_WIDTH);
        println!("{} {} {}", "│".cyan(), padded, "│".cyan());
    };

    // Helper to word-wrap and print with prefix
    let print_wrapped = |text: &str, first_prefix: &str, cont_prefix: &str| {
        let wrap_width = CONTENT_WIDTH - UnicodeWidthStr::width(first_prefix);
        let wrapped = textwrap::fill(text, wrap_width);
        for (i, line) in wrapped.lines().enumerate() {
            let prefix = if i == 0 { first_prefix } else { cont_prefix };
            let full_line = format!("{}{}", prefix, line);
            print_line(&full_line);
        }
    };

    println!();

    // Top border with title
    let title = format!(" Ganesha {} ", timestamp);
    let title_len = UnicodeWidthStr::width(title.as_str()) + 2; // +2 for emoji width
    let left_dashes = 3;
    let right_dashes = BOX_WIDTH.saturating_sub(left_dashes + title_len + 2);
    println!("{}{}{}{}{}",
        "┌".cyan(),
        "─".repeat(left_dashes).cyan(),
        format!(" 🐘{}", title).bright_green().bold(),
        "─".repeat(right_dashes).cyan(),
        "┐".cyan()
    );

    // Empty line after header
    print_line("");

    // Process content with markdown-aware rendering
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Handle code blocks
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            print_line(&format!("  {}", "─".repeat(CONTENT_WIDTH - 4)));
            continue;
        }

        if in_code_block {
            print_line(&format!("    {}", trimmed));
            continue;
        }

        // Check for headers
        if trimmed.starts_with("### ") {
            print_line(&format!("   {}", &trimmed[4..]));
        } else if trimmed.starts_with("## ") {
            print_line("");
            print_line(&format!("  {}", &trimmed[3..]));
        } else if trimmed.starts_with("# ") {
            print_line("");
            print_line(&format!(" {}", &trimmed[2..]));
        }
        // Check for bullet points
        else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            print_wrapped(&trimmed[2..], "  • ", "    ");
        }
        // Check for numbered lists
        else if trimmed.len() > 2 && trimmed.chars().next().map(|c| c.is_numeric()).unwrap_or(false)
            && (trimmed.contains(". ") || trimmed.contains(") "))
        {
            if let Some(pos) = trimmed.find(". ").or_else(|| trimmed.find(") ")) {
                let (num, rest) = trimmed.split_at(pos + 2);
                let prefix = format!("  {} ", num.trim());
                let cont_prefix = " ".repeat(prefix.len());
                print_wrapped(rest.trim(), &prefix, &cont_prefix);
            } else {
                print_wrapped(trimmed, "  ", "  ");
            }
        }
        // Empty lines
        else if trimmed.is_empty() {
            print_line("");
        }
        // Regular text
        else {
            print_wrapped(trimmed, "  ", "  ");
        }
    }

    // Empty line before footer
    print_line("");

    // Bottom border
    println!("{}{}{}",
        "└".cyan(),
        "─".repeat(BOX_WIDTH - 2).cyan(),
        "┘".cyan()
    );
    println!();
}

/// Print info about a single file
#[allow(dead_code)]
fn print_file_info(path: &std::path::Path, detailed: bool) -> anyhow::Result<()> {
    let metadata = std::fs::metadata(path)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let icon = get_file_icon(&name, false, false);

    if detailed {
        #[cfg(unix)]
        let perms = {
            use std::os::unix::fs::PermissionsExt;
            format_permissions(metadata.permissions().mode())
        };
        #[cfg(not(unix))]
        let perms = if metadata.permissions().readonly() { "r--" } else { "rw-" }.to_string();

        let size = format_size(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Local> = t.into();
                datetime.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_else(|| "???".to_string());

        println!();
        println!("  {} {}", icon, name.bright_white().bold());
        println!("  {} Size: {}", "│".dimmed(), size.bright_yellow());
        println!("  {} Perms: {}", "│".dimmed(), perms);
        println!("  {} Modified: {}", "│".dimmed(), modified);
        println!();
    } else {
        println!("  {} {}", icon, name);
    }

    Ok(())
}

/// Get an icon for a file based on name and type
#[allow(dead_code)]
fn get_file_icon(name: &str, is_dir: bool, is_symlink: bool) -> &'static str {
    if is_symlink {
        return "🔗";
    }
    if is_dir {
        // Special directories
        return match name {
            ".git" => "",
            "node_modules" => "📦",
            "target" => "🎯",
            "src" => "📁",
            "tests" | "test" => "🧪",
            "docs" | "doc" => "📚",
            "build" | "dist" => "📦",
            ".github" => "",
            _ => "📂",
        };
    }

    // File icons by extension
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        "rs" => "🦀",
        "py" => "🐍",
        "js" | "jsx" => "",
        "ts" | "tsx" => "󰛦",
        "go" => "🐹",
        "rb" => "💎",
        "java" => "☕",
        "c" | "h" => "🇨",
        "cpp" | "hpp" | "cc" => "⧺",
        "md" => "📝",
        "txt" => "📄",
        "json" => "{}",
        "toml" | "yaml" | "yml" => "⚙️",
        "html" => "🌐",
        "css" | "scss" | "sass" => "🎨",
        "sh" | "bash" | "zsh" => "🐚",
        "sql" => "🗃️",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" => "🖼️",
        "mp3" | "wav" | "ogg" | "flac" => "🎵",
        "mp4" | "avi" | "mkv" | "mov" => "🎬",
        "pdf" => "📕",
        "zip" | "tar" | "gz" | "xz" | "7z" => "📦",
        "lock" => "🔒",
        "log" => "📋",
        "env" => "🔐",
        _ => "📄",
    }
}

/// Colorize a filename based on its type
#[allow(dead_code)]
#[allow(unused_variables)]
fn colorize_filename(name: &str, is_dir: bool, is_symlink: bool, path: &std::path::Path) -> String {
    if is_symlink {
        return format!("{}", name.bright_cyan().italic());
    }
    if is_dir {
        return format!("{}", name.bright_blue().bold());
    }

    // Check if executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata() {
            if meta.permissions().mode() & 0o111 != 0 {
                return format!("{}", name.bright_green().bold());
            }
        }
    }

    // On Windows, check for executable extensions
    #[cfg(windows)]
    {
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        if matches!(ext.as_str(), "exe" | "cmd" | "bat" | "ps1" | "com") {
            return format!("{}", name.bright_green().bold());
        }
    }

    // Special file colors by name
    match name {
        "Cargo.toml" | "Cargo.lock" => name.bright_yellow().to_string(),
        "package.json" | "package-lock.json" => name.bright_green().to_string(),
        "README.md" | "README" => name.bright_cyan().to_string(),
        ".gitignore" | ".env" | ".env.local" => name.dimmed().to_string(),
        _ => name.to_string(),
    }
}

/// Format file size in human-readable form
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Format Unix permissions
#[cfg(unix)]
fn format_permissions(mode: u32) -> String {
    let mut result = String::with_capacity(9);

    // Owner
    result.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    result.push(if mode & 0o200 != 0 { 'w' } else { '-' });
    result.push(if mode & 0o100 != 0 { 'x' } else { '-' });

    // Group
    result.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    result.push(if mode & 0o020 != 0 { 'w' } else { '-' });
    result.push(if mode & 0o010 != 0 { 'x' } else { '-' });

    // Other
    result.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    result.push(if mode & 0o002 != 0 { 'w' } else { '-' });
    result.push(if mode & 0o001 != 0 { 'x' } else { '-' });

    result
}

/// Slash command definition
struct SlashCommand {
    name: &'static str,
    aliases: &'static [&'static str],
    description: &'static str,
    handler: fn(&str, &mut ReplState) -> anyhow::Result<()>,
}

/// REPL state
#[allow(dead_code)]
pub struct ReplState {
    pub mode: ChatMode,
    pub model: Option<String>,
    pub provider_name: Option<String>,
    pub history: Vec<(String, String)>,
    pub messages: Vec<Message>,
    pub working_dir: PathBuf,
    pub session_id: Option<String>,
    pub provider_manager: Arc<ProviderManager>,
    pub context_files: Vec<PathBuf>,
    /// Track consecutive Ctrl-C presses for exit
    pub ctrl_c_count: u8,
    /// Session logger for text logs
    pub session_logger: SessionLogger,
    /// Whether this is the first message (for session title)
    pub is_first_message: bool,
    /// MCP manager for tool servers
    pub mcp_manager: Arc<McpManager>,
    /// Cached MCP tools (refreshed on connect/disconnect)
    pub mcp_tools: Vec<(String, McpTool)>,
}

impl ReplState {
    pub fn new(cli: &Cli, provider_manager: Arc<ProviderManager>) -> Self {
        // Default logging settings - can be overridden by config
        let logging_enabled = true;
        let max_log_size = 512 * 1024 * 1024; // 512 MB

        Self {
            mode: cli.mode,
            model: cli.model.clone(),
            provider_name: cli.provider.clone(),
            history: Vec::new(),
            messages: Vec::new(),
            working_dir: cli
                .directory
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
            session_id: None,
            provider_manager,
            context_files: Vec::new(),
            ctrl_c_count: 0,
            session_logger: SessionLogger::new(logging_enabled, max_log_size),
            is_first_message: true,
            mcp_manager: Arc::new(McpManager::new()),
            mcp_tools: Vec::new(),
        }
    }

    /// Initialize MCP servers with default configuration
    pub async fn init_mcp(&mut self) -> anyhow::Result<()> {
        // Load any existing config
        if let Err(e) = self.mcp_manager.load_config().await {
            debug!("No MCP config found: {}", e);
        }

        // Check if any servers are configured
        let configured = self.mcp_manager.list_configured().await;

        // If no servers configured, auto-add puppeteer for browser automation
        if configured.is_empty() {
            debug!("No MCP servers configured, adding puppeteer by default");
            let preset = mcp_presets::puppeteer();
            self.mcp_manager.add_server_config("puppeteer", preset).await;
        }

        // Auto-connect to configured servers
        if let Err(e) = self.mcp_manager.auto_connect().await {
            // Only warn if there were configured servers that failed
            let configured = self.mcp_manager.list_configured().await;
            if !configured.is_empty() {
                warn!("Failed to auto-connect MCP servers: {}", e);
            }
        }

        // Refresh tool cache
        self.refresh_mcp_tools().await;

        Ok(())
    }

    /// Refresh the cached list of MCP tools
    pub async fn refresh_mcp_tools(&mut self) {
        self.mcp_tools = self.mcp_manager.list_tools().await;
        if !self.mcp_tools.is_empty() {
            info!("Loaded {} MCP tools", self.mcp_tools.len());
        }
    }

    /// Get tool descriptions for system prompt
    pub fn get_mcp_tools_prompt(&self) -> String {
        if self.mcp_tools.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n\nAVAILABLE TOOLS:\n");
        prompt.push_str("You can call these tools using a ```tool code block:\n\n");
        prompt.push_str("```tool\n");
        prompt.push_str("tool_name: <tool_id>\n");
        prompt.push_str("arguments:\n");
        prompt.push_str("  param1: value1\n");
        prompt.push_str("  param2: value2\n");
        prompt.push_str("```\n\n");
        prompt.push_str("Available tools:\n");

        for (tool_id, tool) in &self.mcp_tools {
            prompt.push_str(&format!("\n- **{}**: {}\n", tool_id, tool.description));

            // Add parameter info if available
            if let Some(props) = &tool.input_schema.properties {
                prompt.push_str("  Parameters:\n");
                for (name, prop_schema) in props {
                    let type_str = &prop_schema.prop_type;
                    let desc = prop_schema.description.as_deref().unwrap_or("");
                    let required = tool.input_schema.required.as_ref()
                        .map(|r| r.contains(name))
                        .unwrap_or(false);
                    let req_str = if required { " (required)" } else { "" };
                    prompt.push_str(&format!("    - {}: {}{} - {}\n", name, type_str, req_str, desc));
                }
            }
        }

        prompt
    }

    /// Get the system prompt based on current mode
    #[allow(dead_code)]
    fn system_prompt(&self) -> String {
        let context = if self.context_files.is_empty() {
            String::new()
        } else {
            let files: Vec<_> = self.context_files.iter()
                .filter_map(|p| p.to_str())
                .collect();
            format!("\n\nFiles in context: {}", files.join(", "))
        };

        let base_prompt = match self.mode {
            ChatMode::Code => {
                "You are Ganesha, the Remover of Obstacles. You help users overcome coding challenges. \
                Be concise and provide working code examples. When editing files, show clear diffs."
            }
            ChatMode::Ask => {
                "You are Ganesha, the Remover of Obstacles. Answer questions clearly and concisely. \
                Do not make changes to files - only explain and discuss."
            }
            ChatMode::Architect => {
                "You are Ganesha, the Remover of Obstacles. Help users plan and design software systems. \
                Think through problems systematically, consider trade-offs, and provide clear recommendations."
            }
            ChatMode::Help => {
                "You are Ganesha's help system. Explain Ganesha's features, commands, and capabilities. \
                Available commands: /help, /mode, /model, /clear, /undo, /diff, /git, /commit, /add, /drop, /ls, /mcp, /session, /provider, /exit"
            }
        };

        format!("{}{}", base_prompt, context)
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
        name: "provider",
        aliases: &["p"],
        description: "Add or manage AI providers",
        handler: cmd_provider,
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
    // Initialize provider manager
    let provider_manager = Arc::new(ProviderManager::new());

    // Show brief startup spinner
    print!("{} Starting Ganesha...", "🐘".bright_green());

    // Load saved provider configs FIRST (before auto-discovery)
    // This ensures API keys are set in env vars before discovery
    let saved_config = ProvidersConfig::load();
    if saved_config.has_providers() {
        for provider in saved_config.enabled_providers() {
            match provider.provider_type {
                ProviderType::Local => {
                    // Local providers are handled after cloud providers
                }
                _ => {
                    // Set env var for cloud providers so auto_discover finds them
                    if let Some(ref api_key) = provider.api_key {
                        let env_var = match provider.provider_type {
                            ProviderType::Anthropic => "ANTHROPIC_API_KEY",
                            ProviderType::OpenAI => "OPENAI_API_KEY",
                            ProviderType::Gemini => "GEMINI_API_KEY",
                            ProviderType::OpenRouter => "OPENROUTER_API_KEY",
                            ProviderType::Local => continue,
                        };
                        std::env::set_var(env_var, api_key);
                        info!("Set {} from saved config", env_var);
                    }
                }
            }
        }
    }

    // Auto-discover available providers (will find cloud providers via env vars we just set)
    if let Err(e) = provider_manager.auto_discover().await {
        warn!("Provider discovery failed: {}", e);
    }

    // Now register local providers from saved config
    if saved_config.has_providers() {
        for provider in saved_config.enabled_providers() {
            if provider.provider_type == ProviderType::Local {
                if let Some(ref base_url) = provider.base_url {
                    info!("Loading saved local provider: {} at {}", provider.name, base_url);
                    let url = if base_url.ends_with("/v1") {
                        base_url.clone()
                    } else if base_url.ends_with('/') {
                        format!("{}v1", base_url)
                    } else {
                        format!("{}/v1", base_url)
                    };
                    let local = LocalProvider::new(LocalProviderType::OpenAiCompatible)
                        .with_base_url(url);
                    provider_manager.register(local, ProviderPriority::Primary).await;
                }
            }
        }
    }

    // Check if any providers are available, run setup wizard if not
    let providers = provider_manager.list_providers().await;
    if providers.is_empty() {
        // Run the interactive setup wizard
        match setup::run_setup_wizard() {
            Ok(Some(config)) => {
                // Set up the provider with the new config
                match config.provider_type {
                    ProviderType::Local => {
                        if let Some(ref base_url) = config.base_url {
                            // Ensure /v1 suffix for OpenAI-compatible servers
                            let url = if base_url.ends_with("/v1") {
                                base_url.clone()
                            } else if base_url.ends_with('/') {
                                format!("{}v1", base_url)
                            } else {
                                format!("{}/v1", base_url)
                            };
                            let local = LocalProvider::new(LocalProviderType::OpenAiCompatible)
                                .with_base_url(url);
                            provider_manager.register(local, ProviderPriority::Primary).await;
                        }
                    }
                    _ => {
                        if let Some(ref api_key) = config.api_key {
                            let env_var = match config.provider_type {
                                ProviderType::Anthropic => "ANTHROPIC_API_KEY",
                                ProviderType::OpenAI => "OPENAI_API_KEY",
                                ProviderType::Gemini => "GEMINI_API_KEY",
                                ProviderType::OpenRouter => "OPENROUTER_API_KEY",
                                ProviderType::Local => "",
                            };
                            if !env_var.is_empty() {
                                std::env::set_var(env_var, api_key);
                            }
                        }
                        // Re-discover providers
                        if let Err(e) = provider_manager.auto_discover().await {
                            warn!("Provider setup failed: {}", e);
                        }
                    }
                }
                println!();
            }
            Ok(None) => {
                println!("\n{}", "Running without LLM provider. AI features unavailable.".dimmed());
                println!("Run {} to set up providers later.\n", "ganesha config".bright_cyan());
            }
            Err(e) => {
                warn!("Setup wizard failed: {}", e);
            }
        }
    }

    // Get provider status
    let providers = provider_manager.list_providers().await;
    let provider_names: Vec<_> = providers.iter().map(|p| p.name.as_str()).collect();

    let mut state = ReplState::new(cli, provider_manager);

    // Start session logging immediately to capture any errors
    if let Err(e) = state.session_logger.start_session(Some("startup")) {
        warn!("Failed to start session logging: {}", e);
    } else {
        // Log startup info
        let _ = state.session_logger.write_line(&format!(
            "=== Ganesha Startup at {} ===\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        let _ = state.session_logger.write_line(&format!(
            "Providers: {}\n",
            provider_names.join(", ")
        ));
    }

    // Initialize MCP servers silently
    let _ = state.init_mcp().await;
    let mcp_count = state.mcp_manager.list_connected().await.len();
    let tool_count = state.mcp_tools.len();

    // Show compact startup summary
    println!(" {}", "Ready!".bright_green());
    if !provider_names.is_empty() || mcp_count > 0 || tool_count > 0 {
        let parts: Vec<String> = vec![
            if !provider_names.is_empty() {
                format!("{} providers", provider_names.len())
            } else { String::new() },
            if mcp_count > 0 {
                format!("{} MCP servers", mcp_count)
            } else { String::new() },
            if tool_count > 0 {
                format!("{} tools", tool_count)
            } else { String::new() },
        ].into_iter().filter(|s| !s.is_empty()).collect();

        if !parts.is_empty() {
            println!("  {} {}", "→".dimmed(), parts.join(", ").dimmed());
        }
    }

    println!();

    // Check if stdin is a terminal (interactive) or piped
    let stdin_is_tty = std::io::stdin().is_terminal();

    // Handle piped input (non-interactive mode)
    if !stdin_is_tty {
        debug!("Detected piped input - running in non-interactive mode");

        // Read all lines from stdin
        let stdin = std::io::stdin();
        let lines: Vec<String> = stdin.lock().lines()
            .filter_map(|l| l.ok())
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            // No input provided - show brief usage
            println!("Usage: echo 'your request' | ganesha");
            return Ok(());
        }

        // Process each line as a separate request
        for line in lines {
            // Check for slash commands
            if line.starts_with('/') {
                match handle_slash_command(&line, &mut state) {
                    Ok(should_exit) => {
                        if should_exit {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red().bold(), e);
                    }
                }
                continue;
            }

            // Send to agentic chat
            match agentic_chat(&line, &mut state).await {
                Ok(response) => {
                    // Display final response in Ganesha box
                    print_ganesha_box(&response);
                    state.history.push((line.clone(), response));
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    eprintln!("{} {}", "Error:".red().bold(), error_msg);
                    // Log error to session
                    let _ = state.session_logger.write_line(&format!(
                        "[{}] ERROR:\n{}\n\n",
                        chrono::Local::now().format("%H:%M:%S"),
                        error_msg
                    ));
                }
            }
        }

        return Ok(());
    }

    // Show session log info
    println!(
        "{} Session logs: {}",
        "📝".dimmed(),
        state.session_logger.sessions_dir.display().to_string().dimmed()
    );

    // Interactive mode - print welcome message
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
                // Reset Ctrl-C counter on any input
                state.ctrl_c_count = 0;

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

                // Send everything to agentic chat - let AI decide what to do
                match agentic_chat(line, &mut state).await {
                    Ok(response) => {
                        // Display final response in Ganesha box
                        print_ganesha_box(&response);
                        state.history.push((line.to_string(), response.clone()));

                        // Check if response contains multiple choice options
                        if let Some(options) = detect_options(&response) {
                            if let Some(selection) = prompt_multiple_choice(&options) {
                                // Send the selection back to the AI
                                match agentic_chat(&selection, &mut state).await {
                                    Ok(follow_up) => {
                                        print_ganesha_box(&follow_up);
                                        state.history.push((selection, follow_up));
                                    }
                                    Err(e) => {
                                        let error_msg = format!("{}", e);
                                        eprintln!("{} {}", "Error:".red().bold(), error_msg);
                                        let _ = state.session_logger.write_line(&format!(
                                            "[{}] ERROR:\n{}\n\n",
                                            chrono::Local::now().format("%H:%M:%S"),
                                            error_msg
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("{}", e);
                        eprintln!("{} {}", "Error:".red().bold(), error_msg);
                        // Log error to session
                        let _ = state.session_logger.write_line(&format!(
                            "[{}] ERROR:\n{}\n\n",
                            chrono::Local::now().format("%H:%M:%S"),
                            error_msg
                        ));
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                state.ctrl_c_count += 1;
                if state.ctrl_c_count >= 2 {
                    println!("\nGoodbye! 🐘");
                    break;
                }
                println!("^C (press again to exit)");
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

    // End session logging
    if let Err(e) = state.session_logger.end_session() {
        warn!("Failed to end session log: {}", e);
    }

    // Save history
    if let Some(parent) = history_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = rl.save_history(&history_path);

    Ok(())
}

/// Print welcome message with compact header
fn print_welcome(state: &ReplState) {
    let version = env!("CARGO_PKG_VERSION");

    println!();
    println!(
        "{} v{} - {}",
        "🐘 Ganesha".bright_magenta().bold(),
        version.dimmed(),
        "The Remover of Obstacles".bright_cyan()
    );
    println!();
    println!(
        "Mode: {}  •  Type {} for help",
        format!("{:?}", state.mode).bright_yellow(),
        "/help".bright_green()
    );
    println!();
}

/// Get the prompt string based on current mode and directory
fn get_prompt(state: &ReplState) -> String {
    // Get a short version of the current directory
    let dir_display = if let Some(home) = dirs::home_dir() {
        if state.working_dir.starts_with(&home) {
            let relative = state.working_dir.strip_prefix(&home).unwrap_or(&state.working_dir);
            format!("~{}{}", std::path::MAIN_SEPARATOR, relative.display())
        } else {
            state.working_dir.display().to_string()
        }
    } else {
        state.working_dir.display().to_string()
    };

    // Truncate if too long
    let sep = std::path::MAIN_SEPARATOR;
    let dir_short = if dir_display.len() > 30 {
        let parts: Vec<&str> = dir_display.split(|c| c == '/' || c == '\\').collect();
        if parts.len() > 3 {
            format!("...{}{}", sep, parts[parts.len() - 2..].join(&sep.to_string()))
        } else {
            dir_display
        }
    } else {
        dir_display
    };

    // Use plain prompt - ANSI colors confuse readline's cursor positioning
    format!("{} [Ganesha]> ", dir_short)
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
#[allow(dead_code)]
async fn send_message(message: &str, state: &mut ReplState) -> anyhow::Result<String> {
    debug!("Sending message: {}", message);

    // Check if we have a provider
    if !state.provider_manager.has_available_provider().await {
        return Err(anyhow::anyhow!(
            "No LLM providers available. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or start a local server."
        ));
    }

    // Add user message to conversation
    state.messages.push(Message::user(message));

    // Build messages with system prompt
    let mut messages = vec![Message::system(state.system_prompt())];
    messages.extend(state.messages.clone());

    // Set up generation options
    let options = GenerateOptions {
        model: state.model.clone(),
        temperature: Some(0.7),
        max_tokens: Some(4096),
        ..Default::default()
    };

    // Call the provider
    let response = state.provider_manager.chat(&messages, &options).await?;

    // Add assistant response to conversation
    state.messages.push(Message::assistant(&response.content));

    // Show token usage if available
    if let Some(usage) = &response.usage {
        debug!(
            "Tokens: {} prompt + {} completion = {} total",
            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
        );
    }

    Ok(response.content)
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
    let input = args.trim();

    // Get available models
    let models = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(state.provider_manager.list_all_models())
    });

    let models = match models {
        Ok(m) => m,
        Err(e) => {
            println!("{} {}", "Error listing models:".red(), e);
            return Ok(());
        }
    };

    // Check if input is a number (model selection)
    if let Ok(num) = input.parse::<usize>() {
        if num > 0 && num <= models.len() {
            let selected = &models[num - 1];
            state.model = Some(selected.id.clone());
            println!("Switched to model: {}", selected.id.bright_cyan());
            return Ok(());
        } else if !input.is_empty() {
            println!("{} Invalid selection. Choose 1-{}", "Error:".red(), models.len());
            return Ok(());
        }
    }

    // If input matches a model name directly, use it
    if !input.is_empty() && input != "list" && input != "ls" {
        // Check if it's a valid model name
        if models.iter().any(|m| m.id == input) {
            state.model = Some(input.to_string());
            println!("Switched to model: {}", input.bright_cyan());
            return Ok(());
        }
        // Allow setting any model name (might be on a provider we can't list)
        state.model = Some(input.to_string());
        println!("Switched to model: {}", input.bright_cyan());
        return Ok(());
    }

    // Show current model
    if let Some(ref m) = state.model {
        println!("Current model: {}", m.bright_cyan());
    } else {
        println!("Using default model");
    }

    // List available models with numbers
    println!();
    println!("{}", "Available models:".dimmed());

    if models.is_empty() {
        println!("  {}", "No models available".dimmed());
    } else {
        for (i, model) in models.iter().enumerate() {
            let tier_icon = match model.tier {
                ganesha_providers::ModelTier::Exceptional => "🟢",
                ganesha_providers::ModelTier::Capable => "🟡",
                ganesha_providers::ModelTier::Limited => "🟠",
                ganesha_providers::ModelTier::Unsafe => "🔴",
                ganesha_providers::ModelTier::Unknown => "⚪",
            };
            println!(
                "  {:>2}) {} {} {} ({})",
                (i + 1).to_string().bright_yellow(),
                tier_icon,
                model.id.bright_white(),
                format!("[{}]", model.provider).dimmed(),
                if model.supports_vision { "vision" } else { "text" }
            );
        }
    }

    println!();
    println!("Use {} to switch models", "/model <number>".bright_green());
    Ok(())
}

fn cmd_clear(_args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    state.history.clear();
    state.messages.clear();
    println!("Conversation cleared");
    Ok(())
}

fn cmd_undo(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    // Check if we're in a git repo
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&state.working_dir)
        .output();

    match git_check {
        Ok(output) if output.status.success() => {}
        _ => {
            println!("{} Not in a git repository", "Error:".red());
            return Ok(());
        }
    }

    let target = args.trim();

    if target.is_empty() || target == "last" {
        // Undo last changed file(s) - restore from HEAD
        let status = std::process::Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(&state.working_dir)
            .output()?;

        let status_str = String::from_utf8_lossy(&status.stdout).to_string();
        let files: Vec<&str> = status_str
            .lines()
            .filter(|l| !l.is_empty())
            .collect();

        if files.is_empty() {
            // Try staged files
            let staged = std::process::Command::new("git")
                .args(["diff", "--cached", "--name-only"])
                .current_dir(&state.working_dir)
                .output()?;

            let staged_files: Vec<String> = String::from_utf8_lossy(&staged.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();

            if staged_files.is_empty() {
                println!("No changes to undo");
                return Ok(());
            }

            // Unstage files
            for file in &staged_files {
                let _ = std::process::Command::new("git")
                    .args(["restore", "--staged", file])
                    .current_dir(&state.working_dir)
                    .output();
                println!("  {} Unstaged: {}", "↩".bright_yellow(), file);
            }
            return Ok(());
        }

        // Ask confirmation
        println!("
{} files with changes:", files.len());
        for f in &files {
            println!("  {} {}", "•".dimmed(), f);
        }
        println!();
        print!("{} Restore all from HEAD? [y/N] ", "⚠".yellow());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();

        if input.trim().to_lowercase() == "y" {
            let restore = std::process::Command::new("git")
                .args(["checkout", "--", "."])
                .current_dir(&state.working_dir)
                .output()?;

            if restore.status.success() {
                println!("{} Restored {} file(s) from HEAD", "✓".green(), files.len());
            } else {
                println!("{} {}", "Error:".red(), String::from_utf8_lossy(&restore.stderr));
            }
        } else {
            println!("Cancelled");
        }
    } else {
        // Undo specific file
        let restore = std::process::Command::new("git")
            .args(["checkout", "--", target])
            .current_dir(&state.working_dir)
            .output()?;

        if restore.status.success() {
            println!("{} Restored: {}", "✓".green(), target);
        } else {
            println!("{} {}", "Error:".red(), String::from_utf8_lossy(&restore.stderr));
        }
    }

    Ok(())
}

fn cmd_diff(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    let target = args.trim();

    // Check if in git repo
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&state.working_dir)
        .output();

    match git_check {
        Ok(output) if output.status.success() => {}
        _ => {
            println!("{} Not in a git repository", "Error:".red());
            return Ok(());
        }
    }

    // Build git diff command
    let mut git_args = vec!["diff", "--stat"];

    if target == "staged" || target == "cached" {
        git_args.push("--cached");
    } else if target == "all" || target == "full" {
        // Show full diff
        git_args = vec!["diff"];
    } else if !target.is_empty() {
        // Diff specific file
        git_args = vec!["diff", target];
    }

    let output = std::process::Command::new("git")
        .args(&git_args)
        .current_dir(&state.working_dir)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.is_empty() {
        // Try staged
        let staged = std::process::Command::new("git")
            .args(["diff", "--cached", "--stat"])
            .current_dir(&state.working_dir)
            .output()?;

        let staged_out = String::from_utf8_lossy(&staged.stdout);
        if staged_out.is_empty() {
            println!("No changes detected (working tree clean)");
        } else {
            println!("
{}", "Staged changes:".bright_green().bold());
            print!("{}", staged_out);
        }
    } else {
        if target == "all" || target == "full" {
            // Colorize full diff output
            for line in stdout.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    println!("{}", line.green());
                } else if line.starts_with('-') && !line.starts_with("---") {
                    println!("{}", line.red());
                } else if line.starts_with("@@") {
                    println!("{}", line.cyan());
                } else if line.starts_with("diff") || line.starts_with("index") {
                    println!("{}", line.dimmed());
                } else {
                    println!("{}", line);
                }
            }
        } else {
            println!("
{}", "Working tree changes:".bright_yellow().bold());
            print!("{}", stdout);
        }

        // Also check for staged
        if target.is_empty() {
            let staged = std::process::Command::new("git")
                .args(["diff", "--cached", "--stat"])
                .current_dir(&state.working_dir)
                .output()?;
            let staged_out = String::from_utf8_lossy(&staged.stdout);
            if !staged_out.is_empty() {
                println!("
{}", "Staged changes:".bright_green().bold());
                print!("{}", staged_out);
            }
        }
    }

    println!();
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

fn cmd_commit(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    // Check if in git repo
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&state.working_dir)
        .output();

    match git_check {
        Ok(output) if output.status.success() => {}
        _ => {
            println!("{} Not in a git repository", "Error:".red());
            return Ok(());
        }
    }

    let user_msg = args.trim();

    // Get diff for commit message generation
    // First check staged, then unstaged
    let staged_diff = std::process::Command::new("git")
        .args(["diff", "--cached", "--stat"])
        .current_dir(&state.working_dir)
        .output()?;
    let staged_out = String::from_utf8_lossy(&staged_diff.stdout).to_string();

    let has_staged = !staged_out.trim().is_empty();

    // If nothing staged, stage everything
    if !has_staged {
        let unstaged = std::process::Command::new("git")
            .args(["diff", "--stat"])
            .current_dir(&state.working_dir)
            .output()?;
        let unstaged_out = String::from_utf8_lossy(&unstaged.stdout).to_string();

        if unstaged_out.trim().is_empty() {
            // Check for untracked files
            let untracked = std::process::Command::new("git")
                .args(["ls-files", "--others", "--exclude-standard"])
                .current_dir(&state.working_dir)
                .output()?;
            let untracked_out = String::from_utf8_lossy(&untracked.stdout).to_string();

            if untracked_out.trim().is_empty() {
                println!("Nothing to commit (working tree clean)");
                return Ok(());
            }
        }

        // Stage all changes
        println!("{} Staging all changes...", "→".bright_blue());
        let _ = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&state.working_dir)
            .output()?;
    }

    // Get the final staged diff for message generation
    let diff_output = std::process::Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(&state.working_dir)
        .output()?;
    let diff = String::from_utf8_lossy(&diff_output.stdout);

    // Truncate diff if too long
    let diff_truncated = if diff.len() > 4000 {
        format!("{}
... (truncated, {} total bytes)", &diff[..4000], diff.len())
    } else {
        diff.to_string()
    };

    // Show what will be committed
    let stat = std::process::Command::new("git")
        .args(["diff", "--cached", "--stat"])
        .current_dir(&state.working_dir)
        .output()?;
    println!("
{}", "Changes to commit:".bright_green().bold());
    print!("{}", String::from_utf8_lossy(&stat.stdout));
    println!();

    // If user provided a message, use it directly
    if !user_msg.is_empty() {
        let commit = std::process::Command::new("git")
            .args(["commit", "-m", user_msg])
            .current_dir(&state.working_dir)
            .output()?;

        if commit.status.success() {
            println!("{} Committed: {}", "✓".green(), user_msg);
            let hash = std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .current_dir(&state.working_dir)
                .output()?;
            println!("  Hash: {}", String::from_utf8_lossy(&hash.stdout).trim().bright_cyan());
        } else {
            println!("{} {}", "Error:".red(), String::from_utf8_lossy(&commit.stderr));
        }
        return Ok(());
    }

    // Try to generate commit message with AI
    let has_provider = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(state.provider_manager.has_available_provider())
    });

    if has_provider {
        println!("{} Generating commit message...", "🤖".bright_cyan());

        let prompt = format!(
            "Generate a concise git commit message for this diff. \
             Use conventional commit format (feat/fix/refactor/docs/test/chore). \
             First line under 72 chars. Add bullet points for details if needed. \
             Only output the commit message, nothing else.\n\nDiff:\n```\n{}\n```",
            diff_truncated
        );

        let messages = vec![
            ganesha_providers::Message::system("You are a helpful git commit message generator. Output ONLY the commit message text, no markdown, no quotes, no explanation."),
            ganesha_providers::Message::user(&prompt),
        ];

        let options = ganesha_providers::GenerateOptions {
            model: state.model.clone(),
            temperature: Some(0.3),
            max_tokens: Some(256),
            ..Default::default()
        };

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(state.provider_manager.chat(&messages, &options))
        });

        match response {
            Ok(resp) => {
                let msg = resp.content.trim().trim_matches('\'').trim_matches('"').to_string();
                println!("
{}", "Suggested commit message:".bright_yellow());
                println!("{}", "─".repeat(50).dimmed());
                println!("{}", msg);
                println!("{}", "─".repeat(50).dimmed());
                println!();

                print!("{} ", "Accept? [Y/n/e(dit)] ".bright_white());
                std::io::Write::flush(&mut std::io::stdout()).ok();

                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                let choice = input.trim().to_lowercase();

                if choice.is_empty() || choice == "y" || choice == "yes" {
                    let commit = std::process::Command::new("git")
                        .args(["commit", "-m", &msg])
                        .current_dir(&state.working_dir)
                        .output()?;

                    if commit.status.success() {
                        println!("{} Committed!", "✓".green());
                        let hash = std::process::Command::new("git")
                            .args(["rev-parse", "--short", "HEAD"])
                            .current_dir(&state.working_dir)
                            .output()?;
                        println!("  Hash: {}", String::from_utf8_lossy(&hash.stdout).trim().bright_cyan());
                    } else {
                        println!("{} {}", "Error:".red(), String::from_utf8_lossy(&commit.stderr));
                    }
                } else if choice == "e" || choice == "edit" {
                    print!("{} ", "Enter commit message:".bright_white());
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                    let mut custom_msg = String::new();
                    std::io::stdin().read_line(&mut custom_msg).ok();
                    let custom_msg = custom_msg.trim();

                    if !custom_msg.is_empty() {
                        let commit = std::process::Command::new("git")
                            .args(["commit", "-m", custom_msg])
                            .current_dir(&state.working_dir)
                            .output()?;

                        if commit.status.success() {
                            println!("{} Committed: {}", "✓".green(), custom_msg);
                        } else {
                            println!("{} {}", "Error:".red(), String::from_utf8_lossy(&commit.stderr));
                        }
                    } else {
                        println!("Cancelled (no message)");
                    }
                } else {
                    // Unstage if user cancels
                    println!("Cancelled");
                }
            }
            Err(e) => {
                println!("{} AI generation failed: {}", "⚠".yellow(), e);
                print!("{} ", "Enter commit message manually:".bright_white());
                std::io::Write::flush(&mut std::io::stdout()).ok();
                let mut manual_msg = String::new();
                std::io::stdin().read_line(&mut manual_msg).ok();
                let manual_msg = manual_msg.trim();

                if !manual_msg.is_empty() {
                    let commit = std::process::Command::new("git")
                        .args(["commit", "-m", manual_msg])
                        .current_dir(&state.working_dir)
                        .output()?;

                    if commit.status.success() {
                        println!("{} Committed: {}", "✓".green(), manual_msg);
                    } else {
                        println!("{} {}", "Error:".red(), String::from_utf8_lossy(&commit.stderr));
                    }
                } else {
                    println!("Cancelled");
                }
            }
        }
    } else {
        // No AI provider, ask for message directly
        print!("{} ", "Commit message:".bright_white());
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let mut msg = String::new();
        std::io::stdin().read_line(&mut msg).ok();
        let msg = msg.trim();

        if !msg.is_empty() {
            let commit = std::process::Command::new("git")
                .args(["commit", "-m", msg])
                .current_dir(&state.working_dir)
                .output()?;

            if commit.status.success() {
                println!("{} Committed: {}", "✓".green(), msg);
            } else {
                println!("{} {}", "Error:".red(), String::from_utf8_lossy(&commit.stderr));
            }
        } else {
            println!("Cancelled");
        }
    }

    Ok(())
}

fn cmd_add(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    let files: Vec<&str> = args.split_whitespace().collect();
    if files.is_empty() {
        println!("Usage: /add <file1> [file2] ...");
        return Ok(());
    }

    let mut added = 0;
    for file in files {
        let path = if std::path::Path::new(file).is_absolute() {
            PathBuf::from(file)
        } else {
            state.working_dir.join(file)
        };

        if path.exists() {
            if !state.context_files.contains(&path) {
                state.context_files.push(path.clone());
                println!("  {} {}", "+".green(), path.display());
                added += 1;
            } else {
                println!("  {} {} (already in context)", "~".yellow(), path.display());
            }
        } else {
            println!("  {} {} (not found)", "✗".red(), file);
        }
    }

    if added > 0 {
        println!("Added {} file(s) to context", added);
    }
    Ok(())
}

fn cmd_drop(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    let files: Vec<&str> = args.split_whitespace().collect();
    if files.is_empty() {
        println!("Usage: /drop <file1> [file2] ...");
        return Ok(());
    }

    let mut removed = 0;
    for file in files {
        let path = if std::path::Path::new(file).is_absolute() {
            PathBuf::from(file)
        } else {
            state.working_dir.join(file)
        };

        if let Some(pos) = state.context_files.iter().position(|p| p == &path) {
            state.context_files.remove(pos);
            println!("  {} {}", "-".red(), path.display());
            removed += 1;
        } else {
            // Also try to match by filename only
            if let Some(pos) = state.context_files.iter().position(|p| {
                p.file_name().map(|n| n.to_str()) == Some(Some(file))
            }) {
                let removed_path = state.context_files.remove(pos);
                println!("  {} {}", "-".red(), removed_path.display());
                removed += 1;
            } else {
                println!("  {} {} (not in context)", "?".yellow(), file);
            }
        }
    }

    if removed > 0 {
        println!("Removed {} file(s) from context", removed);
    }
    Ok(())
}

fn cmd_ls(_args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    if state.context_files.is_empty() {
        println!("No files in context");
        println!("Use {} to add files", "/add <file>".bright_green());
    } else {
        println!("{} file(s) in context:", state.context_files.len());
        for path in &state.context_files {
            println!("  {} {}", "•".dimmed(), path.display());
        }
    }
    Ok(())
}

fn cmd_mcp(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().map(|s| *s).unwrap_or("");

    // Helper to run async ops from sync context within async runtime
    fn run_async<F: std::future::Future>(f: F) -> F::Output {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(f)
        })
    }

    match action {
        "list" | "ls" | "" => {
            // List connected servers and their tools
            let connected = run_async(state.mcp_manager.list_connected());

            if connected.is_empty() {
                println!("\n{}\n", "No MCP servers connected".yellow());
                println!("  Configured servers will auto-connect on startup.");
                println!("  Use {} to add servers.\n", "/mcp add <preset>".bright_green());
            } else {
                println!("\n{}\n", "Connected MCP Servers".bright_cyan().bold());
                for server_id in &connected {
                    println!("  {} {}", "•".green(), server_id.bright_white());
                }
            }

            if !state.mcp_tools.is_empty() {
                println!("\n{}\n", "Available Tools".bright_cyan().bold());
                for (tool_id, tool) in &state.mcp_tools {
                    let desc = &tool.description;
                    // Truncate description if too long
                    let desc_short = if desc.len() > 60 {
                        format!("{}...", &desc[..57])
                    } else {
                        desc.clone()
                    };
                    println!("  {} {}", tool_id.bright_green(), desc_short.dimmed());
                }
            }
            println!();
        }
        "tools" => {
            // List all tools in detail
            if state.mcp_tools.is_empty() {
                println!("No tools available. Connect to MCP servers first.");
            } else {
                println!("\n{}\n", "Available MCP Tools".bright_cyan().bold());
                for (tool_id, tool) in &state.mcp_tools {
                    println!("  {}", tool_id.bright_green().bold());
                    println!("    {}", tool.description.dimmed());

                    if let Some(props) = &tool.input_schema.properties {
                        println!("    Parameters:");
                        for (name, prop) in props {
                            let type_str = &prop.prop_type;
                            let required = tool.input_schema.required.as_ref()
                                .map(|r| r.contains(name))
                                .unwrap_or(false);
                            let req_marker = if required { " (required)" } else { "" };
                            println!("      {} {}{}", name.bright_yellow(), type_str.dimmed(), req_marker.red());
                        }
                    }
                    println!();
                }
            }
        }
        "connect" => {
            // Connect to a specific server
            if let Some(server_id) = parts.get(1) {
                println!("Connecting to {}...", server_id);
                match run_async(state.mcp_manager.connect(server_id)) {
                    Ok(_) => {
                        println!("{} Connected to {}", "✓".green(), server_id);
                        // Refresh tools
                        run_async(state.refresh_mcp_tools());
                        println!("  {} tools now available", state.mcp_tools.len());
                    }
                    Err(e) => {
                        println!("{} Failed to connect: {}", "✗".red(), e);
                    }
                }
            } else {
                println!("Usage: /mcp connect <server_id>");
            }
        }
        "disconnect" => {
            // Disconnect from a specific server
            if let Some(server_id) = parts.get(1) {
                match run_async(state.mcp_manager.disconnect(server_id)) {
                    Ok(_) => {
                        println!("{} Disconnected from {}", "✓".green(), server_id);
                        run_async(state.refresh_mcp_tools());
                    }
                    Err(e) => {
                        println!("{} Failed to disconnect: {}", "✗".red(), e);
                    }
                }
            } else {
                println!("Usage: /mcp disconnect <server_id>");
            }
        }
        "add" => {
            // Add a preset server
            if let Some(preset) = parts.get(1) {
                let config = match *preset {
                    "puppeteer" => Some(("puppeteer", mcp_presets::puppeteer())),
                    "playwright" => Some(("playwright", mcp_presets::playwright())),
                    "github" => Some(("github", mcp_presets::github())),
                    "brave" | "brave-search" => Some(("brave", mcp_presets::brave_search())),
                    "memory" => Some(("memory", mcp_presets::memory())),
                    "filesystem" | "fs" => {
                        // Filesystem needs a path
                        let paths: Vec<String> = parts.iter().skip(2).map(|s| s.to_string()).collect();
                        if paths.is_empty() {
                            println!("Usage: /mcp add filesystem <path1> [path2] ...");
                            return Ok(());
                        }
                        Some(("filesystem", mcp_presets::filesystem(paths)))
                    }
                    _ => None
                };

                if let Some((id, server_config)) = config {
                    run_async(state.mcp_manager.add_server_config(id, server_config));
                    println!("{} Added {} server", "✓".green(), id);

                    // Try to connect
                    match run_async(state.mcp_manager.connect(id)) {
                        Ok(_) => {
                            println!("{} Connected to {}", "✓".green(), id);
                            run_async(state.refresh_mcp_tools());
                        }
                        Err(e) => {
                            println!("{} Added but failed to connect: {}", "⚠".yellow(), e);
                        }
                    }
                } else {
                    println!("Unknown preset: {}", preset);
                    println!("Available presets: puppeteer, playwright, github, brave-search, memory, filesystem");
                }
            } else {
                println!("Usage: /mcp add <preset>");
                println!("Available presets:");
                println!("  {} - Browser automation (recommended)", "puppeteer".bright_green());
                println!("  {} - Browser automation (alt)", "playwright".bright_green());
                println!("  {} - GitHub API access", "github".bright_green());
                println!("  {} - Web search via Brave", "brave-search".bright_green());
                println!("  {} - Persistent memory storage", "memory".bright_green());
                println!("  {} <paths...> - Filesystem access", "filesystem".bright_green());
            }
        }
        "refresh" => {
            // Refresh tool list
            run_async(state.refresh_mcp_tools());
            println!("{} Refreshed tools: {} available", "✓".green(), state.mcp_tools.len());
        }
        _ => {
            println!("Usage: /mcp [list|tools|connect|disconnect|add|refresh]");
            println!();
            println!("  {} - List connected servers and tools", "list".bright_green());
            println!("  {} - Show detailed tool information", "tools".bright_green());
            println!("  {} <id> - Connect to a configured server", "connect".bright_green());
            println!("  {} <id> - Disconnect from a server", "disconnect".bright_green());
            println!("  {} <preset> - Add and connect to a preset server", "add".bright_green());
            println!("  {} - Refresh tool cache", "refresh".bright_green());
        }
    }

    Ok(())
}

fn cmd_session(args: &str, state: &mut ReplState) -> anyhow::Result<()> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().map(|s| *s).unwrap_or("");

    match action {
        "list" | "ls" => {
            // List session logs
            match state.session_logger.list_sessions() {
                Ok(sessions) if !sessions.is_empty() => {
                    println!("\n{}\n", "Session Logs".bright_cyan().bold());

                    // Show total size
                    let total: u64 = sessions.iter().map(|(_, size, _)| size).sum();
                    println!("  Total size: {} / {} max\n",
                        format_size(total).bright_yellow(),
                        format_size(state.session_logger.max_total_size).dimmed()
                    );

                    for (i, (path, size, modified)) in sessions.iter().take(10).enumerate() {
                        let filename = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        println!(
                            "  {} {} {} {}",
                            format!("[{}]", i + 1).dimmed(),
                            filename.bright_white(),
                            format_size(*size).bright_yellow(),
                            modified.format("%Y-%m-%d %H:%M").to_string().dimmed()
                        );
                    }

                    if sessions.len() > 10 {
                        println!("\n  ... and {} more", sessions.len() - 10);
                    }

                    println!("\n  Logs stored in: {}",
                        state.session_logger.sessions_dir.display().to_string().dimmed()
                    );
                    println!();
                }
                Ok(_) => {
                    println!("No session logs found");
                }
                Err(e) => {
                    println!("{} Failed to list sessions: {}", "Error:".red(), e);
                }
            }
        }
        "path" | "current" => {
            // Show current session log path
            if let Some(path) = state.session_logger.log_path() {
                println!("Current session log: {}", path.display().to_string().bright_cyan());
            } else {
                println!("No active session log");
            }
        }
        "size" => {
            // Show total size of all logs
            match state.session_logger.total_size() {
                Ok(size) => {
                    println!(
                        "Total session logs: {} / {} max",
                        format_size(size).bright_yellow(),
                        format_size(state.session_logger.max_total_size).dimmed()
                    );
                }
                Err(e) => {
                    println!("{} Failed to get size: {}", "Error:".red(), e);
                }
            }
        }
        "save" => {
            // Flush current session
            if let Err(e) = state.session_logger.flush() {
                println!("{} {}", "Error:".red(), e);
            } else {
                if let Some(path) = state.session_logger.log_path() {
                    println!("Session saved to: {}", path.display());
                } else {
                    println!("No active session to save");
                }
            }
        }
        "resume" => {
            println!("Resume not yet implemented - sessions are logged as text files for review");
        }
        _ => {
            println!("Usage: /session [list|path|size|save]");
            println!();
            println!("  {} - List recent session logs", "list".bright_green());
            println!("  {} - Show current session log path", "path".bright_green());
            println!("  {} - Show total size of all logs", "size".bright_green());
            println!("  {} - Flush current session to disk", "save".bright_green());
        }
    }
    Ok(())
}

fn cmd_provider(args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    use setup::{ProvidersConfig, ProviderType, run_setup_wizard};

    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().map(|s| *s).unwrap_or("");

    match action {
        "list" | "ls" => {
            // List configured providers
            let config = ProvidersConfig::load();

            if config.providers.is_empty() {
                println!("\n{}\n", "No providers configured".yellow());
                println!("Run {} to add a provider", "/provider".bright_green());
                println!();
            } else {
                println!("\n{}\n", "Configured Providers".bright_cyan().bold());

                for (i, provider) in config.providers.iter().enumerate() {
                    let status = if provider.enabled { "●".green() } else { "○".dimmed() };
                    let default = if config.default_provider.as_ref() == Some(&provider.name) {
                        " (default)".bright_yellow()
                    } else {
                        "".normal()
                    };

                    let provider_info = match provider.provider_type {
                        ProviderType::Local => {
                            if let Some(ref url) = provider.base_url {
                                format!("{} @ {}", provider.provider_type.display_name(), url.dimmed())
                            } else {
                                provider.provider_type.display_name().to_string()
                            }
                        }
                        _ => provider.provider_type.display_name().to_string(),
                    };

                    println!(
                        "  {} {} {} - {}{}",
                        format!("[{}]", i + 1).dimmed(),
                        status,
                        provider.name.bright_white(),
                        provider_info,
                        default
                    );
                }
                println!();
                println!("Config file: {}", ProvidersConfig::config_path().display().to_string().dimmed());
                println!();
            }
        }
        "" | "add" => {
            // Run the setup wizard to add a new provider
            match run_setup_wizard() {
                Ok(Some(_)) => {
                    println!("{} Use {} to reload providers", "Tip:".bright_cyan(), "/provider reload".bright_green());
                }
                Ok(None) => {}
                Err(e) => {
                    println!("{} {}", "Error:".red(), e);
                }
            }
        }
        "remove" | "rm" | "delete" => {
            // Remove a provider
            if let Some(name) = parts.get(1) {
                let mut config = ProvidersConfig::load();
                let before_len = config.providers.len();
                config.providers.retain(|p| p.name != *name);

                if config.providers.len() < before_len {
                    if let Err(e) = config.save() {
                        println!("{} Failed to save: {}", "Error:".red(), e);
                    } else {
                        println!("{} Removed provider '{}'", "✓".green(), name);
                    }
                } else {
                    println!("{} Provider '{}' not found", "Error:".red(), name);
                }
            } else {
                println!("Usage: /provider remove <name>");
            }
        }
        "default" => {
            // Set default provider
            if let Some(name) = parts.get(1) {
                let mut config = ProvidersConfig::load();
                if config.providers.iter().any(|p| p.name == *name) {
                    config.default_provider = Some(name.to_string());
                    if let Err(e) = config.save() {
                        println!("{} Failed to save: {}", "Error:".red(), e);
                    } else {
                        println!("{} Set '{}' as default provider", "✓".green(), name);
                    }
                } else {
                    println!("{} Provider '{}' not found", "Error:".red(), name);
                }
            } else {
                println!("Usage: /provider default <name>");
            }
        }
        "reload" => {
            println!("{} Restart Ganesha to reload providers", "Note:".bright_cyan());
        }
        _ => {
            println!("\n{}\n", "Provider Management".bright_cyan().bold());
            println!("Usage: /provider [action]");
            println!();
            println!("  {} - List configured providers", "list".bright_green());
            println!("  {} - Add a new provider (interactive)", "add".bright_green());
            println!("  {} - Remove a provider", "remove <name>".bright_green());
            println!("  {} - Set the default provider", "default <name>".bright_green());
            println!("  {} - Reload provider configuration", "reload".bright_green());
            println!();
            println!("Supported cloud providers:");
            println!("  • Anthropic (Claude)");
            println!("  • OpenAI (GPT-4)");
            println!("  • Google (Gemini)");
            println!("  • OpenRouter");
            println!();
            println!("Local providers:");
            println!("  • LM Studio, Ollama, vLLM");
            println!("  • Any OpenAI-compatible server");
            println!();
        }
    }
    Ok(())
}

fn cmd_exit(_args: &str, _state: &mut ReplState) -> anyhow::Result<()> {
    println!("Goodbye! 🐘");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // looks_like_shell_command tests
    // ============================================================
    mod looks_like_shell_command_tests {
        use super::*;

        #[test]
        fn accepts_basic_commands() {
            assert!(looks_like_shell_command("ls -la"));
            assert!(looks_like_shell_command("cat file.txt"));
            assert!(looks_like_shell_command("grep -r pattern ."));
            assert!(looks_like_shell_command("echo hello"));
            assert!(looks_like_shell_command("cd /tmp"));
            assert!(looks_like_shell_command("git status"));
            assert!(looks_like_shell_command("npm install"));
            assert!(looks_like_shell_command("cargo build"));
            assert!(looks_like_shell_command("python3 script.py"));
        }

        #[test]
        fn accepts_compound_commands() {
            assert!(looks_like_shell_command("cd /tmp && ls"));
            assert!(looks_like_shell_command("mkdir -p dir && cd dir"));
        }

        #[test]
        fn accepts_path_prefixed_commands() {
            assert!(looks_like_shell_command("./script.sh"));
            assert!(looks_like_shell_command("/usr/bin/env python3"));
        }

        #[test]
        fn rejects_empty_string() {
            assert!(!looks_like_shell_command(""));
            assert!(!looks_like_shell_command("   "));
        }

        #[test]
        fn rejects_prose() {
            assert!(!looks_like_shell_command("If you want to list files, use ls"));
            assert!(!looks_like_shell_command("The file contains data"));
            assert!(!looks_like_shell_command("You should try running the command"));
            assert!(!looks_like_shell_command("This is a description of the problem"));
        }

        #[test]
        fn rejects_ls_output() {
            assert!(!looks_like_shell_command("drwxrwxr-x 5 user user 4096 Feb 18 file"));
            assert!(!looks_like_shell_command("-rw-r--r-- 1 user user 1234 Feb 18 file.txt"));
            assert!(!looks_like_shell_command("total 48"));
        }

        #[test]
        fn rejects_config_lines() {
            assert!(!looks_like_shell_command("[section]"));
        }

        #[test]
        fn rejects_markdown() {
            assert!(!looks_like_shell_command("## Header"));
            assert!(!looks_like_shell_command("---"));
            assert!(!looks_like_shell_command("```bash"));
        }

        #[test]
        fn rejects_code_declarations() {
            assert!(!looks_like_shell_command("fn main() {"));
            assert!(!looks_like_shell_command("pub struct Foo {"));
            assert!(!looks_like_shell_command("def hello():"));
            assert!(!looks_like_shell_command("class MyClass:"));
            assert!(!looks_like_shell_command("const x = 42;"));
            assert!(!looks_like_shell_command("function doStuff() {"));
        }

        #[test]
        fn rejects_day_names() {
            assert!(!looks_like_shell_command("Tuesday, January 20, 2026 11:36:58 AM"));
            assert!(!looks_like_shell_command("Monday morning is great"));
        }

        #[test]
        fn rejects_numbered_lists() {
            assert!(!looks_like_shell_command("1. Add the user to the system"));
            assert!(!looks_like_shell_command("2. Configure the settings"));
        }
    }

    // ============================================================
    // extract_commands tests
    // ============================================================
    mod extract_commands_tests {
        use super::*;

        #[test]
        fn extracts_from_bash_block() {
            let response = "Let me check:\n```bash\nls -la\n```\nThat should show the files.";
            let cmds = extract_commands(response);
            assert_eq!(cmds, vec!["ls -la"]);
        }

        #[test]
        fn extracts_from_shell_block() {
            let response = "```shell\ngit status\n```";
            let cmds = extract_commands(response);
            assert_eq!(cmds, vec!["git status"]);
        }

        #[test]
        fn skips_comments_in_code_blocks() {
            let response = "```bash\n# This is a comment\necho hello\n```";
            let cmds = extract_commands(response);
            assert_eq!(cmds, vec!["echo hello"]);
        }

        #[test]
        fn limits_to_first_command() {
            let response = "```bash\nls -la\npwd\necho done\n```";
            let cmds = extract_commands(response);
            assert_eq!(cmds.len(), 1);
            assert_eq!(cmds[0], "ls -la");
        }

        #[test]
        fn returns_empty_for_no_commands() {
            let response = "Here is some text without any code blocks.";
            let cmds = extract_commands(response);
            assert!(cmds.is_empty());
        }

        #[test]
        fn extracts_from_json_format() {
            let response = r#"{"cmd": ["bash", "-c", "ls -la"]}"#;
            let cmds = extract_commands(response);
            assert_eq!(cmds, vec!["ls -la"]);
        }

        #[test]
        fn extracts_from_command_json() {
            let response = r#"{"command": "git log --oneline -5"}"#;
            let cmds = extract_commands(response);
            assert_eq!(cmds, vec!["git log --oneline -5"]);
        }
    }

    // ============================================================
    // extract_command_from_json tests
    // ============================================================
    mod json_extraction_tests {
        use super::*;

        #[test]
        fn extracts_cmd_array() {
            let resp = r#"{"cmd": ["bash", "-c", "ls -la"]}"#;
            let cmd = extract_command_from_json(resp);
            assert_eq!(cmd, Some("ls -la".to_string()));
        }

        #[test]
        fn extracts_command_string() {
            let resp = r#"{"command": "echo hello"}"#;
            let cmd = extract_command_from_json(resp);
            assert_eq!(cmd, Some("echo hello".to_string()));
        }

        #[test]
        fn extracts_tool_shell() {
            let resp = r#"{"tool": "shell", "arguments": {"command": "pwd"}}"#;
            let cmd = extract_command_from_json(resp);
            assert_eq!(cmd, Some("pwd".to_string()));
        }

        #[test]
        fn returns_none_for_non_json() {
            let resp = "This is just text";
            assert!(extract_command_from_json(resp).is_none());
        }

        #[test]
        fn handles_embedded_json() {
            let resp = r#"Let me run: {"command": "find . -name '*.rs'"} to find files."#;
            let cmd = extract_command_from_json(resp);
            assert_eq!(cmd, Some("find . -name '*.rs'".to_string()));
        }
    }

    // ============================================================
    // extract_tool_calls tests
    // ============================================================
    mod tool_call_tests {
        use super::*;

        #[test]
        fn extracts_tool_block() {
            let resp = "```tool\ntool_name: puppeteer:navigate\narguments:\n  url: https://example.com\n```";
            let calls = extract_tool_calls(resp);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "puppeteer:navigate");
        }

        #[test]
        fn returns_empty_for_no_tools() {
            let resp = "No tools here, just text.";
            let calls = extract_tool_calls(resp);
            assert!(calls.is_empty());
        }

        #[test]
        fn handles_json_tool_format() {
            let resp = r#"{"name": "shell", "arguments": {"command": "ls"}}"#;
            let calls = extract_tool_calls(resp);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "shell");
        }
    }

    // ============================================================
    // clean_response tests
    // ============================================================
    mod clean_response_tests {
        use super::*;

        #[test]
        fn passes_through_normal_text() {
            let text = "Hello, this is a normal response.";
            assert_eq!(clean_response(text), text);
        }

        #[test]
        fn removes_channel_tokens() {
            let text = "<|channel|>some token<|message|>Hello world";
            let cleaned = clean_response(text);
            assert!(!cleaned.contains("<|channel|>"));
            assert!(!cleaned.contains("<|message|>"));
        }

        #[test]
        fn removes_commentary_lines() {
            let text = "commentary to=container.exec json\nHello";
            let cleaned = clean_response(text);
            assert!(!cleaned.contains("commentary"));
        }
    }

    // ============================================================
    // strip_shell_comment tests
    // ============================================================
    mod strip_comment_tests {
        use super::*;

        #[test]
        fn strips_trailing_comment() {
            assert_eq!(strip_shell_comment("ls -la # list files"), "ls -la");
        }

        #[test]
        fn preserves_comment_in_quotes() {
            assert_eq!(strip_shell_comment("echo '# not a comment'"), "echo '# not a comment'");
        }

        #[test]
        fn preserves_no_comment() {
            assert_eq!(strip_shell_comment("ls -la"), "ls -la");
        }
    }

    // ============================================================
    // is_interactive_command tests
    // ============================================================
    mod interactive_cmd_tests {
        use super::*;

        #[test]
        fn detects_editors() {
            assert!(is_interactive_command("nano file.txt").is_some());
            assert!(is_interactive_command("vim file.txt").is_some());
            assert!(is_interactive_command("vi file.txt").is_some());
        }

        #[test]
        fn detects_passwd() {
            assert!(is_interactive_command("passwd").is_some());
        }

        #[test]
        fn allows_non_interactive() {
            assert!(is_interactive_command("ls -la").is_none());
            assert!(is_interactive_command("cat file.txt").is_none());
            assert!(is_interactive_command("grep pattern file").is_none());
        }

        #[test]
        fn allows_mysql_with_flag() {
            assert!(is_interactive_command("mysql -e 'SELECT 1'").is_none());
        }
    }

    // ============================================================
    // format_size tests
    // ============================================================
    mod format_size_tests {
        use super::*;

        #[test]
        fn formats_bytes() {
            assert_eq!(format_size(42), "42B");
        }

        #[test]
        fn formats_kilobytes() {
            assert_eq!(format_size(2048), "2.0K");
        }

        #[test]
        fn formats_megabytes() {
            assert_eq!(format_size(5 * 1024 * 1024), "5.0M");
        }

        #[test]
        fn formats_gigabytes() {
            assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0G");
        }
    }

    // ============================================================
    // SessionLogger tests
    // ============================================================
    mod session_logger_tests {
        use super::*;

        #[test]
        fn new_disabled_logger() {
            let logger = SessionLogger::new(false, 1024);
            assert!(!logger.enabled);
            assert_eq!(logger.max_total_size, 1024);
        }

        #[test]
        fn new_enabled_logger() {
            let logger = SessionLogger::new(true, 512 * 1024 * 1024);
            assert!(logger.enabled);
        }

        #[test]
        fn disabled_logger_skips_operations() {
            let mut logger = SessionLogger::new(false, 1024);
            assert!(logger.log_user("test").is_ok());
            assert!(logger.log_assistant("test").is_ok());
            assert!(logger.log_command("ls", "output", true).is_ok());
            assert!(logger.end_session().is_ok());
            assert!(logger.log_path().is_none());
        }

        #[test]
        fn log_path_none_when_no_session() {
            let logger = SessionLogger::new(true, 1024);
            assert!(logger.log_path().is_none());
        }
    }
}
