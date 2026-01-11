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

            let check = self.access.check_command(&action.command);
            action.risk_level = check.risk_level;

            if !check.allowed {
                self.logger
                    .command_denied("user", &action.command, &check.reason);
                return Err(GaneshaError::AccessDenied(check.reason));
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

            // Final access check
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

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(GaneshaError::ExecutionFailed(stderr.to_string()))
        }
    }

    fn build_planning_prompt(&self) -> String {
        format!(r#"You are Ganesha, an AI system control assistant and knowledgeable helper.

CURRENT CONTEXT:
- Working directory: {}

CONVERSATION MEMORY - CRITICAL:
This is a MULTI-TURN conversation. The messages above contain our conversation history.
When the user asks follow-up questions like:
- "how many people live there" → refers to the location discussed previously
- "who celebrates it" → refers to the event/holiday discussed previously
- "tell me more" → expand on the previous topic
- "what about..." → continues the previous discussion
YOU MUST use the conversation history to understand what "there", "it", "that", etc. refer to.

Given a task, determine if it requires system commands or is a question/conversation.

FOR SYSTEM TASKS (file operations, commands, etc):
{{
  "actions": [
    {{
      "command": "the shell command",
      "explanation": "what this does",
      "reversible": true/false,
      "reverse_command": "command to undo (if reversible)"
    }}
  ]
}}

FOR QUESTIONS/CONVERSATIONS (no commands needed):
{{
  "response": "Your helpful answer here"
}}

FILE WRITING - IMPORTANT:
When generating large content (HTML pages, code files, lists with many items, etc.):
- Use shell commands to write directly to files: cat << 'EOF' > filename.ext
- DO NOT display the full content in chat - it floods the screen
- Simply confirm: "Writing filename.ext with [description]..."
- You are CAPABLE of generating large amounts of content (1000+ items, full applications, etc.)
- Generate the COMPLETE content as requested - do not truncate or use placeholders
- If writing code, HTML, or data files, write them directly - the user will open/view them

RULES:
- ALWAYS check conversation history for context on pronouns (it, there, that, they, etc.)
- For questions, use the response format
- For system tasks, use the actions format with safe, idiomatic commands
- Prefer non-destructive operations
- Each action should be atomic
- Be confident - you have extensive knowledge and can generate substantial content"#, self.working_directory.display())
    }

    fn parse_actions(&self, response: &str) -> Result<Vec<Action>, GaneshaError> {
        // Strip LLM control tokens (LM Studio, etc.)
        let response = Self::strip_control_tokens(response);

        // First, try to extract JSON from response
        let json_start = response.find('{');
        let json_end = response.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &response[start..=end];

            // Sanitize JSON string - escape control characters properly
            let sanitized = Self::sanitize_json_string(json_str);

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
            if let Ok(parsed) = serde_json::from_str::<PlanResponse>(&sanitized) {
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

        let mut result = response.to_string();
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

    /// Sanitize JSON string by escaping control characters within string values
    fn sanitize_json_string(json: &str) -> String {
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
