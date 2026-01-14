//! Ganesha Core - Execution Engine, Session Management, Safety

pub mod access_control;

pub use access_control::RiskLevel;

use crate::logging::SystemLogger;
use crate::providers::{LlmProvider, ChatMessage};
use access_control::{AccessController, AccessPolicy};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum GaneshaError {
    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("User cancelled")]
    UserCancelled,

    #[error("Timeout after {0} seconds")]
    Timeout(u64),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Action types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Shell,
    FileWrite,
    FileDelete,
    ServiceControl,
    PackageInstall,
    Response,  // Conversational response, no command execution
    Question,  // LLM wants to ask user a question with options
    McpTool,   // MCP tool call (command = "server:tool|{json_args}")
    Custom(String),
}

/// A question with multiple choice options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipleChoiceQuestion {
    pub question: String,
    pub options: Vec<String>,
    pub context: Option<String>,  // Additional context for the question
}

/// A planned action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub action_type: ActionType,
    pub command: String,
    pub explanation: String,
    pub risk_level: RiskLevel,
    pub reversible: bool,
    pub reverse_command: Option<String>,
    /// For Question action type - the question to ask
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<MultipleChoiceQuestion>,
}

/// Execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub id: String,
    pub task: String,
    pub actions: Vec<Action>,
    pub created_at: DateTime<Utc>,
}

impl ExecutionPlan {
    pub fn new(task: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task: task.into(),
            actions: vec![],
            created_at: Utc::now(),
        }
    }

    pub fn total_actions(&self) -> usize {
        self.actions.len()
    }

    pub fn high_risk_count(&self) -> usize {
        self.actions
            .iter()
            .filter(|a| matches!(a.risk_level, RiskLevel::High | RiskLevel::Critical))
            .count()
    }
}

/// Result of executing an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub action_id: String,
    pub command: String,
    pub explanation: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Session state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Pending,
    Planning,
    AwaitingConsent,
    Executing,
    Completed,
    Failed,
    RolledBack,
}

/// A Ganesha session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub task: String,
    pub state: SessionState,
    pub plan: Option<ExecutionPlan>,
    pub results: Vec<ExecutionResult>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Session {
    pub fn new(task: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task: task.into(),
            state: SessionState::Pending,
            plan: None,
            results: vec![],
            started_at: Utc::now(),
            completed_at: None,
        }
    }
}

/// Consent handler trait
pub trait ConsentHandler: Send + Sync {
    fn request_consent(&self, action: &Action) -> bool;
    fn request_batch_consent(&self, plan: &ExecutionPlan) -> ConsentResult;
}

#[derive(Debug, Clone)]
pub enum ConsentResult {
    ApproveAll,
    ApproveSingle,
    Deny,
    Cancel,
}

/// The Ganesha Engine
pub struct GaneshaEngine<L: LlmProvider, C: ConsentHandler> {
    pub llm: L,
    pub consent: C,
    pub access: AccessController,
    pub logger: SystemLogger,
    pub auto_approve: bool,
    pub session_dir: PathBuf,
    pub current_session: Option<Session>,
    /// Conversation history for multi-turn context
    pub conversation_history: Vec<ChatMessage>,
    /// Current working directory
    pub working_directory: PathBuf,
}

impl<L: LlmProvider, C: ConsentHandler> GaneshaEngine<L, C> {
    pub fn new(llm: L, consent: C, policy: AccessPolicy) -> Self {
        use directories::ProjectDirs;

        let session_dir = ProjectDirs::from("com", "gtechsd", "ganesha")
            .map(|p| p.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".ganesha"))
            .join("sessions");

        std::fs::create_dir_all(&session_dir).ok();

        let working_directory = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."));

