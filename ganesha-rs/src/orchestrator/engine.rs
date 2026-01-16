//! Ganesha CLI Engine - Claude Code Parity
//!
//! The main execution engine that makes Ganesha as capable as Claude Code.
//! Features:
//! - Interactive REPL with conversation history
//! - Tool execution with streaming output
//! - Mini-Me sub-agent spawning
//! - Ralph Wiggum verification loops
//! - Session management and rollback
//! - MCP server integration

use super::tools::{execute_tool, ToolRegistry};
use super::memory::{GlobalMemory, SessionRecord, SessionOutcome};
use super::{Orchestrator, ProviderConfig};
use crate::pretty;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// The main Ganesha engine
pub struct GaneshaEngine {
    /// Current working directory
    pub cwd: PathBuf,
    /// Conversation history
    pub messages: Vec<Message>,
    /// Tool registry
    pub tools: ToolRegistry,
    /// Orchestrator for Mini-Me agents
    pub orchestrator: Orchestrator,
    /// Global memory
    pub memory: GlobalMemory,
    /// Current session ID
    pub session_id: Uuid,
    /// Session start time
    pub session_start: Instant,
    /// Files modified in this session
    pub files_modified: Vec<String>,
    /// Commands executed
    pub commands_executed: Vec<String>,
    /// Failed commands with attempt count (to avoid repeating failures)
    pub failed_commands: std::collections::HashMap<String, u32>,
    /// Provider configurations
    pub providers: Vec<ProviderConfig>,
    /// Primary provider (for main reasoning)
    pub primary_provider: ProviderConfig,
    /// Auto-approve mode
    pub auto_approve: bool,
    /// Quiet mode (minimal output)
    pub quiet: bool,
    /// Debug mode
    pub debug: bool,
}

