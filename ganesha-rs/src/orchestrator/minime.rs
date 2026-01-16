//! Mini-Me Sub-Agent Implementation
//!
//! Mini-Me agents are lightweight, focused sub-agents that:
//! - Receive forked (minimal) context from the orchestrator
//! - Execute specific subtasks independently
//! - Report back summaries (not full transcripts)
//! - Can escalate to more capable models when stuck
//!
//! The name comes from Austin Powers - they're smaller versions of the main agent.

use super::{ForkedContext, MiniMeTask, ProviderConfig};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Tool call from Mini-Me
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Tool result back to Mini-Me
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub name: String,
    pub success: bool,
    pub output: String,
}

/// Execute a Mini-Me task
pub async fn execute_task(
    task: &MiniMeTask,
    provider: &ProviderConfig,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::builder()
        .timeout(task.timeout)
        .build()?;

    // Build the system prompt for Mini-Me
    let system_prompt = build_minime_prompt(&task.context);

    // Initial message
    let user_message = format!(
        "TASK: {}\n\nGOAL: {}\n\nRELEVANT FILES:\n{}\n\nFACTS:\n{}",
        task.description,
        task.context.goal,
        task.context.relevant_files.join("\n"),
        task.context.facts.join("\n"),
    );

    // Execute the task with tool use
    let mut conversation = vec![
        json!({"role": "system", "content": system_prompt}),
        json!({"role": "user", "content": user_message}),
    ];

    let max_turns = 20;
    let mut findings = Vec::new();
    let mut files_modified = Vec::new();

    for _turn in 0..max_turns {
        let response = call_provider(&client, provider, &conversation).await?;

        // Check if response contains tool calls
        if let Some(tool_calls) = extract_tool_calls(&response) {
            // Execute each tool
            for tool_call in tool_calls {
                let result = execute_tool(&tool_call, &task.context).await;

                if result.success
                    && tool_call.name == "write_file" {
                        if let Some(path) = tool_call.arguments.get("path").and_then(|p| p.as_str()) {
                            files_modified.push(path.to_string());
                        }
                    }

                // Add tool result to conversation
                conversation.push(json!({
                    "role": "assistant",
                    "content": response
                }));
                conversation.push(json!({
                    "role": "user",
                    "content": format!("[Tool Result: {}]\n{}", tool_call.name, result.output)
                }));
            }
        } else if response.contains("TASK_COMPLETE") || response.contains("I have completed") {
            // Task is done, extract summary
            let summary = extract_summary(&response);
            findings.push(summary.clone());
            return Ok(summary);
        } else {
            // Regular response, continue conversation
            conversation.push(json!({"role": "assistant", "content": response}));

            // Check if stuck
            if (response.contains("I need help") || response.contains("I cannot"))
                && task.allow_escalation {
                    return Err("ESCALATE: Mini-Me needs more capable model".into());
                }

            // Prompt for next action
            conversation.push(json!({
                "role": "user",
                "content": "Continue with the task. Use tools as needed. Say TASK_COMPLETE when done."
            }));
        }
    }

    Ok(format!(
        "Task completed after {} turns. Files modified: {:?}",
        max_turns,
        files_modified
    ))
}

/// Build the Mini-Me system prompt
fn build_minime_prompt(context: &ForkedContext) -> String {
    format!(r#"You are Mini-Me, a focused sub-agent executing a specific task.

CONSTRAINTS:
- You are working in: {}
- You can ONLY use these tools: {}
- Stay focused on your goal. Do not explore beyond what's needed.
- Be concise in your responses.
- When done, say "TASK_COMPLETE" followed by a brief summary.

TOOLS:
You can use tools by outputting JSON in this format:
```json
{{"tool": "tool_name", "args": {{"arg1": "value1"}}}}
```

Available tools:
- read_file: Read a file. Args: {{"path": "file/path"}}
- write_file: Write to a file. Args: {{"path": "file/path", "content": "content"}}
- bash: Run a bash command. Args: {{"command": "cmd"}}
- search: Search for text. Args: {{"pattern": "regex", "path": "dir"}}

IMPORTANT:
- Report findings concisely
- If stuck, say "I need help with: <issue>"
- Do not make changes outside your goal scope
"#,
        context.cwd,
        context.allowed_tools.join(", "),
    )
}

/// Call the provider API
async fn call_provider(
    client: &Client,
    provider: &ProviderConfig,
    messages: &[serde_json::Value],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let endpoint = if provider.endpoint.contains("anthropic") {
        format!("{}/v1/messages", provider.endpoint)
    } else {
        format!("{}/v1/chat/completions", provider.endpoint)
    };

    let request_body = if provider.endpoint.contains("anthropic") {
        // Anthropic format
        let system = messages.first()
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        let messages: Vec<_> = messages.iter().skip(1).cloned().collect();

        json!({
            "model": provider.model,
            "max_tokens": 65536,
            "system": system,
            "messages": messages,
            "temperature": 0.3
        })
    } else {
        // OpenAI-compatible format
        json!({
            "model": provider.model,
            "messages": messages,
            "temperature": 0.3,
            "max_tokens": 65536,
            "stream": false
        })
    };

    let mut req = client.post(&endpoint).json(&request_body);

    if let Some(ref key) = provider.api_key {
        if provider.endpoint.contains("anthropic") {
            req = req
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01");
        } else {
            req = req.bearer_auth(key);
        }
    }

    let response = req.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, body).into());
    }

    let json: serde_json::Value = response.json().await?;

    // Extract content based on provider
    let content = if provider.endpoint.contains("anthropic") {
        json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string()
    } else {
        json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };

    Ok(content)
}

