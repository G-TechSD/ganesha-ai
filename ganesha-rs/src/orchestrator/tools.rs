//! Ganesha Tool System
//!
//! Tools available to the orchestrator and Mini-Me agents.
//! Modeled after Claude Code's comprehensive toolset.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecResult {
    pub success: bool,
    pub output: String,
    pub metadata: HashMap<String, Value>,
}

/// Available tools registry
pub struct ToolRegistry {
    tools: HashMap<String, ToolDef>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_core_tools();
        registry
    }

    fn register_core_tools(&mut self) {
        // Read tool
        self.tools.insert("read".into(), ToolDef {
            name: "read".into(),
            description: "Read a file from disk. Supports text files and common formats.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute or relative path to file"},
                    "offset": {"type": "integer", "description": "Line number to start reading from"},
                    "limit": {"type": "integer", "description": "Maximum lines to read"}
                },
                "required": ["path"]
            }),
        });

        // Edit tool
        self.tools.insert("edit".into(), ToolDef {
            name: "edit".into(),
            description: "Edit a file by replacing old_string with new_string. The old_string must be unique.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path to file to edit"},
                    "old_string": {"type": "string", "description": "Exact string to replace"},
                    "new_string": {"type": "string", "description": "Replacement string"}
                },
                "required": ["path", "old_string", "new_string"]
            }),
        });

        // Write tool
        self.tools.insert("write".into(), ToolDef {
            name: "write".into(),
            description: "Write content to a file (creates or overwrites).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path to file to write"},
                    "content": {"type": "string", "description": "Content to write"}
                },
                "required": ["path", "content"]
            }),
        });

        // Bash tool
        self.tools.insert("bash".into(), ToolDef {
            name: "bash".into(),
            description: "Execute a bash command. Use for git, npm, system commands.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Command to execute"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds (default 60)"}
                },
                "required": ["command"]
            }),
        });

        // Glob tool
        self.tools.insert("glob".into(), ToolDef {
            name: "glob".into(),
            description: "Find files matching a glob pattern.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern like **/*.rs"},
                    "path": {"type": "string", "description": "Base directory (default: cwd)"}
                },
                "required": ["pattern"]
            }),
        });

        // Grep tool
        self.tools.insert("grep".into(), ToolDef {
            name: "grep".into(),
            description: "Search for patterns in files using regex.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex pattern to search"},
                    "path": {"type": "string", "description": "Directory or file to search"},
                    "type": {"type": "string", "description": "File type filter (e.g., 'rs', 'py')"}
                },
                "required": ["pattern"]
            }),
        });

        // Web fetch tool
        self.tools.insert("web_fetch".into(), ToolDef {
            name: "web_fetch".into(),
            description: "Fetch content from a URL.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to fetch"},
                    "prompt": {"type": "string", "description": "What to extract from the page"}
                },
                "required": ["url"]
            }),
        });

        // Vision tool (screen analysis)
        self.tools.insert("vision".into(), ToolDef {
            name: "vision".into(),
            description: "Capture and analyze the current screen state.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "What to look for on screen"}
                },
                "required": []
            }),
        });

        // Task tool (spawn Mini-Me)
        self.tools.insert("task".into(), ToolDef {
            name: "task".into(),
            description: "Spawn a Mini-Me sub-agent for a specific task.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "description": {"type": "string", "description": "Task description"},
                    "context": {"type": "array", "items": {"type": "string"}, "description": "Relevant file paths"},
                    "allow_escalation": {"type": "boolean", "description": "Allow escalation to cloud"}
                },
                "required": ["description"]
            }),
        });
    }

    pub fn get_tool(&self, name: &str) -> Option<&ToolDef> {
        self.tools.get(name)
    }

    pub fn list_tools(&self) -> Vec<&ToolDef> {
        self.tools.values().collect()
    }

    pub fn get_tools_json(&self) -> Value {
        let tools: Vec<Value> = self.tools.values().map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                }
            })
        }).collect();
        json!(tools)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a tool
pub async fn execute_tool(
    name: &str,
    args: &Value,
    cwd: &str,
) -> ToolExecResult {
    match name {
        "read" => exec_read(args, cwd),
        "edit" => exec_edit(args, cwd),
        "write" => exec_write(args, cwd),
        "bash" => exec_bash(args, cwd).await,
        "glob" => exec_glob(args, cwd),
        "grep" => exec_grep(args, cwd),
        "web_fetch" => exec_web_fetch(args).await,
        _ => ToolExecResult {
            success: false,
            output: format!("Unknown tool: {}", name),
            metadata: HashMap::new(),
        },
    }
}

