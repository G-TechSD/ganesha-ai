//! Ganesha Agent Mode - Full Agentic Coding Assistant
//!
//! This is the "Claude Code parity" mode with:
//! - Read/Edit/Write file operations
//! - Bash command execution
//! - Multi-turn conversation with tool use
//! - Iterative verification

use crate::cli::{print_banner, print_error, print_info, print_success, print_warning};
use crate::orchestrator::tools::{execute_tool, ToolRegistry};
use crate::pretty;
use console::style;
use rustyline::error::ReadlineError;
use rustyline::{Config, DefaultEditor, EditMode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Message in the conversation
#[derive(Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

/// Tool call parsed from LLM response
#[derive(Debug, Clone)]
struct ToolCall {
    name: String,
    args: Value,
}

/// The Agent Engine
pub struct AgentEngine {
    cwd: PathBuf,
    messages: Vec<Message>,
    tools: ToolRegistry,
    provider_url: String,
    model: String,
    auto_approve: bool,
    max_turns: usize,
    files_modified: Vec<String>,
    commands_executed: Vec<String>,
}

impl AgentEngine {
    pub fn new(provider_url: &str, model: &str) -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            messages: vec![],
            tools: ToolRegistry::new(),
            provider_url: provider_url.to_string(),
            model: model.to_string(),
            auto_approve: false,
            max_turns: 30,
            files_modified: vec![],
            commands_executed: vec![],
        }
    }

    pub fn set_auto_approve(&mut self, auto: bool) {
        self.auto_approve = auto;
    }

    fn system_prompt(&self) -> String {
        format!(r#"You are Ganesha, an expert AI coding assistant. You help users with software engineering tasks.

CURRENT DIRECTORY: {}

You have access to these tools:

1. **read** - Read file contents
   ```tool
   {{"name": "read", "args": {{"path": "src/main.rs"}}}}
   ```

2. **edit** - Edit files by replacing text (old_string must be unique in file)
   ```tool
   {{"name": "edit", "args": {{"path": "src/main.rs", "old_string": "old code", "new_string": "new code"}}}}
   ```

3. **write** - Create or overwrite a file
   ```tool
   {{"name": "write", "args": {{"path": "new_file.rs", "content": "file content here"}}}}
   ```

4. **bash** - Run shell commands
   ```tool
   {{"name": "bash", "args": {{"command": "cargo build"}}}}
   ```

5. **glob** - Find files by pattern
   ```tool
   {{"name": "glob", "args": {{"pattern": "**/*.rs"}}}}
   ```

6. **grep** - Search file contents
   ```tool
   {{"name": "grep", "args": {{"pattern": "TODO", "path": "src"}}}}
   ```

IMPORTANT RULES:
1. ALWAYS read files before editing them
2. Use tools to accomplish tasks - don't just describe what to do
3. After modifying files, verify the changes worked (run tests, build, etc.)
4. When a task is complex, break it into steps and execute them one by one
5. If something fails, analyze the error and try a different approach
6. Be concise in explanations but thorough in execution

When you're done with a task, summarize what was accomplished."#,
            self.cwd.display()
        )
    }

    /// Run the agent on a single task
    pub async fn run_task(&mut self, task: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize with system message
        self.messages.push(Message {
            role: "system".into(),
            content: self.system_prompt(),
        });

        // Add user task
        self.messages.push(Message {
            role: "user".into(),
            content: task.to_string(),
        });

        println!("\n{}", style("Working on task...").cyan().bold());

        // Run the agentic loop
        let result = self.agent_loop().await?;

        Ok(result)
    }

    /// Run interactive REPL mode
    pub async fn run_interactive(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Initialize with system message
        self.messages.push(Message {
            role: "system".into(),
            content: self.system_prompt(),
        });

        let config = Config::builder()
            .edit_mode(EditMode::Emacs)
            .build();

        let mut rl = DefaultEditor::with_config(config)?;

        // Load history
        let history_path = dirs::data_dir()
            .map(|p| p.join("ganesha").join("agent_history.txt"))
            .unwrap_or_else(|| PathBuf::from(".ganesha_agent_history"));
        if history_path.exists() {
            let _ = rl.load_history(&history_path);
        }

        println!("\n{}", style("─".repeat(60)).dim());
        println!("{}", style("Agent Mode - Full coding assistant").cyan().bold());
        println!("{}", style("Tools: read, edit, write, bash, glob, grep").dim());
        println!("{}", style("Commands: /help /clear /files exit").dim());
        println!("{}\n", style("─".repeat(60)).dim());

        let mut last_interrupt: Option<Instant> = None;

        loop {
            let prompt = format!("{} ", style("agent>").green().bold());

            match rl.readline(&prompt) {
                Ok(line) => {
                    last_interrupt = None;
                    let input = line.trim();

                    if input.is_empty() {
                        continue;
                    }

                    let _ = rl.add_history_entry(input);

                    // Handle commands
                    match input.to_lowercase().as_str() {
                        "exit" | "quit" | "/quit" | "/exit" => {
                            println!("{}", style("Namaste 🙏").yellow());
                            break;
                        }
                        "/help" => {
                            self.print_help();
                            continue;
                        }
                        "/clear" => {
                            self.messages.truncate(1); // Keep system message
                            println!("{}", style("Conversation cleared.").dim());
                            continue;
                        }
                        "/files" => {
                            println!("\n{}", style("Files modified this session:").cyan());
                            for f in &self.files_modified {
                                println!("  - {}", f);
                            }
                            println!();
                            continue;
                        }
                        _ => {}
                    }

                    // Add user message
                    self.messages.push(Message {
                        role: "user".into(),
                        content: input.to_string(),
                    });

                    // Run agent loop
                    match self.agent_loop().await {
                        Ok(_) => {}
                        Err(e) => {
                            print_error(&format!("Error: {}", e));
                        }
                    }

                    println!(); // Spacing
                }
                Err(ReadlineError::Interrupted) => {
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
                Err(e) => {
                    print_error(&format!("Input error: {}", e));
                    break;
                }
            }
        }

        // Save history
        if let Some(parent) = history_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = rl.save_history(&history_path);

        Ok(())
    }

    /// The main agentic loop
    async fn agent_loop(&mut self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut last_response = String::new();

        for turn in 0..self.max_turns {
            // Call LLM
            let response = self.call_llm().await?;
            last_response = response.clone();

            // Extract tool calls
            let tool_calls = self.extract_tool_calls(&response);

            if tool_calls.is_empty() {
                // No tools - just a response
                self.messages.push(Message {
                    role: "assistant".into(),
                    content: response.clone(),
                });

                // Print the response with pretty formatting
                pretty::print_ganesha_response(&response);

                // Check if task seems complete
                if self.is_task_complete(&response) {
                    break;
                }

                // No more tools to execute
                break;
            }

            // Execute each tool
            for tool_call in tool_calls {
                // Show what we're doing
                let tool_display = match tool_call.name.as_str() {
                    "read" => format!("📖 Reading {}", tool_call.args.get("path").and_then(|p| p.as_str()).unwrap_or("file")),
                    "edit" => format!("✏️  Editing {}", tool_call.args.get("path").and_then(|p| p.as_str()).unwrap_or("file")),
                    "write" => format!("📝 Writing {}", tool_call.args.get("path").and_then(|p| p.as_str()).unwrap_or("file")),
                    "bash" => format!("⚡ Running: {}", tool_call.args.get("command").and_then(|c| c.as_str()).unwrap_or("command").chars().take(50).collect::<String>()),
                    "glob" => format!("🔍 Finding {}", tool_call.args.get("pattern").and_then(|p| p.as_str()).unwrap_or("files")),
                    "grep" => format!("🔎 Searching for '{}'", tool_call.args.get("pattern").and_then(|p| p.as_str()).unwrap_or("pattern")),
                    _ => format!("🔧 {}", tool_call.name),
                };

                println!("{}", style(&tool_display).yellow());

                // Check consent for dangerous operations
                if !self.auto_approve && self.requires_consent(&tool_call) {
                    if !self.get_consent(&tool_call)? {
                        self.messages.push(Message {
                            role: "user".into(),
                            content: format!("[Tool {} was DENIED by user. Try a different approach.]", tool_call.name),
                        });
                        continue;
                    }
                }

                // Execute
                let result = execute_tool(
                    &tool_call.name,
                    &tool_call.args,
                    &self.cwd.to_string_lossy(),
                ).await;

                // Track modifications
                if result.success {
                    if tool_call.name == "edit" || tool_call.name == "write" {
                        if let Some(path) = tool_call.args.get("path").and_then(|p| p.as_str()) {
                            if !self.files_modified.contains(&path.to_string()) {
                                self.files_modified.push(path.to_string());
                            }
                        }
                    }
                    if tool_call.name == "bash" {
                        if let Some(cmd) = tool_call.args.get("command").and_then(|c| c.as_str()) {
                            self.commands_executed.push(cmd.to_string());
                        }
                    }
                }

                // Show result status
                if result.success {
                    let preview = result.output.lines().next().unwrap_or("").chars().take(60).collect::<String>();
                    println!("  {} {}", style("✓").green().bold(), style(&preview).dim());
                } else {
                    println!("  {} {}", style("✗").red().bold(), style(&result.output.lines().next().unwrap_or("Failed")).dim());
                }

                // Add to conversation
                self.messages.push(Message {
                    role: "assistant".into(),
                    content: format!("Using tool: {}", tool_call.name),
                });

                self.messages.push(Message {
                    role: "user".into(),
                    content: format!(
                        "[Tool Result: {} - {}]\n{}",
                        tool_call.name,
                        if result.success { "SUCCESS" } else { "FAILED" },
                        // Truncate large outputs
                        if result.output.len() > 10000 {
                            format!("{}...\n[truncated, {} chars total]", &result.output[..10000], result.output.len())
                        } else {
                            result.output
                        }
                    ),
                });
            }
        }

        Ok(last_response)
    }

    /// Call the LLM API
    async fn call_llm(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;

        let endpoint = format!("{}/v1/chat/completions", self.provider_url);

        let api_messages: Vec<Value> = self.messages.iter().map(|m| {
            json!({
                "role": m.role,
                "content": m.content
            })
        }).collect();

        let request = json!({
            "model": self.model,
            "messages": api_messages,
            "temperature": 0.2,
            "max_tokens": 65536,  // Large output for big file generations
            "stream": false
        });

        let response = client
            .post(&endpoint)
            .json(&request)
            .send()
            .await?;

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

    /// Extract tool calls from LLM response
    fn extract_tool_calls(&self, response: &str) -> Vec<ToolCall> {
        let mut calls = Vec::new();

        // Look for ```tool blocks
        for block in response.split("```tool") {
            if let Some(end) = block.find("```") {
                let json_str = block[..end].trim();
                if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                    if let (Some(name), args) = (
                        parsed.get("name").and_then(|n| n.as_str()),
                        parsed.get("args").cloned().unwrap_or(json!({})),
                    ) {
                        calls.push(ToolCall {
                            name: name.to_string(),
                            args,
                        });
                    }
                }
            }
        }

        // Handle <|channel|> format from local models
        // Format: <|channel|>commentary to=bash <|constrain|>json<|message|>{"command":"..."}
        // The tool name comes from "to=xxx" and the JSON is the args directly
        if response.contains("<|channel|>") && response.contains("<|message|>") {
            for part in response.split("<|channel|>") {
                if let Some(msg_start) = part.find("<|message|>") {
                    let json_part = &part[msg_start + 11..]; // Skip "<|message|>"

                    // Find the tool type from "to=xxx" pattern
                    let tool_name = if part.contains("to=bash") {
                        Some("bash")
                    } else if part.contains("to=write") {
                        Some("write")
                    } else if part.contains("to=read") {
                        Some("read")
                    } else if part.contains("to=edit") {
                        Some("edit")
                    } else if part.contains("to=glob") {
                        Some("glob")
                    } else if part.contains("to=grep") {
                        Some("grep")
                    } else {
                        None
                    };

                    if let Some(name) = tool_name {
                        // Find the JSON object
                        if let Some(json_start) = json_part.find('{') {
                            if let Some(json_end) = json_part.rfind('}') {
                                let json_str = &json_part[json_start..=json_end];
                                if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                                    // The JSON IS the args (not wrapped in name/args)
                                    calls.push(ToolCall {
                                        name: name.to_string(),
                                        args: parsed,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Look for <|message|>{json} format (some LLMs use this with name/args structure)
        if calls.is_empty() {
            if let Some(start) = response.find("<|message|>") {
                let after = &response[start + 11..];
                if let Some(json_start) = after.find('{') {
                    if let Some(json_end) = after.rfind('}') {
                        let json_str = &after[json_start..=json_end];
                        if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                            if let (Some(name), args) = (
                                parsed.get("name").and_then(|n| n.as_str()),
                                parsed.get("args").cloned().unwrap_or(json!({})),
                            ) {
                                calls.push(ToolCall {
                                    name: name.to_string(),
                                    args,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Also look for inline JSON tool calls
        for line in response.lines() {
            let line = line.trim();
            if line.starts_with('{') && line.contains("\"name\"") && line.contains("\"args\"") {
                if let Ok(parsed) = serde_json::from_str::<Value>(line) {
                    if let (Some(name), args) = (
                        parsed.get("name").and_then(|n| n.as_str()),
                        parsed.get("args").cloned().unwrap_or(json!({})),
                    ) {
                        // Avoid duplicates
                        if !calls.iter().any(|c| c.name == name) {
                            calls.push(ToolCall {
                                name: name.to_string(),
                                args,
                            });
                        }
                    }
                }
            }
        }

        // Generic JSON extraction - find any JSON object with name/args
        if calls.is_empty() {
            if let Some(start) = response.find('{') {
                if let Some(end) = response.rfind('}') {
                    let json_str = &response[start..=end];
                    if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                        if let (Some(name), args) = (
                            parsed.get("name").and_then(|n| n.as_str()),
                            parsed.get("args").cloned().unwrap_or(json!({})),
                        ) {
                            calls.push(ToolCall {
                                name: name.to_string(),
                                args,
                            });
                        }
                    }
                }
            }
        }

        calls
    }

    /// Check if tool requires consent
    fn requires_consent(&self, tool: &ToolCall) -> bool {
        match tool.name.as_str() {
            "read" | "glob" | "grep" => false,
            "bash" => {
                if let Some(cmd) = tool.args.get("command").and_then(|c| c.as_str()) {
                    // Modifying commands need approval
                    let modifying = ["rm", "mv", "apt", "yum", "pip install", "npm install", "cargo install"];
                    modifying.iter().any(|m| cmd.contains(m))
                } else {
                    true
                }
            }
            "edit" | "write" => true,
            _ => true,
        }
    }

    /// Get user consent
    fn get_consent(&self, tool: &ToolCall) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        println!("\n{}", style("⚠  Approval required:").yellow().bold());
        println!("  Tool: {}", tool.name);

        // Show relevant args
        match tool.name.as_str() {
            "edit" => {
                if let Some(path) = tool.args.get("path").and_then(|p| p.as_str()) {
                    println!("  File: {}", path);
                }
            }
            "write" => {
                if let Some(path) = tool.args.get("path").and_then(|p| p.as_str()) {
                    println!("  File: {}", path);
                }
            }
            "bash" => {
                if let Some(cmd) = tool.args.get("command").and_then(|c| c.as_str()) {
                    println!("  Command: {}", cmd);
                }
            }
            _ => {
                println!("  Args: {}", serde_json::to_string_pretty(&tool.args).unwrap_or_default());
            }
        }

        print!("\n{} ", style("Approve? [y/N]:").yellow());
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input.trim().to_lowercase().starts_with('y'))
    }

    /// Check if task seems complete
    fn is_task_complete(&self, response: &str) -> bool {
        let lower = response.to_lowercase();
        let completions = [
            "task complete", "completed successfully", "all done",
            "finished", "i've completed", "successfully created",
            "successfully updated", "that's it", "done!",
        ];
        completions.iter().any(|c| lower.contains(c))
    }

    fn print_help(&self) {
        println!(r#"
{} - Full coding assistant with tool use

{}
  Just describe what you want to do in plain English.
  The agent will use tools to accomplish the task.

{}
  /help    - Show this help
  /clear   - Clear conversation history
  /files   - Show files modified this session
  exit     - Exit agent mode

{}
  read   - Read file contents
  edit   - Edit files (find/replace)
  write  - Create/overwrite files
  bash   - Run shell commands
  glob   - Find files by pattern
  grep   - Search file contents
"#,
            style("Agent Mode").cyan().bold(),
            style("Usage:").yellow().bold(),
            style("Commands:").yellow().bold(),
            style("Tools:").yellow().bold(),
        );
    }
}

/// Run agent mode
pub async fn run_agent(
    provider_url: &str,
    model: &str,
    task: Option<&str>,
    auto_approve: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut engine = AgentEngine::new(provider_url, model);
    engine.set_auto_approve(auto_approve);

    if let Some(task) = task {
        // Single task mode
        engine.run_task(task).await?;
    } else {
        // Interactive mode
        engine.run_interactive().await?;
    }

    Ok(())
}