/// Extract tool calls from response
fn extract_tool_calls(response: &str) -> Option<Vec<ToolCall>> {
    // Look for JSON blocks with tool calls
    let mut calls = Vec::new();

    // Check for fenced JSON blocks first (more reliable)
    if response.contains("```json") {
        for block in response.split("```json") {
            if let Some(end) = block.find("```") {
                let json_str = &block[..end].trim();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let (Some(name), Some(args)) = (
                        parsed.get("tool").and_then(|t| t.as_str()),
                        parsed.get("args"),
                    ) {
                        calls.push(ToolCall {
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                    }
                }
            }
        }
    }

    // Only use inline extraction if no fenced blocks found
    if calls.is_empty() {
        for line in response.lines() {
            let line = line.trim();
            if line.starts_with('{') && line.contains("\"tool\"") {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                    if let (Some(name), Some(args)) = (
                        parsed.get("tool").and_then(|t| t.as_str()),
                        parsed.get("args"),
                    ) {
                        calls.push(ToolCall {
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                    }
                }
            }
        }
    }

    if calls.is_empty() {
        None
    } else {
        Some(calls)
    }
}

/// Execute a tool call
async fn execute_tool(tool: &ToolCall, context: &ForkedContext) -> ToolResult {
    match tool.name.as_str() {
        "read_file" => {
            let path = tool.arguments.get("path")
                .and_then(|p| p.as_str())
                .unwrap_or("");

            match std::fs::read_to_string(path) {
                Ok(content) => ToolResult {
                    name: tool.name.clone(),
                    success: true,
                    output: if content.len() > 10000 {
                        format!("{}...\n[truncated, {} bytes total]", &content[..10000], content.len())
                    } else {
                        content
                    },
                },
                Err(e) => ToolResult {
                    name: tool.name.clone(),
                    success: false,
                    output: format!("Error reading file: {}", e),
                },
            }
        }

        "write_file" => {
            let path = tool.arguments.get("path")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let content = tool.arguments.get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("");

            // Safety: don't write outside cwd
            if path.contains("..") || path.starts_with('/') {
                return ToolResult {
                    name: tool.name.clone(),
                    success: false,
                    output: "Security: Cannot write outside working directory".into(),
                };
            }

            match std::fs::write(path, content) {
                Ok(_) => ToolResult {
                    name: tool.name.clone(),
                    success: true,
                    output: format!("Wrote {} bytes to {}", content.len(), path),
                },
                Err(e) => ToolResult {
                    name: tool.name.clone(),
                    success: false,
                    output: format!("Error writing file: {}", e),
                },
            }
        }

        "bash" => {
            let command = tool.arguments.get("command")
                .and_then(|c| c.as_str())
                .unwrap_or("");

            // Safety check
            let dangerous = ["rm -rf", "dd if=", "mkfs", "> /dev/", ":(){ :|:& };:"];
            for pattern in dangerous {
                if command.contains(pattern) {
                    return ToolResult {
                        name: tool.name.clone(),
                        success: false,
                        output: format!("Blocked dangerous command pattern: {}", pattern),
                    };
                }
            }

            match std::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&context.cwd)
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    ToolResult {
                        name: tool.name.clone(),
                        success: output.status.success(),
                        output: format!(
                            "Exit code: {}\nstdout:\n{}\nstderr:\n{}",
                            output.status.code().unwrap_or(-1),
                            stdout,
                            stderr
                        ),
                    }
                }
                Err(e) => ToolResult {
                    name: tool.name.clone(),
                    success: false,
                    output: format!("Error executing command: {}", e),
                },
            }
        }

        "search" => {
            let pattern = tool.arguments.get("pattern")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let path = tool.arguments.get("path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");

            // Use ripgrep if available, fallback to grep
            let output = std::process::Command::new("rg")
                .args(["--line-number", "--no-heading", pattern, path])
                .current_dir(&context.cwd)
                .output()
                .or_else(|_| {
                    std::process::Command::new("grep")
                        .args(["-rn", pattern, path])
                        .current_dir(&context.cwd)
                        .output()
                });

            match output {
                Ok(out) => {
                    let results = String::from_utf8_lossy(&out.stdout);
                    ToolResult {
                        name: tool.name.clone(),
                        success: true,
                        output: if results.is_empty() {
                            "No matches found".into()
                        } else if results.len() > 5000 {
                            format!("{}...\n[{} more bytes]", &results[..5000], results.len() - 5000)
                        } else {
                            results.to_string()
                        },
                    }
                }
                Err(e) => ToolResult {
                    name: tool.name.clone(),
                    success: false,
                    output: format!("Search error: {}", e),
                },
            }
        }

        _ => ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Unknown tool: {}", tool.name),
        },
    }
}

/// Extract summary from a TASK_COMPLETE response
fn extract_summary(response: &str) -> String {
    if let Some(idx) = response.find("TASK_COMPLETE") {
        let after = &response[idx + "TASK_COMPLETE".len()..];
        after.trim().to_string()
    } else {
        // Just take last paragraph
        response.lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tool_calls() {
        let response = r#"Let me read that file.
{"tool": "read_file", "args": {"path": "src/main.rs"}}
"#;
        let calls = extract_tool_calls(response).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
    }

    #[test]
    fn test_extract_tool_calls_fenced() {
        let response = r#"I'll search for that pattern.
```json
{"tool": "search", "args": {"pattern": "TODO", "path": "."}}
```
"#;
        let calls = extract_tool_calls(response).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search");
    }

    #[test]
    fn test_extract_summary() {
        let response = "I found the issue.\n\nTASK_COMPLETE: Fixed the bug in line 42 by updating the regex pattern.";
        let summary = extract_summary(response);
        assert!(summary.contains("Fixed the bug"));
    }
}