fn exec_read(args: &Value, cwd: &str) -> ToolExecResult {
    let path = args["path"].as_str().unwrap_or("");
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let limit = args["limit"].as_u64().unwrap_or(2000) as usize;

    let full_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", cwd, path)
    };

    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();
            let selected: Vec<String> = lines
                .into_iter()
                .skip(offset)
                .take(limit)
                .enumerate()
                .map(|(i, line)| format!("{:>6}\t{}", offset + i + 1, line))
                .collect();

            let mut metadata = HashMap::new();
            metadata.insert("total_lines".into(), json!(total_lines));
            metadata.insert("offset".into(), json!(offset));
            metadata.insert("lines_returned".into(), json!(selected.len()));

            ToolExecResult {
                success: true,
                output: selected.join("\n"),
                metadata,
            }
        }
        Err(e) => ToolExecResult {
            success: false,
            output: format!("Error reading file: {}", e),
            metadata: HashMap::new(),
        },
    }
}

fn exec_edit(args: &Value, cwd: &str) -> ToolExecResult {
    let path = args["path"].as_str().unwrap_or("");
    let old_string = args["old_string"].as_str().unwrap_or("");
    let new_string = args["new_string"].as_str().unwrap_or("");

    if old_string.is_empty() {
        return ToolExecResult {
            success: false,
            output: "old_string cannot be empty".into(),
            metadata: HashMap::new(),
        };
    }

    let full_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", cwd, path)
    };

    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let count = content.matches(old_string).count();

            if count == 0 {
                return ToolExecResult {
                    success: false,
                    output: format!("old_string not found in file: {:?}", old_string),
                    metadata: HashMap::new(),
                };
            }

            if count > 1 {
                return ToolExecResult {
                    success: false,
                    output: format!(
                        "old_string is not unique (found {} times). Provide more context.",
                        count
                    ),
                    metadata: HashMap::new(),
                };
            }

            let new_content = content.replace(old_string, new_string);

            match std::fs::write(&full_path, &new_content) {
                Ok(_) => {
                    let mut metadata = HashMap::new();
                    metadata.insert("bytes_before".into(), json!(content.len()));
                    metadata.insert("bytes_after".into(), json!(new_content.len()));

                    ToolExecResult {
                        success: true,
                        output: format!("Successfully edited {}", path),
                        metadata,
                    }
                }
                Err(e) => ToolExecResult {
                    success: false,
                    output: format!("Error writing file: {}", e),
                    metadata: HashMap::new(),
                },
            }
        }
        Err(e) => ToolExecResult {
            success: false,
            output: format!("Error reading file: {}", e),
            metadata: HashMap::new(),
        },
    }
}

fn exec_write(args: &Value, cwd: &str) -> ToolExecResult {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");

    let full_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", cwd, path)
    };

    // Create parent directories if needed
    if let Some(parent) = Path::new(&full_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::write(&full_path, content) {
        Ok(_) => {
            let mut metadata = HashMap::new();
            metadata.insert("bytes_written".into(), json!(content.len()));

            ToolExecResult {
                success: true,
                output: format!("Wrote {} bytes to {}", content.len(), path),
                metadata,
            }
        }
        Err(e) => ToolExecResult {
            success: false,
            output: format!("Error writing file: {}", e),
            metadata: HashMap::new(),
        },
    }
}

async fn exec_bash(args: &Value, cwd: &str) -> ToolExecResult {
    let command = args["command"].as_str().unwrap_or("");
    let timeout_secs = args["timeout"].as_u64().unwrap_or(60);

    // Safety checks
    let dangerous_patterns = [
        "rm -rf /",
        "rm -rf /*",
        "dd if=/dev/zero",
        "dd if=/dev/random",
        "mkfs.",
        "> /dev/sda",
        ":(){ :|:& };:",
        "chmod -R 777 /",
        "chown -R",
    ];

    for pattern in &dangerous_patterns {
        if command.contains(pattern) {
            return ToolExecResult {
                success: false,
                output: format!("Blocked dangerous pattern: {}", pattern),
                metadata: HashMap::new(),
            };
        }
    }

    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let exit_code = out.status.code().unwrap_or(-1);

            let mut metadata = HashMap::new();
            metadata.insert("exit_code".into(), json!(exit_code));
            metadata.insert("stdout_bytes".into(), json!(out.stdout.len()));
            metadata.insert("stderr_bytes".into(), json!(out.stderr.len()));

            let output = if stderr.is_empty() {
                stdout.to_string()
            } else {
                format!("{}\n\nSTDERR:\n{}", stdout, stderr)
            };

            // Truncate if too long
            let output = if output.len() > 50000 {
                format!("{}...\n[truncated, {} bytes total]", &output[..50000], output.len())
            } else {
                output
            };

            ToolExecResult {
                success: exit_code == 0,
                output,
                metadata,
            }
        }
        Err(e) => ToolExecResult {
            success: false,
            output: format!("Failed to execute command: {}", e),
            metadata: HashMap::new(),
        },
    }
}

