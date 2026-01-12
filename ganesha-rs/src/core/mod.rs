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
    Custom(String),
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

        // Build messages with conversation history
        let system_prompt = self.build_planning_prompt();

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

        // Validate each action against access control (skip Response actions)
        for action in &mut plan.actions {
            // Response actions don't need access control - they're just text
            if matches!(action.action_type, ActionType::Response) {
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
            result_summary.push_str(&format!(
                "Command: {}\nStatus: {}\nOutput:\n{}\n\n",
                result.command,
                if result.success { "SUCCESS" } else { "FAILED" },
                if result.output.len() > 2000 {
                    format!("{}...(truncated)", &result.output[..2000])
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

        let system_prompt = format!(
            r#"You are Ganesha, an AI system administrator. The user asked: "{}"

Commands executed with results:
{}

YOUR JOB: Interpret the output and respond OR fix errors.

RESPONSE FORMAT (JSON only):
1. Task complete: {{"response":"<plain English interpretation>"}}
2. Need more actions: {{"actions":[{{"command":"cmd","explanation":"why"}}]}}

CRITICAL - CHECK IF USER'S TARGET IS IN OUTPUT:
- User asked "is X running/installed?" → Check if X appears in output
- If X is NOT in output → Say "No, X is not running/installed"
- If output shows other things but NOT X → "No, X is not there. Only Y and Z are running."

ERROR HANDLING - If command failed, FIX IT:
- "container name already in use" → {{"actions":[{{"command":"docker rm -f <name>","explanation":"Remove old container"}},{{"command":"docker run ...","explanation":"Try again"}}]}}
- "permission denied" → suggest sudo or fix permissions
- "not found" → suggest installation
- NEVER just stop on errors - always try to fix or explain how to fix

INTERPRETATION RULES:
- ALWAYS give a clear answer to the user's question
- Use EXACT values from output
- For errors: explain what went wrong AND how to fix

EXAMPLES:
- User: "is pihole running?" Output: "n8n\ngitlab" → {{"response":"No, pihole is not running. Only n8n and gitlab are running."}}
- Error: "container name in use" → {{"actions":[{{"command":"docker rm -f pihole","explanation":"Remove existing container"}},{{"command":"docker run -d --name pihole pihole/pihole","explanation":"Start fresh"}}]}}
- User: "is apache installed?" Output: "not found" → {{"response":"No, Apache is not installed. Would you like me to install it?"}}"#,
            task, result_summary
        );

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

        // Fallback - if cleaned looks like plain text (not JSON), use it directly
        let cleaned_trimmed = cleaned.trim();
        if !cleaned_trimmed.is_empty()
            && !cleaned_trimmed.starts_with('{')
            && !cleaned_trimmed.starts_with('[')
        {
            return Ok((cleaned_trimmed.to_string(), None));
        }

        // Last resort - return empty (execution output was already shown)
        Ok((String::new(), None))
    }

    async fn execute_command(&self, command: &str) -> Result<String, GaneshaError> {
        use tokio::process::Command;

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", command])
                .output()
                .await?
        } else {
            Command::new("sh")
                .args(["-c", command])
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

        format!(r#"You are Ganesha, a system control assistant. Working directory: {}{}

OUTPUT FORMAT - MANDATORY JSON:
System tasks: {{"actions":[{{"command":"cmd","explanation":"brief"}}]}}
Questions: {{"response":"brief answer"}}

RULES:
- Output ONLY valid JSON - no markdown, no code blocks
- For checks (is X installed, status, etc) → run the command to find out
- Use ADVANCED BASH: pipes, grep, awk for extracting info
- Multiple related actions can go in one plan

MULTI-STEP TASKS (installations, configurations):
Include verification in your plan:
1. Do the action
2. Verify it worked (e.g., docker ps | grep name, which pkg, systemctl status)

EXAMPLES:
- "is nginx running" → {{"actions":[{{"command":"systemctl status nginx | grep Active","explanation":"Check nginx status"}}]}}
- "install X in docker" → {{"actions":[{{"command":"docker pull X","explanation":"Pull image"}},{{"command":"docker run -d X","explanation":"Start container"}},{{"command":"docker ps | grep X","explanation":"Verify running"}}]}}
- "what's in docker" → {{"actions":[{{"command":"docker ps --format 'table {{{{.Names}}}}\t{{{{.Status}}}}'","explanation":"List containers"}}]}}

BASH TECHNIQUES:
- grep -E for regex matching
- awk for column extraction
- pipes to chain commands
- docker ps --format for cleaner output"#, self.working_directory.display(), auto_mode)
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
        // Strip LLM control tokens (LM Studio, etc.)
        let response = Self::strip_control_tokens(response);

        // Strip markdown code block markers
        let response = Self::strip_markdown_code_blocks(&response);

        // First, try to extract JSON from response
        // Look for JSON containing "actions" or "response" key
        if let Some(json_str) = Self::extract_best_json(&response) {
            // Sanitize JSON string - escape control characters properly
            let sanitized = Self::sanitize_json_string(&json_str);


            // First try to parse as a conversational response
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
                command: String,
                explanation: String,
                #[serde(default)]
                reversible: bool,
                reverse_command: Option<String>,
            }

            // Try to parse as action plan
            // Only treat as action plan if JSON explicitly contains "actions" key
            let has_actions_key = sanitized.contains("\"actions\"");

            match serde_json::from_str::<PlanResponse>(&sanitized) {
                Ok(parsed) if !parsed.actions.is_empty() => {
                    return Ok(parsed
                        .actions
                        .into_iter()
                        .map(|a| Action {
                            id: Uuid::new_v4().to_string()[..8].to_string(),
                            action_type: ActionType::Shell,
                            command: a.command,
                            explanation: a.explanation,
                            risk_level: RiskLevel::Low, // Will be set by access check
                            reversible: a.reversible,
                            reverse_command: a.reverse_command,
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
                            }]);
                        }
                    }
                }
            }

            // JSON parsing failed - fall back to treating as conversational response
            // This handles cases where LLM outputs malformed JSON or large content
            return Ok(vec![Action {
                id: Uuid::new_v4().to_string()[..8].to_string(),
                action_type: ActionType::Response,
                command: String::new(),
                explanation: response.trim().to_string(),
                risk_level: RiskLevel::Low,
                reversible: false,
                reverse_command: None,
            }]);
        } else {
            // No JSON found - treat the entire response as a conversational answer
            // This handles LLMs that don't follow the JSON format
            let clean_response = response.trim();
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

        // Strip everything before the first { or [ (JSON start)
        // This handles all prefix garbage like "json", "JSON:", "output:", etc.
        if let Some(json_start) = result.find('{').or_else(|| result.find('[')) {
            if json_start > 0 {
                let prefix = &result[..json_start];
                // Only keep prefix if it looks like meaningful content (long and has spaces)
                if prefix.len() < 50 && !prefix.contains('\n') {
                    result = result[json_start..].to_string();
                }
            }
        }

        // Common LLM control token patterns to remove
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
            "final ",  // Often follows <|channel|>
            "response",  // Often follows <|constrain|>
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
