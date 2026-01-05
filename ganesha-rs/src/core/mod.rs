//! Ganesha Core - Execution Engine, Session Management, Safety

pub mod access_control;

pub use access_control::RiskLevel;

use crate::logging::SystemLogger;
use crate::providers::LlmProvider;
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
}

impl<L: LlmProvider, C: ConsentHandler> GaneshaEngine<L, C> {
    pub fn new(llm: L, consent: C, policy: AccessPolicy) -> Self {
        use directories::ProjectDirs;

        let session_dir = ProjectDirs::from("com", "gtechsd", "ganesha")
            .map(|p| p.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".ganesha"))
            .join("sessions");

        std::fs::create_dir_all(&session_dir).ok();

        Self {
            llm,
            consent,
            access: AccessController::new(policy),
            logger: SystemLogger::new(),
            auto_approve: false,
            session_dir,
            current_session: None,
        }
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

        // Generate plan via LLM
        let system_prompt = self.build_planning_prompt();
        let response = self
            .llm
            .generate(&system_prompt, task)
            .await
            .map_err(|e| GaneshaError::LlmError(e.to_string()))?;

        // Parse plan from response
        let mut plan = ExecutionPlan::new(task);
        plan.actions = self.parse_actions(&response)?;

        // Validate each action against access control
        for action in &mut plan.actions {
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

        // Get consent
        if !self.auto_approve {
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

            // Final access check
            let check = self.access.check_command(&action.command);
            if !check.allowed {
                self.logger
                    .command_denied("user", &action.command, &check.reason);
                results.push(ExecutionResult {
                    action_id: action.id.clone(),
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
                        success: true,
                        output,
                        error: None,
                        duration_ms,
                    });
                }
                Err(e) => {
                    results.push(ExecutionResult {
                        action_id: action.id.clone(),
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
        r#"You are Ganesha, an AI system control assistant.

Given a task, output a JSON plan with actions to execute.

FORMAT:
{
  "actions": [
    {
      "command": "the shell command",
      "explanation": "what this does",
      "reversible": true/false,
      "reverse_command": "command to undo (if reversible)"
    }
  ]
}

RULES:
- Use safe, idiomatic commands
- Prefer non-destructive operations
- Include verification steps
- Each action should be atomic
- Output ONLY valid JSON"#.to_string()
    }

    fn parse_actions(&self, response: &str) -> Result<Vec<Action>, GaneshaError> {
        // Try to extract JSON from response
        let json_start = response.find('{');
        let json_end = response.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &response[start..=end];

            #[derive(Deserialize)]
            struct PlanResponse {
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

            let parsed: PlanResponse = serde_json::from_str(json_str)
                .map_err(|e| GaneshaError::LlmError(format!("Failed to parse plan: {}", e)))?;

            Ok(parsed
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
                .collect())
        } else {
            Err(GaneshaError::LlmError("No valid JSON in response".into()))
        }
    }

    fn save_session(&self, session: &Session) -> Result<(), GaneshaError> {
        let path = self.session_dir.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| GaneshaError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