        Self {
            llm,
            consent,
            access: AccessController::new(policy),
            logger: SystemLogger::new(),
            auto_approve: false,
            session_dir,
            current_session: None,
            conversation_history: Vec::new(),
            working_directory,
        }
    }

    /// Clear conversation history (for new session)
    pub fn clear_history(&mut self) {
        self.conversation_history.clear();
    }

    /// Get summary of conversation for /recall command
    pub fn get_conversation_summary(&self) -> String {
        if self.conversation_history.is_empty() {
            return "No conversation history yet.".to_string();
        }

        let mut summary = format!("Conversation history ({} messages):\n", self.conversation_history.len());
        for (i, msg) in self.conversation_history.iter().enumerate() {
            if msg.role != "system" {
                let preview = if msg.content.len() > 100 {
                    format!("{}...", &msg.content[..100])
                } else {
                    msg.content.clone()
                };
                summary.push_str(&format!("  {}. [{}]: {}\n", i + 1, msg.role.to_uppercase(), preview));
            }
        }
        summary
    }

    /// Plan execution for a task
    pub async fn plan(&mut self, task: &str) -> Result<ExecutionPlan, GaneshaError> {
        // Check for manipulation
        if let Some(indicator) = self.access.check_manipulation(task) {
            self.logger.manipulation_detected("user", task, &indicator);
            return Err(GaneshaError::AccessDenied(format!(
                "Manipulation detected: {}",
                indicator
            )));
        }

        let mut session = Session::new(task);
        session.state = SessionState::Planning;
        self.current_session = Some(session);

        // Check if this is a large list request that needs chunked generation
        if let Some((count, item_type)) = Self::detect_large_list_request(task) {
            return self.plan_chunked_list(task, count, &item_type).await;
        }

        // Auto-connect MCP servers based on task content
        self.auto_connect_mcp_if_needed(task);

        // Build messages with conversation history
        let system_prompt = self.build_planning_prompt();

        // Debug: Check if MCP tools are in prompt (only in debug mode)
        if std::env::var("GANESHA_DEBUG").is_ok() {
            if system_prompt.contains("MCP TOOLS AVAILABLE") {
                eprintln!("[MCP] Tools included in prompt");
            }
        }

        // Build message list: system + history + current user message
        let mut messages = vec![ChatMessage::system(&system_prompt)];

        // Add conversation history (keeps context between turns)
        for msg in &self.conversation_history {
            messages.push(msg.clone());
        }

        // Add current user message
        messages.push(ChatMessage::user(task));

        // Generate with full conversation context
        let response = self
            .llm
            .generate_with_history(&messages)
            .await
            .map_err(|e| GaneshaError::LlmError(e.to_string()))?;

        // Debug: show raw LLM response
        if std::env::var("GANESHA_DEBUG").is_ok() {
            eprintln!("[DEBUG] Raw LLM response ({} chars): {}", response.len(), &response[..std::cmp::min(500, response.len())]);
        }

        // Add to conversation history for future context
        self.conversation_history.push(ChatMessage::user(task));
        self.conversation_history.push(ChatMessage::assistant(&response));

        // Trim history if it gets too long (keep last 20 turns = 40 messages)
        if self.conversation_history.len() > 40 {
            self.conversation_history.drain(0..2);
        }

        // Parse plan from response
        let mut plan = ExecutionPlan::new(task);
        plan.actions = self.parse_actions(&response)?;

        // Post-processing: Override shell commands for website tasks with MCP browser actions
        // This handles the case where LLM uses container.exec/python/curl instead of MCP tools
        // Also handles the case where LLM just outputs a URL as a Response
        if Self::is_website_task(task) && Self::has_browser_mcp() {
            let url = Self::extract_url_from_task(task);
            let has_shell_action = plan.actions.iter().any(|a| matches!(a.action_type, ActionType::Shell));

            // Check if LLM returned just a URL as a Response (not using MCP tools)
            let has_url_response = plan.actions.iter().any(|a| {
                matches!(a.action_type, ActionType::Response) && Self::is_url_response(&a.explanation)
            });

            // Override if LLM used shell command OR just returned URL text
            if (has_shell_action || has_url_response) && !url.is_empty() {
                // Replace the plan with MCP browser actions
                plan.actions = vec![
                    Action {
                        id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
                        action_type: ActionType::McpTool,
                        command: format!("playwright:browser_navigate|{{\"url\":\"{}\"}}", url),
                        explanation: format!("Navigate to {}", url),
                        risk_level: RiskLevel::Low,
                        reversible: false,
                        reverse_command: None,
                        question: None,
                    },
                    Action {
                        id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
                        action_type: ActionType::McpTool,
                        command: "playwright:browser_snapshot|{}".to_string(),
                        explanation: "Get page content".to_string(),
                        risk_level: RiskLevel::Low,
                        reversible: false,
                        reverse_command: None,
                        question: None,
                    },
                ];
            }
        }

        // Validate each action against access control (skip Response and McpTool actions)
        for action in &mut plan.actions {
            // Response actions don't need access control - they're just text
            // McpTool actions are sandboxed by the MCP server - no shell access control needed
            if matches!(action.action_type, ActionType::Response | ActionType::McpTool) {
                continue;
            }

            // In auto mode (-A), allow most commands but still block truly dangerous ones
            if self.auto_approve {
                // Only block critical dangers in auto mode
                if self.access.is_critical_danger(&action.command) {
                    self.logger
                        .command_denied("user", &action.command, "Critical danger blocked");
                    return Err(GaneshaError::AccessDenied(
                        "Command blocked for safety (even in auto mode)".into()
                    ));
                }
                action.risk_level = self.access.assess_risk_only(&action.command);
            } else {
                let check = self.access.check_command(&action.command);
                action.risk_level = check.risk_level;

                if !check.allowed {
                    self.logger
                        .command_denied("user", &action.command, &check.reason);
                    return Err(GaneshaError::AccessDenied(check.reason));
                }
            }
        }

        if let Some(ref mut session) = self.current_session {
            session.plan = Some(plan.clone());
            session.state = SessionState::AwaitingConsent;
        }

        Ok(plan)
    }

    /// Execute a plan
    pub async fn execute(&mut self, plan: &ExecutionPlan) -> Result<Vec<ExecutionResult>, GaneshaError> {
        let mut results = vec![];

        // Check if this is a response-only plan (no commands to execute)
        let has_commands = plan.actions.iter().any(|a| !matches!(a.action_type, ActionType::Response));

        // Get consent only if there are actual commands to run
        if !self.auto_approve && has_commands {
            match self.consent.request_batch_consent(plan) {
                ConsentResult::Cancel | ConsentResult::Deny => {
                    if let Some(ref mut session) = self.current_session {
                        session.state = SessionState::Failed;
                    }
                    return Err(GaneshaError::UserCancelled);
                }
                _ => {}
            }
        }

        if let Some(ref mut session) = self.current_session {
            session.state = SessionState::Executing;
        }

        // Execute each action
        for action in &plan.actions {
            let start = std::time::Instant::now();

            // Handle Response actions - just return the text, no execution
            if matches!(action.action_type, ActionType::Response) {
                results.push(ExecutionResult {
                    action_id: action.id.clone(),
                    command: String::new(),
                    explanation: action.explanation.clone(),
                    success: true,
                    output: action.explanation.clone(),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
                continue;
            }

            // Handle MCP tool actions
            if matches!(action.action_type, ActionType::McpTool) {
                use crate::orchestrator::mcp::call_mcp_tool;

                // Parse command format: "server:tool|{json_args}"
                let (tool_part, args_json) = action.command.split_once('|')
                    .unwrap_or((&action.command, "{}"));

                let (server, tool) = match tool_part.split_once(':') {
                    Some((s, t)) => (s, t),
                    None => {
                        results.push(ExecutionResult {
                            action_id: action.id.clone(),
                            command: action.command.clone(),
                            explanation: action.explanation.clone(),
                            success: false,
                            output: String::new(),
                            error: Some("Invalid MCP tool format".into()),
                            duration_ms: start.elapsed().as_millis() as u64,
                        });
                        continue;
                    }
                };

                let args: serde_json::Value = serde_json::from_str(args_json)
                    .unwrap_or(serde_json::json!({}));

                // Handle built-in "ganesha:" tools
                if server == "ganesha" {
                    let result: Result<String, String> = match tool {
                        "web_search" => {
                            // Try multiple ways to get the query (LLMs format it differently)
                            let query = args.get("query")
                                .and_then(|q| q.as_str())
                                .or_else(|| args.get("q").and_then(|q| q.as_str()))
                                .or_else(|| args.get("search").and_then(|q| q.as_str()))
                                .unwrap_or("");

                            if query.is_empty() {
                                Err(format!("Empty search query. Args received: {}", args))
                            } else {
                                let max_results = args.get("max_results").and_then(|m| m.as_u64()).unwrap_or(10) as usize;
                                match crate::websearch::search(query, max_results).await {
                                    Ok(response) => {
                                        let output = crate::websearch::format_results(&response);
                                        Ok(output)
                                    }
                                    Err(e) => Err(e)
                                }
                            }
                        }
                        "exec" | "execute" | "shell" | "run" | "cmd" => {
                            // Model hallucinated this tool - redirect to show helpful error
                            // Commands should use the "command" field, not mcp_tool
                            let cmd = args.get("command")
                                .or_else(|| args.get("cmd"))
                                .or_else(|| args.get("script"))
                                .and_then(|c| c.as_str())
                                .unwrap_or("");

                            if cmd.is_empty() {
                                Err("Shell commands must use the 'command' field, NOT mcp_tool. Correct format: {\"actions\":[{\"command\":\"pwd\",\"explanation\":\"Show directory\"}]}".to_string())
                            } else {
                                Err(format!("Shell commands must use 'command' field: {{\"actions\":[{{\"command\":\"{}\",\"explanation\":\"Execute\"}}]}}", cmd))
                            }
                        }
                        _ => Err(format!("Unknown ganesha tool: {}. Available: web_search", tool))
                    };

                    match result {
                        Ok(output) => {
                            results.push(ExecutionResult {
                                action_id: action.id.clone(),
                                command: format!("ganesha:{}", tool),
                                explanation: action.explanation.clone(),
                                success: true,
                                output,
                                error: None,
                                duration_ms: start.elapsed().as_millis() as u64,
                            });
                        }
                        Err(e) => {
                            results.push(ExecutionResult {
                                action_id: action.id.clone(),
                                command: format!("ganesha:{}", tool),
                                explanation: action.explanation.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!("Ganesha tool error: {}", e)),
                                duration_ms: start.elapsed().as_millis() as u64,
                            });
                        }
                    }
                    continue;
                }

                match call_mcp_tool(server, tool, args) {
                    Ok(result) => {
                        // Extract text content from MCP response
                        // Format: {"content":[{"text":"...","type":"text"}]}
                        let output = if let Some(content) = result.get("content") {
                            if let Some(arr) = content.as_array() {
                                arr.iter()
                                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            } else {
                                result.to_string()
                            }
                        } else {
                            // Fallback to string representation
                            result.to_string()
                        };
                        results.push(ExecutionResult {
                            action_id: action.id.clone(),
                            command: format!("{}:{}", server, tool),
                            explanation: action.explanation.clone(),
                            success: true,
                            output,
                            error: None,
                            duration_ms: start.elapsed().as_millis() as u64,
                        });
                    }
                    Err(e) => {
                        results.push(ExecutionResult {
                            action_id: action.id.clone(),
                            command: format!("{}:{}", server, tool),
                            explanation: action.explanation.clone(),
                            success: false,
                            output: String::new(),
                            error: Some(format!("MCP error: {}", e)),
                            duration_ms: start.elapsed().as_millis() as u64,
                        });
                    }
                }
                continue;
            }

            // Final access check (skip for auto mode, except critical dangers)
            if self.auto_approve {
                if self.access.is_critical_danger(&action.command) {
                    self.logger
                        .command_denied("user", &action.command, "Critical danger blocked");
                    results.push(ExecutionResult {
                        action_id: action.id.clone(),
                        command: action.command.clone(),
                        explanation: action.explanation.clone(),
                        success: false,
                        output: String::new(),
                        error: Some("Command blocked for safety".into()),
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                    continue;
                }
            } else {
                let check = self.access.check_command(&action.command);
                if !check.allowed {
                    self.logger
                        .command_denied("user", &action.command, &check.reason);
                    results.push(ExecutionResult {
                        action_id: action.id.clone(),
                        command: action.command.clone(),
                        explanation: action.explanation.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(check.reason),
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                    continue;
                }
            }

            // Execute
            let result = self.execute_command(&action.command).await;
            let duration_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(output) => {
                    self.logger.command_executed(
                        "user",
                        &action.command,
                        &action.risk_level.to_string(),
                        self.current_session
                            .as_ref()
                            .map(|s| s.id.as_str())
                            .unwrap_or(""),
                    );
                    results.push(ExecutionResult {
                        action_id: action.id.clone(),
                        command: action.command.clone(),
                        explanation: action.explanation.clone(),
                        success: true,
                        output,
                        error: None,
                        duration_ms,
                    });
                }
                Err(e) => {
                    results.push(ExecutionResult {
                        action_id: action.id.clone(),
                        command: action.command.clone(),
                        explanation: action.explanation.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e.to_string()),
                        duration_ms,
                    });
                }
            }
        }

        if let Some(ref mut session) = self.current_session {
            session.results = results.clone();
            session.state = if results.iter().all(|r| r.success) {
                SessionState::Completed
            } else {
                SessionState::Failed
            };
            session.completed_at = Some(Utc::now());
        }

        // Save session (separate borrow scope)
        if let Some(ref session) = self.current_session {
            self.save_session(session)?;
        }

        Ok(results)
    }

    /// Analyze execution results and generate a response
    /// Returns (summary, optional_next_actions)
    pub async fn analyze_results(
        &mut self,
        task: &str,
        results: &[ExecutionResult],
    ) -> Result<(String, Option<ExecutionPlan>), GaneshaError> {
        use crate::providers::ChatMessage;

        // Build context from results
        let mut result_summary = String::new();
        for result in results {
            if result.command.is_empty() {
                // Response action - already handled
                continue;
            }
            // For browser/MCP content, allow more output so LLM can analyze full page
            let max_output = if result.command.contains("browser") || result.command.contains("playwright") {
                30000  // 30K for browser snapshots
            } else {
                8000   // 8K for regular commands
            };

            result_summary.push_str(&format!(
                "Command: {}\nStatus: {}\nOutput:\n{}\n\n",
                result.command,
                if result.success { "SUCCESS" } else { "FAILED" },
                if result.output.len() > max_output {
                    format!("{}...(truncated)", &result.output[..max_output])
                } else {
                    result.output.clone()
                }
            ));
            if let Some(ref err) = result.error {
                result_summary.push_str(&format!("Error: {}\n", err));
            }
        }

        // If no commands were run (response-only), skip analysis
        if result_summary.is_empty() {
            return Ok((String::new(), None));
        }

        // Check if this was a browser/MCP task - provide appropriate prompt
        let has_browser_output = result_summary.contains("Page Snapshot") ||
                                  result_summary.contains("browser_navigate") ||
                                  result_summary.contains("Page URL:");

        // Get current date/time for context
        let now = chrono::Local::now();
        let date_str = now.format("%B %d, %Y at %H:%M").to_string();

        let system_prompt = if has_browser_output {
            format!(
                r#"You are Ganesha. Current date: {}.
User question: "{}"

Page data:
{}

RESPOND WITH JSON: {{"response":"your complete answer"}}

RULES:
1. DEDUPLICATE - List each unique item only ONCE (no duplicates!)
2. COMPLETE - Finish all sentences, never cut off mid-word
3. CONCISE - One line per item, no verbose descriptions
4. ALL ITEMS - List every unique item found on the page

FORMAT FOR LISTS:
{{"response":"Found on the website:\n- Item A\n- Item B\n- Item C"}}

BAD (duplicates): "- 2026 RAV4\n- 2026 RAV4 (hybrid)"
GOOD (unique only): "- 2026 RAV4\n- 2026 RAV4 Hybrid"

BAD (cut off): "- 2026 RAV4 (listed again in the"
GOOD (complete): "- 2026 RAV4""#,
                date_str, task, result_summary
            )
        } else {
            format!(
                r#"You are Ganesha, an autonomous AI assistant. The user asked: "{}"

Commands executed:
{}

YOUR JOB: Interpret results and either RESPOND or CONTINUE with more actions.

RESPONSE FORMAT (valid JSON only, no prefixes):
1. Task complete: {{"response":"<clear interpretation>"}}
2. Need more exploration: {{"actions":[{{"command":"cmd","explanation":"why"}}]}}

FOR ANALYSIS/EXPLORATION TASKS:
- If you only read partial info, continue reading more files
- Don't say "I can only see..." - instead, read more files
- Keep going until you have enough info to give a complete answer
- Example: if asked to analyze code and only saw file list, read the actual files

CRITICAL CHECKS:
- User asked "is X running?" → Check if X appears in output. If NOT → say "No, X is not running"
- User asked to analyze code → Did you read enough files? If not → read more

ERROR RECOVERY:
- "container name in use" → remove old container and retry
- "permission denied" → suggest sudo
- "not found" → suggest installation
- NEVER just stop on errors

INTERPRETATION:
- Give CLEAR, DIRECT answers
- Use EXACT values from output
- For code analysis: summarize what you learned, mention key files/patterns

EXAMPLES:
- Partial exploration → {{"actions":[{{"command":"cat src/main.rs","explanation":"Read main entry point"}}]}}
- Complete analysis → {{"response":"The codebase is organized into X modules. Key files are..."}}
- Error recovery → {{"actions":[{{"command":"docker rm -f X","explanation":"Remove conflict"}},{{"command":"docker run...","explanation":"Retry"}}]}}"#,
                task, result_summary
            )
        };

        // Build messages for LLM
        let user_msg = format!("Analyze the results and respond to: {}", task);
        let messages = vec![
            ChatMessage::system(&system_prompt),
            ChatMessage::user(&user_msg),
        ];

        let response = self.llm.generate_with_history(&messages).await
            .map_err(|e| GaneshaError::LlmError(e.to_string()))?;

        // Clean up LLM control tokens
        let cleaned = Self::strip_control_tokens(&response);
        let sanitized = Self::sanitize_json_string(&cleaned);

        // Try to parse as response
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&sanitized) {
            // Check for response (task complete) - try multiple keys
            let response_text = parsed.get("response").and_then(|v| v.as_str())
                .or_else(|| parsed.get("").and_then(|v| v.as_str()))  // Handle {"":"text"}
                .or_else(|| parsed.get("answer").and_then(|v| v.as_str()))
                .or_else(|| parsed.get("result").and_then(|v| v.as_str()));

            if let Some(response_text) = response_text {
                if !response_text.is_empty() {
                    self.conversation_history.push(ChatMessage::assistant(response_text));
                    return Ok((response_text.to_string(), None));
                }
            }

            // Check for more actions needed
            if let Some(actions) = parsed.get("actions").and_then(|v| v.as_array()) {
                if !actions.is_empty() {
                    let mut plan = ExecutionPlan::new(task);
                    for action_val in actions {
                        if let (Some(cmd), Some(expl)) = (
                            action_val.get("command").and_then(|v| v.as_str()),
                            action_val.get("explanation").and_then(|v| v.as_str()),
                        ) {
                            plan.actions.push(Action {
                                id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
                                action_type: ActionType::Shell,
                                command: cmd.to_string(),
                                explanation: expl.to_string(),
                                risk_level: self.access.assess_risk_only(cmd),
                                reversible: false,
                                reverse_command: None,
                                question: None,
                            });
                        }
                    }
                    if !plan.actions.is_empty() {
                        return Ok((String::new(), Some(plan)));
                    }
                }
            }

            // If parsed but no known keys, try to extract any string value
            if let Some(obj) = parsed.as_object() {
                for (_, value) in obj {
                    if let Some(text) = value.as_str() {
                        if !text.is_empty() && text.len() > 10 {
                            self.conversation_history.push(ChatMessage::assistant(text));
                            return Ok((text.to_string(), None));
                        }
                    }
                }
            }
        }

        // Fallback - try to extract text from JSON-like response
        let cleaned_trimmed = cleaned.trim();
        if !cleaned_trimmed.is_empty() {
            // If it looks like JSON but parsing failed, try to extract text content
            if cleaned_trimmed.contains("\":\"") {
                // Try to extract value from {"key":"value"} pattern
                if let Ok(re) = regex::Regex::new(r#""[^"]*"\s*:\s*"([^"]+)""#) {
                    if let Some(caps) = re.captures(cleaned_trimmed) {
                        if let Some(text) = caps.get(1) {
                            let extracted = text.as_str().to_string();
                            if !extracted.is_empty() {
                                return Ok((extracted, None));
                            }
                        }
                    }
                }
            }

            // If not JSON-looking, return as plain text
            if !cleaned_trimmed.starts_with('{') && !cleaned_trimmed.starts_with('[') {
                return Ok((cleaned_trimmed.to_string(), None));
            }
        }

        // Last resort - return empty (execution output was already shown)
        Ok((String::new(), None))
    }

    async fn execute_command(&self, command: &str) -> Result<String, GaneshaError> {
        use tokio::process::Command;

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", command])
                .current_dir(&self.working_directory)
                .output()
                .await?
        } else {
            Command::new("sh")
                .args(["-c", command])
                .current_dir(&self.working_directory)
                .output()
                .await?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // For informational commands, non-zero exit is still a valid result
        // e.g., `which foo` returns 1 if not found, but that's an answer not an error
        let is_info_command = command.starts_with("which ")
            || command.starts_with("type ")
            || command.starts_with("command -v ")
            || command.contains("--version")
            || command.contains("-v ")
            || command.starts_with("dpkg -l")
            || command.starts_with("rpm -q")
            || command.starts_with("apt list")
            || command.starts_with("systemctl status")
            || command.starts_with("service ")
            || command.contains("| grep")
            // find often has permission errors but still returns useful results
            || command.starts_with("find ")
            // git diff-index returns 1 when there are changes (not an error)
            || command.starts_with("git diff")
            || command.starts_with("git status")
            // test/[ commands return 1 for false, not an error
            || command.starts_with("test ")
            || command.starts_with("[ ")
            // diff returns 1 when files differ (expected behavior)
            || command.starts_with("diff ")
            // grep returns 1 when no matches (not an error)
            || command.starts_with("grep ");

        if output.status.success() {
            Ok(stdout)
        } else if is_info_command {
            // For info commands, return stdout even on non-zero exit
            // If stdout is empty, return a message instead of failing
            if stdout.trim().is_empty() && stderr.trim().is_empty() {
                Ok("Not found / not installed".to_string())
            } else if !stdout.trim().is_empty() {
                Ok(stdout)
            } else {
                Ok(stderr) // Some commands write to stderr
            }
        } else {
            Err(GaneshaError::ExecutionFailed(
                if stderr.is_empty() { stdout } else { stderr }
            ))
        }
    }

    fn build_planning_prompt(&self) -> String {
        let auto_mode = if self.auto_approve {
            "\nAUTO MODE ENABLED: User wants actions, not explanations. Run commands to find answers."
        } else {
            ""
        };

        // Get available MCP tools
        let mcp_section = self.build_mcp_tools_prompt();
        if std::env::var("GANESHA_DEBUG").is_ok() {
            if mcp_section.is_empty() {
                eprintln!("[DEBUG] MCP: No tools in prompt (none connected)");
            } else {
                eprintln!("[DEBUG] MCP: {} chars of tools in prompt", mcp_section.len());
                eprintln!("[DEBUG] MCP section preview: {}", &mcp_section[..mcp_section.len().min(200)]);
            }
        }

        let now = chrono::Local::now();
        let date_context = now.format("Current date: %B %d, %Y (%H:%M)").to_string();

        format!(r#"You are Ganesha, an autonomous AI system assistant. {}
Working directory: {}{}{}

OUTPUT FORMAT - MANDATORY JSON:
Shell commands (ls, pwd, cat, apt, etc.): {{"actions":[{{"command":"ls -la","explanation":"list files"}}]}}
MCP tools (web search, browser): {{"actions":[{{"mcp_tool":"ganesha:web_search","mcp_args":{{"query":"search term"}},"explanation":"search"}}]}}
Simple answers: {{"response":"brief answer"}}
Need clarification: {{"question":"What do you want?","options":["Option A","Option B","Option C"]}}

IMPORTANT - Use the correct field:
- "command" field = ALL shell commands (ls, pwd, cat, grep, apt, systemctl, etc.)
- "mcp_tool" field = ONLY for MCP server tools (ganesha:web_search, playwright:*, fetch:*)
- NEVER use mcp_tool for shell commands like pwd, ls, cat - use "command" instead!

CRITICAL: COMPLETE ALL STEPS IN ONE RESPONSE
- Generate ALL commands needed to FULLY complete the task
- Do NOT stop after one command - include ALL necessary steps
- Example: "install X and configure Y" = update + install + mkdir + edit config + restart service
- Chain related operations: apt update && apt install, mkdir && chown, etc.
- ALWAYS verify your work with a final check command

BEHAVIOR RULES:
- BE AUTONOMOUS: Don't tell the user to do things - DO them yourself
- NEVER say "you can..." - YOU do it directly
- For installations: apt update first, then install, then configure, then verify
- For configurations: create directories, edit files, set permissions, restart services

INSTALLATION TASKS (like "install apache/nginx/docker"):
Generate ALL steps in one response:
1. Update package lists: sudo apt-get update
2. Install package: sudo apt-get install -y <package>
3. Create any requested directories: sudo mkdir -p /path
4. Configure if needed: edit config files
5. Set permissions: sudo chown/chmod
6. Enable/restart service: sudo systemctl enable --now <service>
7. Verify: systemctl status or curl localhost

CONFIG TASKS (like "set document root to X"):
1. Create directory: sudo mkdir -p /path/to/dir
2. Set ownership: sudo chown -R www-data:www-data /path
3. Edit config: use sed or echo to modify config file
4. Restart service: sudo systemctl restart <service>

CODE ANALYSIS TASKS:
- Read multiple files to get full picture
- Use: cat, find, head, grep to explore
- Keep exploring until you have enough context

WEB SEARCH - USE SELECTIVELY:

Use ganesha:web_search ONLY when user explicitly asks to search the web or needs current/external information.
DO NOT use web search for: greetings, basic knowledge, system commands, or questions you can answer directly.

WHEN TO SEARCH (explicit web requests):
- "search for X" / "look up X online" / "find X on the web" → use ganesha:web_search
- "what's the latest news about X" → use ganesha:web_search
- "go to X website" → use ganesha:web_search to find URL first

WHEN NOT TO SEARCH (answer directly instead):
- "hello" / "hi" → respond with greeting
- "what is air" / "what is water" → explain using your knowledge
- "how do I install X" → provide commands directly
- "list files" / "show disk usage" → execute system commands

Search format:
{{"actions":[{{"mcp_tool":"ganesha:web_search","mcp_args":{{"query":"your search terms","max_results":10}},"explanation":"Search the web"}}]}}

FETCH (only for KNOWN URLs provided by user or from search results):
{{"actions":[{{"mcp_tool":"fetch:fetch","mcp_args":{{"url":"https://exact-url.com"}},"explanation":"Get page"}}]}}

BROWSER (only for interactive tasks: clicking, filling forms, JavaScript sites):
{{"actions":[{{"mcp_tool":"playwright:browser_navigate","mcp_args":{{"url":"URL"}},"explanation":"Browse"}}]}}

EXAMPLES:
- "install apache and set doc root to /home/user/WWW" → {{"actions":[
    {{"command":"sudo apt-get update && sudo apt-get install -y apache2","explanation":"Install Apache"}},
    {{"command":"sudo mkdir -p /home/user/WWW && sudo chown -R www-data:www-data /home/user/WWW","explanation":"Create doc root"}},
    {{"command":"sudo sed -i 's|DocumentRoot /var/www/html|DocumentRoot /home/user/WWW|' /etc/apache2/sites-available/000-default.conf","explanation":"Update config"}},
    {{"command":"sudo systemctl restart apache2","explanation":"Apply changes"}},
    {{"command":"systemctl status apache2 | head -5","explanation":"Verify"}}
  ]}}
- "what time is it" → {{"response":"It's currently [time]"}}
- "is nginx running" → {{"actions":[{{"command":"systemctl status nginx | grep Active","explanation":"Check status"}}]}}"#, date_context, self.working_directory.display(), auto_mode, mcp_section)
    }

    /// Build MCP tools section for prompt (if any MCP servers are connected)
    fn build_mcp_tools_prompt(&self) -> String {
        use crate::orchestrator::mcp::get_all_mcp_tools;

        let mcp_tools = get_all_mcp_tools();
        if mcp_tools.is_empty() {
            return String::new();
        }

        let mut section = String::from("\n\nMCP TOOLS AVAILABLE (use mcp_tool/mcp_args format):\n");

        for (server, tools) in mcp_tools {
            section.push_str(&format!("{}:\n", server));
            for tool in tools.iter().take(8) {
                section.push_str(&format!("  - {}:{} - {}\n",
                    server, tool.name,
                    tool.description.as_deref().unwrap_or("").chars().take(60).collect::<String>()
                ));
            }
            if tools.len() > 8 {
                section.push_str(&format!("  ... +{} more\n", tools.len() - 8));
            }
        }

        // Remind about web search but not too aggressively
        section.push_str("\nWEB SEARCH: Use ganesha:web_search only for explicit web/search requests.\n");
        section.push_str("- \"search for X\" or \"look up X online\" → use ganesha:web_search\n");
        section.push_str("- Greetings, basic questions, commands → respond directly, no search needed.\n");

        // Add dynamic examples based on connected server
        section.push_str("\nBROWSER EXAMPLES (only for navigating to KNOWN URLs):\n");

        // Find tools for examples
        let mcp_tools = get_all_mcp_tools();
        for (server, tools) in &mcp_tools {
            let mut has_navigate = false;
            let mut has_snapshot = false;

            for tool in tools {
                if !has_navigate && tool.name.contains("navigate") && !tool.name.contains("back") {
                    section.push_str(&format!(
                        "- \"see google.com\" → {{\"actions\":[{{\"mcp_tool\":\"{}:{}\",\"mcp_args\":{{\"url\":\"https://google.com\"}},\"explanation\":\"Navigate\"}}]}}\n",
                        server, tool.name
                    ));
                    has_navigate = true;
                }
                if !has_snapshot && tool.name.contains("snapshot") {
                    section.push_str(&format!(
                        "- \"what's on the page\" → {{\"actions\":[{{\"mcp_tool\":\"{}:{}\",\"mcp_args\":{{}},\"explanation\":\"Get page content\"}}]}}\n",
                        server, tool.name
                    ));
                    has_snapshot = true;
                }
            }
        }

        section
    }

    /// Auto-connect MCP servers based on task content
    /// Detects if task needs browser/web capabilities and connects playwright if not already connected
    fn auto_connect_mcp_if_needed(&self, task: &str) {
        use crate::orchestrator::mcp::{get_all_mcp_tools, connect_mcp_server_verbose, McpManager};

        let task_lower = task.to_lowercase();

        // Check if task needs browser capabilities
        let needs_browser = task_lower.contains("website")
            || task_lower.contains("webpage")
            || task_lower.contains("web page")
            || task_lower.contains("browse")
            || task_lower.contains("browser")
            || task_lower.contains("navigate to")
            || task_lower.contains("go to http")
            || task_lower.contains("go to www")
            || task_lower.contains("open http")
            || task_lower.contains("open www")
            || task_lower.contains("visit http")
            || task_lower.contains("visit www")
            || task_lower.contains(".com")
            || task_lower.contains(".org")
            || task_lower.contains(".net")
            || task_lower.contains(".io")
            || task_lower.contains("what's on")
            || task_lower.contains("whats on")
            || task_lower.contains("can you see")
            || task_lower.contains("look at")
            || regex::Regex::new(r"https?://").map(|re| re.is_match(&task_lower)).unwrap_or(false);

        if !needs_browser {
            return;
        }

        // Check if any browser MCP is already connected
        let connected = get_all_mcp_tools();
        let has_browser = connected.iter().any(|(name, _)| {
            name.contains("playwright") || name.contains("browser") || name.contains("puppeteer")
        });

        if has_browser {
            return; // Already have a browser server connected
        }

        // Auto-connect playwright quietly
        let manager = McpManager::new();

        // Try regular playwright first (matches prompt examples), then playwright-ea
        let server = manager.get_server("playwright")
            .or_else(|| manager.get_server("playwright-ea"));

        if let Some(server) = server {
            // Use quiet mode - just show a brief message
            eprintln!("🌐 Connecting browser...");
            match connect_mcp_server_verbose(server, false) {
                Ok(_) => {
                    eprintln!("✓ Browser ready ({})", server.name);
                }
                Err(e) => {
                    eprintln!("⚠ Browser connection failed: {}", e);
                }
            }
        }

        // Also auto-connect context7 for documentation/library questions
        let needs_docs = task_lower.contains("documentation")
            || task_lower.contains(" docs")
            || task_lower.contains("library")
            || task_lower.contains("api reference")
            || task_lower.contains("how to use")
            || task_lower.contains("code example")
            || (task_lower.contains("how do") && (task_lower.contains("react") || task_lower.contains("node") || task_lower.contains("python") || task_lower.contains("rust")));

        if needs_docs {
            let has_context7 = connected.iter().any(|(name, _)| name == "context7");
            if !has_context7 {
                if let Some(server) = manager.get_server("context7") {
                    eprintln!("📚 Connecting documentation...");
                    match connect_mcp_server_verbose(server, false) {
                        Ok(_) => eprintln!("✓ Documentation ready (context7)"),
                        Err(e) => eprintln!("⚠ Documentation connection failed: {}", e),
                    }
                }
            }
        }
    }

    /// Detect if a task requests a large list (>20 items)
    /// Returns Some((count, item_description)) if detected
    fn detect_large_list_request(task: &str) -> Option<(u32, String)> {
        // Pattern: number followed by optional adjectives, then item words
        // E.g., "100 gargoyle facts", "50 funny jokes", "25 random facts"
        let re = regex::Regex::new(
            r"(?i)(\d{2,})\s+(?:\w+\s+)?(facts?|items?|things?|entries?|elements?|records?|rows?|lines?|examples?|quotes?|jokes?|tips?|ideas?|suggestions?|names?|words?|sentences?)"
        ).ok()?;

        if let Some(caps) = re.captures(task) {
            let count: u32 = caps.get(1)?.as_str().parse().ok()?;
            let item_type = caps.get(2)?.as_str().to_lowercase();

            // Trigger chunking for lists with 20+ items (LLMs struggle with large lists)
            if count >= 20 {
                // Extract the full context (e.g., "gargoyle facts" not just "facts")
                let full_match = caps.get(0)?.as_str();
                let item_desc = full_match
                    .strip_prefix(&format!("{} ", count))
                    .unwrap_or(&item_type)
                    .to_string();
                return Some((count, item_desc));
            }
        }
        None
    }

    /// Build a prompt for generating a specific chunk of items
    fn build_chunk_prompt(item_type: &str, start: u32, end: u32, context: &str) -> String {
        format!(
            r#"Generate items {start} through {end} for this request: {context}

CRITICAL INSTRUCTIONS:
- Output ONLY items numbered {start} to {end}, one per line
- Format: "N. [content]" where N is the number
- Each item MUST contain REAL, FACTUAL, INTERESTING content
- NO placeholders like "Lorem ipsum" or "Fact N about X"
- NO shortcuts like "continue pattern" or "same as above"
- Each item must be UNIQUE and SPECIFIC - real information
- No introductions, no commentary - JUST the numbered items
- Start with "{start}." and end with "{end}."

Example of GOOD output:
1. Gargoyles were first used in ancient Egypt to drain water from flat roofs.
2. The word "gargoyle" comes from the French "gargouille" meaning throat.

Example of BAD output (DO NOT DO THIS):
1. Fact 1 about gargoyles.
2. Lorem ipsum dolor sit amet.

BEGIN OUTPUT:"#,
            start = start,
            end = end,
            context = context
        )
    }

    /// Generate a large list using chunked requests
    pub async fn generate_chunked_list(
        &mut self,
        total_count: u32,
        item_type: &str,
        original_task: &str,
    ) -> Result<Vec<String>, GaneshaError> {
        let chunk_size = 150; // Items per chunk - safe for most models
        let mut all_items: Vec<String> = Vec::with_capacity(total_count as usize);
        let mut current = 1u32;

        // Extract context (remove the count from task for cleaner prompts)
        let context = original_task.to_string();

        while current <= total_count {
            let end = std::cmp::min(current + chunk_size - 1, total_count);

            // Build chunk-specific prompt
            let chunk_prompt = Self::build_chunk_prompt(item_type, current, end, &context);

            // Build messages for this chunk
            let system = format!(
                "You are a content generator. Generate exactly the items requested, numbered sequentially. \
                 No placeholders, no shortcuts. Output real, unique content for each item."
            );

            let messages = vec![
                ChatMessage::system(&system),
                ChatMessage::user(&chunk_prompt),
            ];

            // Generate chunk
            let response = self
                .llm
                .generate_with_history(&messages)
                .await
                .map_err(|e| GaneshaError::LlmError(format!("Chunk {}-{}: {}", current, end, e)))?;

            // Parse items from response
            let chunk_items = Self::parse_numbered_items(&response, current, end);
            all_items.extend(chunk_items);

            current = end + 1;

            // Small delay between chunks to avoid rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        all_items.retain(|item| {
            let normalized = item.to_lowercase();
            seen.insert(normalized)
        });

        Ok(all_items)
    }

    /// Parse numbered items from LLM response
    fn parse_numbered_items(response: &str, expected_start: u32, expected_end: u32) -> Vec<String> {
        let mut items = Vec::new();
        let re = regex::Regex::new(r"^(\d+)\.\s*(.+)$").unwrap();

        for line in response.lines() {
            let line = line.trim();
            if let Some(caps) = re.captures(line) {
                if let (Some(num_match), Some(content_match)) = (caps.get(1), caps.get(2)) {
                    if let Ok(num) = num_match.as_str().parse::<u32>() {
                        if num >= expected_start && num <= expected_end {
                            let content = content_match.as_str().trim();
                            if !content.is_empty() {
                                items.push(format!("{}. {}", num, content));
                            }
                        }
                    }
                }
            }
        }

        items
    }

    /// Build file content from chunked items
    fn build_chunked_file_content(
        items: &[String],
        original_task: &str,
        file_path: &str,
    ) -> String {
        // Detect if this should be HTML, JSON, or plain text based on task/path
        let is_html = file_path.ends_with(".html") || original_task.to_lowercase().contains("html")
            || original_task.to_lowercase().contains("website")
            || original_task.to_lowercase().contains("page");
        let is_json = file_path.ends_with(".json");

        if is_html {
            Self::build_html_with_items(items, original_task)
        } else if is_json {
            Self::build_json_with_items(items)
        } else {
            items.join("\n")
        }
    }

    /// Build HTML page with items
    fn build_html_with_items(items: &[String], task: &str) -> String {
        let title = if task.to_lowercase().contains("cat") {
            "Cat Facts"
        } else {
            "Generated Content"
        };

        let items_js: String = items
            .iter()
            .map(|item| format!("  \"{}\",", item.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
body {{
  margin: 0;
  min-height: 100vh;
  background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
  font-family: 'Segoe UI', system-ui, sans-serif;
  overflow-x: hidden;
}}
.container {{
  max-width: 1200px;
  margin: 0 auto;
  padding: 2rem;
}}
h1 {{
  text-align: center;
  color: #e94560;
  font-size: 3rem;
  text-shadow: 0 0 20px rgba(233, 69, 96, 0.5);
  margin-bottom: 2rem;
}}
.stats {{
  text-align: center;
  color: #eee;
  margin-bottom: 2rem;
  font-size: 1.2rem;
}}
.bubble {{
  position: fixed;
  background: rgba(233, 69, 96, 0.9);
  border-radius: 50% 50% 50% 50% / 60% 60% 40% 40%;
  color: white;
  padding: 15px 20px;
  font-size: 12px;
  max-width: 200px;
  text-align: center;
  box-shadow: 0 4px 15px rgba(0,0,0,0.3);
  cursor: pointer;
  transition: transform 0.3s, box-shadow 0.3s;
  animation: float var(--duration) ease-in-out infinite;
}}
.bubble::before {{
  content: '';
  position: absolute;
  width: 20px;
  height: 20px;
  background: inherit;
  border-radius: 50%;
  top: -8px;
  left: 10px;
}}
.bubble::after {{
  content: '';
  position: absolute;
  width: 12px;
  height: 12px;
  background: inherit;
  border-radius: 50%;
  top: -15px;
  left: 25px;
}}
.bubble:hover {{
  transform: scale(1.1);
  box-shadow: 0 8px 25px rgba(233, 69, 96, 0.5);
  z-index: 1000;
}}
@keyframes float {{
  0%, 100% {{ transform: translateY(0) rotate(-2deg); }}
  50% {{ transform: translateY(-20px) rotate(2deg); }}
}}
.list-view {{
  display: grid;
  gap: 1rem;
}}
.list-item {{
  background: rgba(255,255,255,0.1);
  padding: 1rem;
  border-radius: 8px;
  color: #eee;
  transition: background 0.3s;
}}
.list-item:hover {{
  background: rgba(233, 69, 96, 0.3);
}}
.toggle-btn {{
  display: block;
  margin: 2rem auto;
  padding: 1rem 2rem;
  background: #e94560;
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  cursor: pointer;
  transition: background 0.3s;
}}
.toggle-btn:hover {{
  background: #ff6b6b;
}}
#bubbles {{ display: block; }}
#list {{ display: none; }}
</style>
</head>
<body>
<div class="container">
  <h1>{title}</h1>
  <div class="stats">Total items: <span id="count">0</span></div>
  <button class="toggle-btn" onclick="toggleView()">Switch to List View</button>
</div>
<div id="bubbles"></div>
<div id="list" class="container list-view"></div>

<script>
const items = [
{items_js}
];

document.getElementById('count').textContent = items.length;

let bubbleMode = true;
const bubbleContainer = document.getElementById('bubbles');
const listContainer = document.getElementById('list');

// Create floating bubbles
function createBubbles() {{
  bubbleContainer.innerHTML = '';
  const displayed = new Set();
  const maxBubbles = Math.min(30, items.length);

  for (let i = 0; i < maxBubbles; i++) {{
    createBubble();
  }}
}}

function createBubble() {{
  const item = items[Math.floor(Math.random() * items.length)];
  const bubble = document.createElement('div');
  bubble.className = 'bubble';
  bubble.textContent = item;
  bubble.style.left = Math.random() * (window.innerWidth - 250) + 'px';
  bubble.style.top = Math.random() * (window.innerHeight - 100) + 100 + 'px';
  bubble.style.setProperty('--duration', (3 + Math.random() * 4) + 's');
  bubble.onclick = () => {{
    bubble.remove();
    setTimeout(createBubble, 1000);
  }};
  bubbleContainer.appendChild(bubble);
}}

// Create list view
function createList() {{
  listContainer.innerHTML = items.map(item =>
    `<div class="list-item">${{item}}</div>`
  ).join('');
}}

function toggleView() {{
  bubbleMode = !bubbleMode;
  bubbleContainer.style.display = bubbleMode ? 'block' : 'none';
  listContainer.style.display = bubbleMode ? 'none' : 'block';
  document.querySelector('.toggle-btn').textContent =
    bubbleMode ? 'Switch to List View' : 'Switch to Bubble View';
}}

createBubbles();
createList();

// Refresh bubbles periodically
setInterval(() => {{
  if (bubbleMode && bubbleContainer.children.length < 30) {{
    createBubble();
  }}
}}, 2000);
</script>
</body>
</html>"#,
            title = title,
            items_js = items_js
        )
    }

    /// Build JSON array with items
    fn build_json_with_items(items: &[String]) -> String {
        let json_items: Vec<String> = items
            .iter()
            .map(|item| format!("  \"{}\"", item.replace('"', "\\\"")))
            .collect();
        format!("[\n{}\n]", json_items.join(",\n"))
    }

    /// Plan a large list request using chunked generation
    async fn plan_chunked_list(
        &mut self,
        task: &str,
        count: u32,
        item_type: &str,
    ) -> Result<ExecutionPlan, GaneshaError> {
        use crate::cli::print_info;

        // Check if user specified a directory
        let directory = Self::extract_directory_path(task);

        // Determine output filename (without directory)
        let filename = Self::extract_file_path(task).unwrap_or_else(|| {
            // Generate a sensible default filename from item_type
            // item_type might be "gargoyle facts" or "real facts" - extract the noun
            let clean_type = item_type
                .split_whitespace()
                .last()  // Get last word (e.g., "facts" from "gargoyle facts")
                .unwrap_or(item_type)
                .trim_end_matches('s')  // "facts" -> "fact"
                .replace(' ', "_");  // Safety: replace any remaining spaces

            let extension = if task.to_lowercase().contains("html")
                || task.to_lowercase().contains("page")
                || task.to_lowercase().contains("website") {
                "html"
            } else if task.to_lowercase().contains("json") {
                "json"
            } else {
                "txt"
            };

            format!("{}_{}.{}", clean_type, count, extension)
        });

        // Combine directory and filename if directory was specified
        // and filename doesn't already include it
        let file_path = if let Some(ref dir) = directory {
            // Check if filename already has a path
            if filename.starts_with('/') || filename.starts_with(dir.as_str()) {
                filename  // Already has full path
            } else {
                format!("{}/{}", dir.trim_end_matches('/'), filename)
            }
        } else {
            filename
        };

        print_info(&format!(
            "Generating {} {} in chunks (chunked generation mode)...",
            count, item_type
        ));

        // Generate all items using chunked requests
        let items = self.generate_chunked_list(count, item_type, task).await?;

        print_info(&format!("Generated {} unique items", items.len()));

        // Build file content
        let content = Self::build_chunked_file_content(&items, task, &file_path);

        // Create the write command (properly quote path if it has spaces)
        let quoted_path = Self::quote_path_if_needed(&file_path);
        let write_command = format!(
            "cat << 'GANESHA_EOF' > {}\n{}\nGANESHA_EOF",
            quoted_path, content
        );

        // Build the execution plan
        let mut plan = ExecutionPlan::new(task);

        // Add mkdir command if directory was specified
        if let Some(ref dir) = directory {
            let quoted_dir = Self::quote_path_if_needed(dir);
            plan.actions.push(Action {
                id: Uuid::new_v4().to_string()[..8].to_string(),
                action_type: ActionType::Shell,
                command: format!("mkdir -p {}", quoted_dir),
                explanation: format!("Create directory: {}", dir),
                risk_level: RiskLevel::Low,
                reversible: true,
                reverse_command: Some(format!("rmdir {}", quoted_dir)),
                question: None,
            });
        }

        plan.actions.push(Action {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            action_type: ActionType::FileWrite,
            command: write_command,
            explanation: format!(
                "Write {} {} to {} (generated via chunked requests)",
                items.len(), item_type, file_path
            ),
            risk_level: RiskLevel::Low,
            reversible: true,
            reverse_command: Some(format!("rm {}", quoted_path)),
            question: None,
        });

        // Update session
        if let Some(ref mut session) = self.current_session {
            session.plan = Some(plan.clone());
            session.state = SessionState::AwaitingConsent;
        }

        Ok(plan)
    }

    /// Extract file path from task description
    fn extract_file_path(task: &str) -> Option<String> {
        // Look for common patterns like "to file.html", "in output.txt", "as data.json"
        // Also support quoted paths and paths with spaces
        let patterns = [
            // Quoted paths (with spaces)
            r#"(?:to|into|in|as|called|named)\s+"([^"]+\.[a-z]+)""#,
            r#"(?:to|into|in|as|called|named)\s+'([^']+\.[a-z]+)'"#,
            // Unquoted paths (no spaces)
            r"(?:to|into|in|as|called|named)\s+([a-zA-Z0-9_\-./]+\.[a-z]+)",
            r"([a-zA-Z0-9_\-]+\.(?:html|json|txt|md|csv))(?:\s|$)",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(task) {
                    if let Some(m) = caps.get(1) {
                        return Some(m.as_str().to_string());
                    }
                }
            }
        }
        None
    }

    /// Extract directory path from task description (for "make folder X" patterns)
    fn extract_directory_path(task: &str) -> Option<String> {
        let patterns = [
            r"(?i)(?:make|create)\s+(?:a\s+)?(?:folder|directory|dir)\s+([/a-zA-Z0-9_\-\.]+)",
            r"(?i)(?:in|into)\s+(?:folder|directory|dir)\s+([/a-zA-Z0-9_\-\.]+)",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(task) {
                    if let Some(m) = caps.get(1) {
                        return Some(m.as_str().to_string());
                    }
                }
            }
        }
        None
    }

    /// Quote a path for shell usage if it contains spaces
    fn quote_path_if_needed(path: &str) -> String {
        if path.contains(' ') || path.contains('\'') || path.contains('"') {
            // Use single quotes, escaping any single quotes in the path
            format!("'{}'", path.replace('\'', "'\\''"))
        } else {
            path.to_string()
        }
    }

    fn parse_actions(&self, response: &str) -> Result<Vec<Action>, GaneshaError> {
        // Check for LM Studio function-calling format FIRST (before stripping)
        // This format gets destroyed by strip_control_tokens's prefix removal
        // We do light cleanup here: remove only the control tokens, not prefixes
        let lightly_cleaned = Self::strip_control_tokens_preserve_prefix(response);
        if std::env::var("GANESHA_DEBUG").is_ok() {
            eprintln!("[DEBUG] Lightly cleaned: {}", &lightly_cleaned[..lightly_cleaned.len().min(150)]);
        }
        if let Some(action) = Self::parse_lm_studio_function_call(&lightly_cleaned) {
            return Ok(vec![action]);
        }

        // Now do full stripping for standard JSON parsing
        let response = Self::strip_control_tokens(response);

        // Strip markdown code block markers
        let response = Self::strip_markdown_code_blocks(&response);

        if std::env::var("GANESHA_DEBUG").is_ok() {
            eprintln!("[DEBUG] Stripped response: {}", &response[..response.len().min(200)]);
        }

        // First, try to extract JSON from response
        // Look for JSON containing "actions" or "response" key
        if let Some(json_str) = Self::extract_best_json(&response) {
            // Sanitize JSON string - escape control characters properly
            let sanitized = Self::sanitize_json_string(&json_str);


            // First try to parse as a question with options
            #[derive(Deserialize)]
            struct QuestionResponse {
                question: String,
                options: Vec<String>,
            }

            if let Ok(q) = serde_json::from_str::<QuestionResponse>(&sanitized) {
                if !q.question.is_empty() && !q.options.is_empty() {
                    // Return a Question action
                    return Ok(vec![Action {
                        id: Uuid::new_v4().to_string()[..8].to_string(),
                        action_type: ActionType::Question,
                        command: String::new(),
                        explanation: q.question.clone(),
                        risk_level: RiskLevel::Low,
                        reversible: false,
                        reverse_command: None,
                        question: Some(MultipleChoiceQuestion {
                            question: q.question,
                            options: q.options,
                            context: None,
                        }),
                    }]);
                }
            }

            // Try to parse as a conversational response
            #[derive(Deserialize)]
            struct ConversationResponse {
                response: String,
            }

            if let Ok(conv) = serde_json::from_str::<ConversationResponse>(&sanitized) {
                // Return a single Response action (no command execution needed)
                return Ok(vec![Action {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    action_type: ActionType::Response,
                    command: String::new(),
                    explanation: conv.response,
                    risk_level: RiskLevel::Low,
                    reversible: false,
                    reverse_command: None,
                    question: None,
                }]);
            }

            // Otherwise parse as action plan
            #[derive(Deserialize)]
            struct PlanResponse {
                #[serde(default)]
                actions: Vec<ActionJson>,
            }

            #[derive(Deserialize)]
            struct ActionJson {
                #[serde(default)]
                command: String,
                #[serde(default)]
                explanation: String,
                #[serde(default)]
                reversible: bool,
                reverse_command: Option<String>,
                // MCP tool fields
                mcp_tool: Option<String>,
                mcp_args: Option<serde_json::Value>,
            }

            // Try to parse as action plan
            // Only treat as action plan if JSON explicitly contains "actions" key
            let has_actions_key = sanitized.contains("\"actions\"");

            match serde_json::from_str::<PlanResponse>(&sanitized) {
                Ok(parsed) if !parsed.actions.is_empty() => {
                    return Ok(parsed
                        .actions
                        .into_iter()
                        .map(|a| {
                            // Check for MCP tool call
                            if let Some(mcp_tool) = a.mcp_tool {
                                // Encode args in command: "server:tool|{json_args}"
                                let args_json = a.mcp_args
                                    .map(|v| serde_json::to_string(&v).unwrap_or_default())
                                    .unwrap_or_else(|| "{}".to_string());
                                Action {
                                    id: Uuid::new_v4().to_string()[..8].to_string(),
                                    action_type: ActionType::McpTool,
                                    command: format!("{}|{}", mcp_tool, args_json),
                                    explanation: a.explanation,
                                    risk_level: RiskLevel::Low,
                                    reversible: false,
                                    reverse_command: None,
                                    question: None,
                                }
                            } else {
                                Action {
                                    id: Uuid::new_v4().to_string()[..8].to_string(),
                                    action_type: ActionType::Shell,
                                    command: a.command,
                                    explanation: a.explanation,
                                    risk_level: RiskLevel::Low,
                                    reversible: a.reversible,
                                    reverse_command: a.reverse_command,
                                    question: None,
                                }
                            }
                        })
                        .collect());
                }
                Ok(_) if has_actions_key => {
                    // Empty actions array WITH explicit actions key - LLM has nothing to do
                    return Ok(vec![Action {
                        id: Uuid::new_v4().to_string()[..8].to_string(),
                        action_type: ActionType::Response,
                        command: String::new(),
                        explanation: "I understand, but there are no actions to perform for this request.".to_string(),
                        risk_level: RiskLevel::Low,
                        reversible: false,
                        reverse_command: None,
                                question: None,
                    }]);
                }
                Err(e) if has_actions_key => {
                    // Has actions key but failed to parse - log the error for debugging
                    if std::env::var("GANESHA_DEBUG").is_ok() {
                        eprintln!("[DEBUG] PlanResponse parse error: {}", e);
                        eprintln!("[DEBUG] Sanitized JSON: {}", &sanitized[..std::cmp::min(500, sanitized.len())]);
                    }
                    // Try other parsers
                }
                _ => {
                    // Different format - try other parsers
                }
            }

            // Try alternative JSON formats some models use
            // Format: {"cmd": ["bash", "-c", "command"]} or {"cmd": "command"}
            #[derive(Deserialize)]
            struct AltCmdFormat {
                cmd: serde_json::Value,
                #[serde(default)]
                timeout: Option<u64>,
            }
            if let Ok(alt) = serde_json::from_str::<AltCmdFormat>(&sanitized) {
                let command = match alt.cmd {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Array(arr) => {
                        // ["bash", "-c", "actual command"] -> extract the actual command
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .last()
                            .unwrap_or("")
                            .to_string()
                    }
                    _ => String::new(),
                };
                if !command.is_empty() {
                                        return Ok(vec![Action {
                        id: Uuid::new_v4().to_string()[..8].to_string(),
                        action_type: ActionType::Shell,
                        command,
                        explanation: "Executing command".to_string(),
                        risk_level: RiskLevel::Low,
                        reversible: false,
                        reverse_command: None,
                                question: None,
                    }]);
                }
            }

            // Try {"": "answer"} format (empty key = conversational response)
            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&sanitized) {
                if let Some(answer) = map.get("") {
                                        return Ok(vec![Action {
                        id: Uuid::new_v4().to_string()[..8].to_string(),
                        action_type: ActionType::Response,
                        command: String::new(),
                        explanation: answer.clone(),
                        risk_level: RiskLevel::Low,
                        reversible: false,
                        reverse_command: None,
                                question: None,
                    }]);
                }
            }

            // Try nested structures like {"Questions":{"":"answer"}} or {"Response":{"":"answer"}}
            // BUT only if there's no "actions" key (which should have been handled above)
            if !has_actions_key {
                if let Ok(outer) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&sanitized) {
                    for (_key, value) in outer.iter() {
                        // Check if value is an object with empty key
                        if let Some(obj) = value.as_object() {
                            if let Some(answer) = obj.get("").and_then(|v| v.as_str()) {
                                return Ok(vec![Action {
                                    id: Uuid::new_v4().to_string()[..8].to_string(),
                                    action_type: ActionType::Response,
                                    command: String::new(),
                                    explanation: answer.to_string(),
                                    risk_level: RiskLevel::Low,
                                    reversible: false,
                                    reverse_command: None,
                                question: None,
                                }]);
                            }
                        }
                        // Also check if value is directly a string response
                        if let Some(answer) = value.as_str() {
                            return Ok(vec![Action {
                                id: Uuid::new_v4().to_string()[..8].to_string(),
                                action_type: ActionType::Response,
                                command: String::new(),
                                explanation: answer.to_string(),
                                risk_level: RiskLevel::Low,
                                reversible: false,
                                reverse_command: None,
                                question: None,
                            }]);
                        }
                    }
                }
            }

            // JSON parsing failed - try regex extraction for {"response":"..."}
            // This handles curly quotes and other characters that break JSON parsing
            if let Ok(re) = regex::Regex::new(r#"[""]response[""]\s*:\s*[""](.+)[""]"#) {
                if let Some(caps) = re.captures(&sanitized) {
                    if let Some(text) = caps.get(1) {
                        let extracted = text.as_str()
                            .replace("\\n", "\n")
                            .replace("\\t", "\t")
                            .replace("\\\"", "\"");
                        if !extracted.is_empty() {
                            return Ok(vec![Action {
                                id: Uuid::new_v4().to_string()[..8].to_string(),
                                action_type: ActionType::Response,
                                command: String::new(),
                                explanation: extracted,
                                risk_level: RiskLevel::Low,
                                reversible: false,
                                reverse_command: None,
                                question: None,
                            }]);
                        }
                    }
                }
            }

            // Final fallback - strip JSON wrapper if present
            let clean_text = response.trim()
                .trim_start_matches('{').trim_end_matches('}')
                .trim();
            // Remove "response": prefix if present
            let clean_text = if clean_text.contains("\"response\"") {
                clean_text.split(':').skip(1).collect::<Vec<_>>().join(":").trim().trim_matches('"').to_string()
            } else {
                clean_text.to_string()
            };

            return Ok(vec![Action {
                id: Uuid::new_v4().to_string()[..8].to_string(),
                action_type: ActionType::Response,
                command: String::new(),
                explanation: clean_text,
                risk_level: RiskLevel::Low,
                reversible: false,
                reverse_command: None,
                question: None,
            }]);
        } else {
            // No JSON found - check if it's a bare URL that should be converted to MCP action
            let clean_response = response.trim();

            // Check if response is just a URL and MCP browser tools are available
            if Self::is_url_response(&clean_response) && Self::has_browser_mcp() {
                // Auto-convert bare URL to MCP navigate action
                if std::env::var("GANESHA_DEBUG").is_ok() {
                    eprintln!("[DEBUG] Auto-converting bare URL to MCP navigate: {}", clean_response);
                }
                return Ok(vec![Action {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    action_type: ActionType::McpTool,
                    command: format!("playwright:browser_navigate|{{\"url\":\"{}\"}}", clean_response),
                    explanation: format!("Navigate to {}", clean_response),
                    risk_level: RiskLevel::Low,
                    reversible: false,
                    reverse_command: None,
                    question: None,
                }]);
            }

            // Treat the entire response as a conversational answer
            // This handles LLMs that don't follow the JSON format
            if clean_response.is_empty() {
                Err(GaneshaError::LlmError("Empty response from LLM".into()))
            } else {
                Ok(vec![Action {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    action_type: ActionType::Response,
                    command: String::new(),
                    explanation: clean_response.to_string(),
                    risk_level: RiskLevel::Low,
                    reversible: false,
                    reverse_command: None,
                                question: None,
                }])
            }
        }
    }

    /// Strip LLM control tokens from response (LM Studio special tokens, etc.)
    /// Also normalizes Unicode punctuation to ASCII for better terminal display
    fn strip_control_tokens(response: &str) -> String {
        let mut result = response.to_string();

        // Strip LM Studio function-calling style prefixes (before JSON)
        // Pattern: "assistantcommentary to=functions.* json" or similar
        if let Ok(func_prefix) = regex::Regex::new(r"(?i)^assistant\w*\s+to=\S+\s+json\s*") {
            result = func_prefix.replace(&result, "").to_string();
        }

        // Strip common JSON prefixes (case insensitive, with optional whitespace/newlines)
        let json_prefixes = [
            r"(?is)^\s*JSON\s+only\s*",
            r"(?is)^\s*json\s*:\s*",
            r"(?is)^\s*JSON\s*:\s*",
            r"(?is)^\s*output\s*:\s*",
            r"(?is)^\s*response\s*:\s*",
            r"(?is)^\s*result\s*:\s*",
            r"(?is)^\s*Here'?s?\s+(?:the\s+)?(?:JSON|response|output)\s*:\s*",
        ];
        for pattern in json_prefixes {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace(&result, "").to_string();
            }
        }

        // Strip everything before the first { or [ (JSON start)
        // This handles any remaining prefix garbage
        if let Some(json_start) = result.find('{').or_else(|| result.find('[')) {
            if json_start > 0 {
                let prefix = &result[..json_start];
                // Strip prefix if it's short and doesn't look like meaningful content
                // (meaningful = long with multiple words and punctuation)
                let word_count = prefix.split_whitespace().count();
                if prefix.len() < 100 && word_count < 10 {
                    result = result[json_start..].to_string();
                }
            }
        }

        // Common LLM control token patterns to remove
        result = Self::remove_control_tokens(&result);
        result.trim().to_string()
    }

    /// Strip control tokens but preserve the prefix (for LM Studio function-call parsing)
    fn strip_control_tokens_preserve_prefix(response: &str) -> String {
        let result = Self::remove_control_tokens(response);
        Self::normalize_unicode_punctuation(&result).trim().to_string()
    }

    /// Remove LLM control tokens from string
    fn remove_control_tokens(text: &str) -> String {
        let mut result = text.to_string();

        let patterns = [
            "<|channel|>",
            "<|constrain|>",
            "<|message|>",
            "<|im_start|>",
            "<|im_end|>",
            "<|endoftext|>",
            "<|assistant|>",
            "<|user|>",
            "<|system|>",
            "<|end|>",
            "<|eot_id|>",
            "<|start_header_id|>",
            "<|end_header_id|>",
        ];

        for pattern in patterns {
            result = result.replace(pattern, "");
        }

        // Strip any remaining <|...|> style tokens
        let re_pattern = regex::Regex::new(r"<\|[^|>]+\|>").unwrap_or_else(|_| {
            regex::Regex::new(r"$^").unwrap() // Never matches
        });
        result = re_pattern.replace_all(&result, "").to_string();

        // Normalize Unicode punctuation to ASCII for better terminal display
        result = Self::normalize_unicode_punctuation(&result);

        result.trim().to_string()
    }

    /// Normalize fancy Unicode punctuation to ASCII equivalents
    fn normalize_unicode_punctuation(text: &str) -> String {
        text.chars().map(|c| match c {
            // Commas
            '\u{FF0C}' | '\u{3001}' | '\u{060C}' | '\u{1802}' | '\u{055D}' => ',',
            // Single quotes / apostrophes
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2039}' | '\u{203A}'
            | '\u{FF07}' | '\u{0060}' | '\u{00B4}' => '\'',
            // Double quotes
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{00AB}' | '\u{00BB}'
            | '\u{FF02}' => '"',
            // Dashes / hyphens
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}' => '-',
            // Ellipsis
            '\u{2026}' => '.',  // Could expand to "..." but single is safer
            // Spaces (various Unicode spaces to regular space)
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{200B}' | '\u{202F}'
            | '\u{205F}' | '\u{3000}' | '\u{FEFF}' => ' ',
            // Colons
            '\u{FF1A}' => ':',
            // Semicolons
            '\u{FF1B}' => ';',
            // Periods
            '\u{3002}' | '\u{FF0E}' => '.',
            // Question marks
            '\u{FF1F}' => '?',
            // Exclamation marks
            '\u{FF01}' => '!',
            // Parentheses
            '\u{FF08}' => '(',
            '\u{FF09}' => ')',
            // Brackets
            '\u{FF3B}' => '[',
            '\u{FF3D}' => ']',
            // Curly braces
            '\u{FF5B}' => '{',
            '\u{FF5D}' => '}',
            // Everything else passes through
            _ => c,
        }).collect()
    }

    /// Strip markdown code block markers from response
    fn strip_markdown_code_blocks(text: &str) -> String {
        let mut result = text.to_string();

        // Remove markdown code block markers - various formats
        // Backticks: ```json, ```
        // Single quotes: '''json, '''
        // Also handle with/without language specifier
        let patterns = [
            r"```json\s*\n?",
            r"```\w*\s*\n?",  // Any language specifier
            r"```\s*\n?",
            r"'''json\s*\n?",
            r"'''\w*\s*\n?",
            r"'''\s*\n?",
            r"`json\s*\n?",   // Single backtick (rare but possible)
            r"`\s*\n?",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, "").to_string();
            }
        }

        result
    }

    /// Parse LM Studio's function-calling format
    /// Format: "commentary to=server:tool mcp_args code{json_args}"
    /// Example: "commentary to=playwright:browser_navigate mcp_args code{"url":"https://google.com"}"
    fn parse_lm_studio_function_call(response: &str) -> Option<Action> {
        // Pattern to match LM Studio function call format
        // Matches: anything to=tool_name ... {json}
        // Tool name can contain letters, numbers, underscore, colon, dot, hyphen
        // Intermediate words like "mcp_args", "code", "json" are optionally matched
        // Handles multiple formats:
        // - mcp_args='{"url":"..."}'>  (with single quotes and equals)
        // - mcp_args={"url":"..."}     (with equals only)
        // - mcp_args {"url":"..."}     (space only)
        // - code{"url":"..."}          (no separator)
        let re = regex::Regex::new(
            r#"(?s)(?:commentary|assistant\w*)?\s*to=([a-zA-Z0-9_:.\-]+)\s+(?:mcp_args|code|json)?[='"]*\s*(\{.+)"#
        ).ok()?;

        let caps = match re.captures(response) {
            Some(c) => c,
            None => {
                if std::env::var("GANESHA_DEBUG").is_ok() && response.contains("to=") {
                    eprintln!("[DEBUG] LM Studio regex didn't match. Response: {}", &response[..response.len().min(150)]);
                }
                return None;
            }
        };
        let tool_name = caps.get(1)?.as_str();
        let raw_args = caps.get(2)?.as_str();

        // Extract just the JSON part - find balanced braces
        let args_json = Self::extract_first_json(raw_args)?;

        if std::env::var("GANESHA_DEBUG").is_ok() {
            eprintln!("[DEBUG] LM Studio regex matched. Tool: {}, Args: {}", tool_name, &args_json[..args_json.len().min(100)]);
        }

        // Filter out non-MCP tools like container.exec, functions.*, etc.
        // Check if tool name matches any connected MCP server OR built-in ganesha tools
        let is_valid_tool = if tool_name.contains(':') {
            use crate::orchestrator::mcp::get_all_mcp_tools;
            let server_prefix = tool_name.split(':').next().unwrap_or("");

            // Allow "ganesha:" prefix for built-in tools (web_search, etc.)
            if server_prefix == "ganesha" {
                true
            } else {
                // Check if the prefix matches any connected MCP server
                let connected_servers = get_all_mcp_tools();
                connected_servers.iter().any(|(name, _)| name == server_prefix)
            }
        } else {
            false
        };

        if !is_valid_tool {
            // This is an LM Studio built-in function, not an MCP or Ganesha tool
            // Let the normal JSON parsing handle it
            return None;
        }

        // Validate that args_json is valid JSON
        let args: serde_json::Value = serde_json::from_str(&args_json).ok()?;

        // Ensure tool_name has server:tool format
        // If no colon, assume it's a playwright tool (most common)
        let full_tool_name = if tool_name.contains(':') {
            tool_name.to_string()
        } else {
            format!("playwright:{}", tool_name)
        };

        if std::env::var("GANESHA_DEBUG").is_ok() {
            eprintln!("[DEBUG] Parsed LM Studio function call: {} with args {}", full_tool_name, args_json);
        }

        Some(Action {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            action_type: ActionType::McpTool,
            command: format!("{}|{}", full_tool_name, args_json),
            explanation: format!("MCP tool call: {}", full_tool_name),
            risk_level: RiskLevel::Low,
            reversible: false,
            reverse_command: None,
            question: None,
        })
    }

    /// Extract the first complete JSON object from a string by tracking balanced braces
    fn extract_first_json(text: &str) -> Option<String> {
        let mut depth = 0;
        let mut start: Option<usize> = None;

        for (i, ch) in text.char_indices() {
            match ch {
                '{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(text[s..=i].to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Check if response is just a URL (LLM outputting URL instead of proper JSON)
    fn is_url_response(response: &str) -> bool {
        let trimmed = response.trim();
        // Check if it's a single line that looks like a URL
        if trimmed.lines().count() > 1 {
            return false;
        }
        // Match common URL patterns
        trimmed.starts_with("http://") ||
        trimmed.starts_with("https://") ||
        (trimmed.contains('.') && !trimmed.contains(' ') &&
         (trimmed.ends_with(".com") || trimmed.ends_with(".org") ||
          trimmed.ends_with(".net") || trimmed.ends_with(".io") ||
          trimmed.ends_with(".edu") || trimmed.ends_with(".gov") ||
          trimmed.contains(".com/") || trimmed.contains(".org/")))
    }

    /// Check if browser MCP tools are connected
    fn has_browser_mcp() -> bool {
        use crate::orchestrator::mcp::get_all_mcp_tools;
        let tools = get_all_mcp_tools();
        tools.iter().any(|(name, _)| {
            name.contains("playwright") || name.contains("browser")
        })
    }

    /// Check if task is about a website/URL
    fn is_website_task(task: &str) -> bool {
        let lower = task.to_lowercase();
        // Check for explicit URLs
        if lower.contains("http://") || lower.contains("https://") {
            return true;
        }
        // Check for domain patterns
        let domain_patterns = [
            ".com", ".org", ".net", ".io", ".edu", ".gov", ".co.uk",
            ".de", ".fr", ".jp", ".au", ".ca", ".ru", ".cn", ".in",
        ];
        for pattern in domain_patterns {
            if lower.contains(pattern) {
                return true;
            }
        }
        // Check for website-related keywords
        let keywords = [
            "website", "webpage", "web page", "browse", "visit",
            "go to", "navigate to", "open", "check out", "look at",
        ];
        let website_verbs = keywords.iter().any(|k| lower.contains(k));
        // Also check if they're asking about items/content on a site
        let content_ask = lower.contains("what is on") ||
                          lower.contains("what's on") ||
                          lower.contains("items on") ||
                          lower.contains("show me");
        website_verbs || content_ask
    }

    /// Extract URL from task description
    fn extract_url_from_task(task: &str) -> String {
        // First check for explicit URLs
        let words: Vec<&str> = task.split_whitespace().collect();
        for word in &words {
            if word.starts_with("http://") || word.starts_with("https://") {
                return word.to_string();
            }
        }
        // Look for domain patterns and construct URL
        let domain_patterns = [
            ".com", ".org", ".net", ".io", ".edu", ".gov", ".co.uk",
        ];
        for word in &words {
            let lower = word.to_lowercase();
            for pattern in &domain_patterns {
                if lower.contains(pattern) {
                    // Strip punctuation from the word
                    let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-');
                    if !clean.is_empty() {
                        // Make sure it has https:// prefix
                        if clean.starts_with("http") {
                            return clean.to_string();
                        }
                        return format!("https://{}", clean);
                    }
                }
            }
        }
        String::new()
    }

    /// Extract the best JSON object from response (one containing "actions" or "response")
    fn extract_best_json(text: &str) -> Option<String> {
        // Find all JSON-like blocks in the text
        let mut candidates: Vec<String> = Vec::new();
        let mut depth = 0;
        let mut start: Option<usize> = None;

        for (i, ch) in text.char_indices() {
            match ch {
                '{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            let json_candidate = &text[s..=i];
                            candidates.push(json_candidate.to_string());
                        }
                        start = None;
                    }
                }
                _ => {}
            }
        }

        // Prefer JSON with "actions" key, then "response" key
        for candidate in &candidates {
            if candidate.contains("\"actions\"") {
                return Some(candidate.clone());
            }
        }
        for candidate in &candidates {
            if candidate.contains("\"response\"") {
                return Some(candidate.clone());
            }
        }

        // Fall back to first valid-looking JSON
        candidates.into_iter().next()
    }

    /// Sanitize JSON string by escaping control characters within string values
    fn sanitize_json_string(json: &str) -> String {
        // First, fix JavaScript-style string concatenation that some LLMs produce
        // Pattern: "text" + "more text" -> "text more text"
        let json = regex::Regex::new(r#""\s*\+\s*""#)
            .map(|re| re.replace_all(json, " ").to_string())
            .unwrap_or_else(|_| json.to_string());

        // Also fix unquoted line continuations like: 79. Cats can...\n, "80. ...
        let json = regex::Regex::new(r#"(\d+\.)\s+([^"]+)\n,\s*""#)
            .map(|re| re.replace_all(&json, r#""$1 $2", ""#).to_string())
            .unwrap_or_else(|_| json);

        let mut result = String::with_capacity(json.len());
        let mut in_string = false;
        let mut escape_next = false;

        for ch in json.chars() {
            if escape_next {
                result.push(ch);
                escape_next = false;
                continue;
            }

            if ch == '\\' && in_string {
                result.push(ch);
                escape_next = true;
                continue;
            }

            if ch == '"' {
                in_string = !in_string;
                result.push(ch);
                continue;
            }

            if in_string && ch.is_control() {
                // Escape control characters within strings
                match ch {
                    '\n' => result.push_str("\\n"),
                    '\r' => result.push_str("\\r"),
                    '\t' => result.push_str("\\t"),
                    _ => {
                        // Escape other control chars as unicode
                        result.push_str(&format!("\\u{:04x}", ch as u32));
                    }
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    fn save_session(&self, session: &Session) -> Result<(), GaneshaError> {
        let path = self.session_dir.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| GaneshaError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