fn exec_glob(args: &Value, cwd: &str) -> ToolExecResult {
    let pattern = args["pattern"].as_str().unwrap_or("");
    let base_path = args["path"].as_str().unwrap_or(cwd);

    let full_pattern = if pattern.starts_with('/') {
        pattern.to_string()
    } else {
        format!("{}/{}", base_path, pattern)
    };

    match glob::glob(&full_pattern) {
        Ok(paths) => {
            let matches: Vec<String> = paths
                .filter_map(|p| p.ok())
                .map(|p| p.to_string_lossy().to_string())
                .take(500) // Limit results
                .collect();

            let mut metadata = HashMap::new();
            metadata.insert("count".into(), json!(matches.len()));

            ToolExecResult {
                success: true,
                output: matches.join("\n"),
                metadata,
            }
        }
        Err(e) => ToolExecResult {
            success: false,
            output: format!("Glob error: {}", e),
            metadata: HashMap::new(),
        },
    }
}

fn exec_grep(args: &Value, cwd: &str) -> ToolExecResult {
    let pattern = args["pattern"].as_str().unwrap_or("");
    let path = args["path"].as_str().unwrap_or(".");
    let file_type = args["type"].as_str();

    let full_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", cwd, path)
    };

    let mut cmd_args = vec![
        "--line-number".to_string(),
        "--no-heading".to_string(),
        "--color=never".to_string(),
    ];

    if let Some(ft) = file_type {
        cmd_args.push(format!("--type={}", ft));
    }

    cmd_args.push(pattern.to_string());
    cmd_args.push(full_path);

    // Try ripgrep first, fall back to grep
    let output = Command::new("rg")
        .args(&cmd_args)
        .output()
        .or_else(|_| {
            Command::new("grep")
                .args(["-rn", pattern, &format!("{}/{}", cwd, path)])
                .output()
        });

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);

            // Truncate if too long
            let output = if stdout.len() > 30000 {
                format!(
                    "{}...\n[truncated, {} matches total]",
                    &stdout[..30000],
                    stdout.lines().count()
                )
            } else if stdout.is_empty() {
                "No matches found".to_string()
            } else {
                stdout.to_string()
            };

            let mut metadata = HashMap::new();
            metadata.insert("match_count".into(), json!(stdout.lines().count()));

            ToolExecResult {
                success: true,
                output,
                metadata,
            }
        }
        Err(e) => ToolExecResult {
            success: false,
            output: format!("Search error: {}", e),
            metadata: HashMap::new(),
        },
    }
}

async fn exec_web_fetch(args: &Value) -> ToolExecResult {
    let url = args["url"].as_str().unwrap_or("");
    let prompt = args["prompt"].as_str().unwrap_or("Extract the main content");

    if url.is_empty() {
        return ToolExecResult {
            success: false,
            output: "URL is required".into(),
            metadata: HashMap::new(),
        };
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    match client.get(url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                return ToolExecResult {
                    success: false,
                    output: format!("HTTP error: {}", response.status()),
                    metadata: HashMap::new(),
                };
            }

            match response.text().await {
                Ok(body) => {
                    // Basic HTML to text conversion
                    let text = html_to_text(&body);

                    // Truncate if needed
                    let output = if text.len() > 20000 {
                        format!("{}...\n[truncated]", &text[..20000])
                    } else {
                        text
                    };

                    let mut metadata = HashMap::new();
                    metadata.insert("url".into(), json!(url));
                    metadata.insert("bytes".into(), json!(body.len()));

                    ToolExecResult {
                        success: true,
                        output,
                        metadata,
                    }
                }
                Err(e) => ToolExecResult {
                    success: false,
                    output: format!("Failed to read response: {}", e),
                    metadata: HashMap::new(),
                },
            }
        }
        Err(e) => ToolExecResult {
            success: false,
            output: format!("Request failed: {}", e),
            metadata: HashMap::new(),
        },
    }
}

/// Basic HTML to text conversion
fn html_to_text(html: &str) -> String {
    // Remove script and style tags
    let mut text = html.to_string();

    // Remove script tags
    while let Some(start) = text.find("<script") {
        if let Some(end) = text[start..].find("</script>") {
            text = format!("{}{}", &text[..start], &text[start + end + 9..]);
        } else {
            break;
        }
    }

    // Remove style tags
    while let Some(start) = text.find("<style") {
        if let Some(end) = text[start..].find("</style>") {
            text = format!("{}{}", &text[..start], &text[start + end + 8..]);
        } else {
            break;
        }
    }

    // Remove all HTML tags
    let mut result = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            result.push(' ');
        } else if !in_tag {
            result.push(c);
        }
    }

    // Decode common HTML entities
    result = result
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse whitespace
    let mut collapsed = String::new();
    let mut last_was_space = false;
    for c in result.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                collapsed.push(' ');
                last_was_space = true;
            }
        } else {
            collapsed.push(c);
            last_was_space = false;
        }
    }

    collapsed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registry() {
        let registry = ToolRegistry::new();
        assert!(registry.get_tool("read").is_some());
        assert!(registry.get_tool("edit").is_some());
        assert!(registry.get_tool("bash").is_some());
    }

    #[test]
    fn test_html_to_text() {
        let html = "<html><head><script>alert('hi')</script></head><body><p>Hello World</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Hello World"));
        assert!(!text.contains("<script>"));
    }

    #[test]
    fn test_read_with_line_numbers() {
        // Would need a test file for full test
    }
}