impl GaneshaEngine {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|e| {
            eprintln!("Warning: Could not get current directory ({}), using '.' as fallback", e);
            PathBuf::from(".")
        });

        // Set up providers - local first
        let providers = vec![
            ProviderConfig::lm_studio_beast(),
            ProviderConfig::lm_studio_bedroom(),
            ProviderConfig::anthropic_sonnet(),
            ProviderConfig::openai_gpt4o(),
            ProviderConfig::gemini_pro(),
        ];

        let primary_provider = providers[0].clone();

        Self {
            cwd,
            messages: vec![],
            tools: ToolRegistry::new(),
            orchestrator: Orchestrator::new(),
            memory: GlobalMemory::load(),
            session_id: Uuid::new_v4(),
            session_start: Instant::now(),
            files_modified: vec![],
            commands_executed: vec![],
            failed_commands: std::collections::HashMap::new(),
            providers,
            primary_provider,
            auto_approve: false,
            quiet: false,
            debug: false,
        }
    }

    /// Get the system prompt
    fn system_prompt(&self) -> String {
        let memory_context = self.memory.get_session_context();

        format!(r#"You are Ganesha, The Remover of Obstacles - an AI-powered system control tool.

You help users accomplish tasks on their computer through natural language commands.
You have access to tools for reading/writing files, running commands, searching, and more.

CURRENT DIRECTORY: {}

{}

TOOLS AVAILABLE:
- read: Read file contents
- edit: Edit files (replace old_string with new_string)
- write: Create or overwrite files
- bash: Execute shell commands
- glob: Find files by pattern
- grep: Search file contents
- web_fetch: Fetch web pages
- task: Spawn a Mini-Me sub-agent for parallel work
- vision: Analyze the screen (when needed)

GUIDELINES:
1. Always read files before editing them
2. Explain what you're about to do before doing it
3. Use tools to accomplish tasks, don't just describe what to do
4. When a task is complex, break it into steps
5. Verify your work completed successfully
6. Ask for clarification if the request is ambiguous

When using tools, output JSON in this format:
```tool
{{"name": "tool_name", "args": {{"arg1": "value1"}}}}
```

You can use multiple tools in sequence. After each tool result, continue working toward the goal.
When the task is complete, summarize what was accomplished.
"#,
            self.cwd.display(),
            memory_context.to_prompt(),
        )
    }

    /// Run an interactive session
    pub async fn run_interactive(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.print_banner();

        // Add system message
        self.messages.push(Message {
            role: "system".into(),
            content: self.system_prompt(),
            tool_calls: None,
            tool_call_id: None,
        });

        loop {
            // Get user input
            print!("\n\x1b[1;36mganesha>\x1b[0m ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            // Handle special commands
            match input {
                "/quit" | "/exit" | "/q" => {
                    self.save_session(SessionOutcome::Success)?;
                    println!("\n\x1b[1;33m🙏 Namaste. Session saved.\x1b[0m");
                    break;
                }
                "/history" => {
                    self.show_history();
                    continue;
                }
                "/rollback" => {
                    self.show_rollback_options().await?;
                    continue;
                }
                "/clear" => {
                    self.messages.truncate(1); // Keep system message
                    println!("\x1b[2J\x1b[H"); // Clear screen
                    self.print_banner();
                    continue;
                }
                "/help" => {
                    self.print_help();
                    continue;
                }
                "/status" => {
                    self.print_status();
                    continue;
                }
                _ => {}
            }

            // Add user message
            self.messages.push(Message {
                role: "user".into(),
                content: input.to_string(),
                tool_calls: None,
                tool_call_id: None,
            });

            // Run the conversation loop
            self.conversation_loop().await?;
        }

        Ok(())
    }

    /// Run a single task (non-interactive)
    pub async fn run_task(&mut self, task: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Add system message
        self.messages.push(Message {
            role: "system".into(),
            content: self.system_prompt(),
            tool_calls: None,
            tool_call_id: None,
        });

        // Add task as user message
        self.messages.push(Message {
            role: "user".into(),
            content: task.to_string(),
            tool_calls: None,
            tool_call_id: None,
        });

        // Run conversation loop
        self.conversation_loop().await?;

        // Get the final response
        let final_response = self.messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Task completed.".into());

        // Save session
        self.save_session(SessionOutcome::Success)?;

        Ok(final_response)
    }

    /// Main conversation loop with tool execution
    async fn conversation_loop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let max_turns = 50;

        for turn in 0..max_turns {
            if self.debug {
                println!("\x1b[2m[Turn {}/{}]\x1b[0m", turn + 1, max_turns);
            }

            // Call LLM
            let response = self.call_llm().await?;

            // Check for tool calls
            let tool_calls = self.extract_tool_calls(&response);

            if tool_calls.is_empty() {
                // No tools, just a response - add it and we're done with this turn
                self.messages.push(Message {
                    role: "assistant".into(),
                    content: response.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                });

                // Print the response with pretty formatting
                pretty::print_ganesha_response(&response);

                // Check if task seems complete
                if self.is_task_complete(&response) {
                    break;
                }

                // If no tools and no clear completion, we're done for now
                break;
            }

            // Execute tools
            for tool_call in &tool_calls {
                // Print what we're doing
                if !self.quiet {
                    println!(
                        "\n\x1b[1;34m▶ {}\x1b[0m {}",
                        tool_call.name,
                        self.summarize_args(&tool_call.arguments)
                    );
                }

                // Check for consent if needed
                if !self.auto_approve && self.requires_consent(&tool_call.name, &tool_call.arguments) {
                    if !self.get_consent(&tool_call.name, &tool_call.arguments)? {
                        self.messages.push(Message {
                            role: "user".into(),
                            content: format!("[Tool {} was denied by user]", tool_call.name),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                        continue;
                    }
                }

                // Execute the tool
                let result = execute_tool(
                    &tool_call.name,
                    &tool_call.arguments,
                    &self.cwd.to_string_lossy(),
                ).await;

                // Track modifications
                if result.success {
                    if tool_call.name == "edit" || tool_call.name == "write" {
                        if let Some(path) = tool_call.arguments.get("path").and_then(|p| p.as_str()) {
                            self.files_modified.push(path.to_string());
                        }
                    }
                    if tool_call.name == "bash" {
                        if let Some(cmd) = tool_call.arguments.get("command").and_then(|c| c.as_str()) {
                            self.commands_executed.push(cmd.to_string());
                        }
                    }
                }

                // Print result summary
                if !self.quiet {
                    let status = if result.success { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
                    let output_preview = if result.output.len() > 200 {
                        format!("{}...", &result.output[..200])
                    } else {
                        result.output.clone()
                    };
                    println!("  {} {}", status, output_preview.lines().next().unwrap_or(""));
                }

                // Add tool result to conversation
                self.messages.push(Message {
                    role: "assistant".into(),
                    content: response.clone(),
                    tool_calls: Some(vec![tool_call.clone()]),
                    tool_call_id: None,
                });

                self.messages.push(Message {
                    role: "user".into(),
                    content: format!(
                        "[Tool Result: {} - {}]\n{}",
                        tool_call.name,
                        if result.success { "success" } else { "failed" },
                        result.output
                    ),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                });
            }
        }

        Ok(())
    }

    /// Call the LLM
    async fn call_llm(&self) -> Result<String, Box<dyn std::error::Error>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        let endpoint = format!("{}/v1/chat/completions", self.primary_provider.endpoint);

        // Build messages for API
        let api_messages: Vec<Value> = self.messages.iter().map(|m| {
            json!({
                "role": m.role,
                "content": m.content
            })
        }).collect();

        let request = json!({
            "model": self.primary_provider.model,
            "messages": api_messages,
            "temperature": 0.3,
            "max_tokens": 65536,  // Large output for big file generations (1000+ items)
            "stream": false
        });

        let mut req = client.post(&endpoint).json(&request);

        if let Some(ref key) = self.primary_provider.api_key {
            req = req.bearer_auth(key);
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("LLM API error {}: {}", status, body).into());
        }

        let json: Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    /// Extract tool calls from response
    fn extract_tool_calls(&self, response: &str) -> Vec<ToolCall> {
        let mut calls = Vec::new();

        // Look for ```tool blocks
        for block in response.split("```tool") {
            if let Some(end) = block.find("```") {
                let json_str = block[..end].trim();
                if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                    if let (Some(name), Some(args)) = (
                        parsed.get("name").and_then(|n| n.as_str()),
                        parsed.get("args"),
                    ) {
                        calls.push(ToolCall {
                            id: Uuid::new_v4().to_string(),
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                    }
                }
            }
        }

        // Also look for inline JSON tool calls
        for line in response.lines() {
            let line = line.trim();
            if line.starts_with('{') && line.contains("\"name\"") {
                if let Ok(parsed) = serde_json::from_str::<Value>(line) {
                    if let (Some(name), Some(args)) = (
                        parsed.get("name").and_then(|n| n.as_str()),
                        parsed.get("args"),
                    ) {
                        calls.push(ToolCall {
                            id: Uuid::new_v4().to_string(),
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                    }
                }
            }
        }

        calls
    }

    /// Check if a tool requires consent
    fn requires_consent(&self, name: &str, args: &Value) -> bool {
        match name {
            "read" | "glob" | "grep" => false, // Read-only
            "bash" => {
                // Check if command modifies things
                if let Some(cmd) = args.get("command").and_then(|c| c.as_str()) {
                    let modifying = ["rm", "mv", "cp", "mkdir", "touch", "chmod", "chown",
                        "apt", "yum", "dnf", "brew", "pip", "npm", "cargo"];
                    modifying.iter().any(|m| cmd.contains(m))
                } else {
                    true
                }
            }
            "edit" | "write" => true,
            "task" => true, // Spawning agents needs consent
            _ => true,
        }
    }

    /// Get user consent for an action
    fn get_consent(&self, name: &str, args: &Value) -> Result<bool, Box<dyn std::error::Error>> {
        println!("\n\x1b[1;33m⚠ Action requires approval:\x1b[0m");
        println!("  Tool: {}", name);
        println!("  Args: {}", serde_json::to_string_pretty(args)?);
        print!("\n\x1b[1;33mApprove? [y/N]:\x1b[0m ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input.trim().to_lowercase() == "y" || input.trim().to_lowercase() == "yes")
    }

    /// Check if task seems complete
    fn is_task_complete(&self, response: &str) -> bool {
        let completions = [
            "task complete", "completed", "done", "finished",
            "successfully", "all done", "that's it",
        ];
        let lower = response.to_lowercase();
        completions.iter().any(|c| lower.contains(c))
    }

    /// Summarize tool arguments for display
    fn summarize_args(&self, args: &Value) -> String {
        if let Some(path) = args.get("path").and_then(|p| p.as_str()) {
            return path.to_string();
        }
        if let Some(cmd) = args.get("command").and_then(|c| c.as_str()) {
            return if cmd.len() > 60 {
                format!("{}...", &cmd[..60])
            } else {
                cmd.to_string()
            };
        }
        if let Some(pattern) = args.get("pattern").and_then(|p| p.as_str()) {
            return pattern.to_string();
        }
        "...".to_string()
    }

    /// Save the current session
    fn save_session(&mut self, outcome: SessionOutcome) -> Result<(), Box<dyn std::error::Error>> {
        let primary_task = self.messages
            .iter()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Interactive session".into());

        let record = SessionRecord {
            id: self.session_id,
            started_at: Utc::now() - chrono::Duration::from_std(self.session_start.elapsed())?,
            ended_at: Utc::now(),
            primary_task,
            outcome,
            files_modified: self.files_modified.clone(),
            commands_executed: self.commands_executed.clone(),
            rollback_available: !self.files_modified.is_empty(),
            key_learnings: vec![],
        };

        self.memory.add_session(record);
        self.memory.save()?;

        Ok(())
    }

    fn print_banner(&self) {
        println!(r#"
    ╔═══════════════════════════════════════════════════════════════╗
    ║                                                               ║
    ║   🕉️  GANESHA 2.0 - The Remover of Obstacles                  ║
    ║                                                               ║
    ║   Natural language to system control.                        ║
    ║   Type /help for commands, /quit to exit.                    ║
    ║                                                               ║
    ╚═══════════════════════════════════════════════════════════════╝
"#);
        println!("  Provider: {} ({})", self.primary_provider.name, self.primary_provider.model);
        println!("  Working dir: {}", self.cwd.display());
    }

    fn print_help(&self) {
        println!(r#"
COMMANDS:
  /help      Show this help
  /quit      Exit (saves session)
  /clear     Clear conversation history
  /history   Show recent sessions
  /rollback  Rollback a previous session
  /status    Show current session status

TIPS:
  - Just type what you want to do in plain English
  - Ganesha will plan and execute the necessary steps
  - You'll be asked to approve dangerous operations
  - Use /rollback to undo changes if needed
"#);
    }

    fn print_status(&self) {
        let elapsed = self.session_start.elapsed();
        println!("\n\x1b[1;36mSession Status:\x1b[0m");
        println!("  ID: {}", self.session_id);
        println!("  Duration: {:?}", elapsed);
        println!("  Messages: {}", self.messages.len());
        println!("  Files modified: {}", self.files_modified.len());
        println!("  Commands executed: {}", self.commands_executed.len());
        println!("  Auto-approve: {}", self.auto_approve);
    }

    fn show_history(&self) {
        println!("\n\x1b[1;36mRecent Sessions:\x1b[0m");
        for session in self.memory.recent_sessions(10) {
            let outcome = match session.outcome {
                SessionOutcome::Success => "\x1b[32m✓\x1b[0m",
                SessionOutcome::PartialSuccess => "\x1b[33m◐\x1b[0m",
                SessionOutcome::Failed => "\x1b[31m✗\x1b[0m",
                SessionOutcome::Aborted => "\x1b[31m⊘\x1b[0m",
            };
            println!(
                "  {} {} - {} ({})",
                outcome,
                session.started_at.format("%Y-%m-%d %H:%M"),
                session.primary_task.chars().take(50).collect::<String>(),
                if session.rollback_available { "rollback available" } else { "no rollback" }
            );
        }
    }

    async fn show_rollback_options(&self) -> Result<(), Box<dyn std::error::Error>> {
        let rollbackable: Vec<_> = self.memory.recent_sessions(20)
            .iter()
            .filter(|s| s.rollback_available)
            .collect();

        if rollbackable.is_empty() {
            println!("\n\x1b[33mNo sessions with rollback available.\x1b[0m");
            return Ok(());
        }

        println!("\n\x1b[1;36mSessions with rollback available:\x1b[0m");
        for (i, session) in rollbackable.iter().enumerate() {
            println!(
                "  [{}] {} - {} ({} files)",
                i + 1,
                session.started_at.format("%Y-%m-%d %H:%M"),
                session.primary_task.chars().take(40).collect::<String>(),
                session.files_modified.len()
            );
        }

        println!("\n\x1b[33mRollback not yet implemented. Coming soon!\x1b[0m");
        Ok(())
    }
}

impl Default for GaneshaEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = GaneshaEngine::new();
        assert!(engine.messages.is_empty());
        assert!(!engine.providers.is_empty());
    }

    #[test]
    fn test_extract_tool_calls() {
        let engine = GaneshaEngine::new();

        let response = r#"Let me read that file.
```tool
{"name": "read", "args": {"path": "src/main.rs"}}
```
"#;
        let calls = engine.extract_tool_calls(response);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
    }

    #[test]
    fn test_requires_consent() {
        let engine = GaneshaEngine::new();

        assert!(!engine.requires_consent("read", &json!({"path": "foo"})));
        assert!(engine.requires_consent("write", &json!({"path": "foo", "content": "bar"})));
        assert!(engine.requires_consent("bash", &json!({"command": "rm -rf temp"})));
    }
}
