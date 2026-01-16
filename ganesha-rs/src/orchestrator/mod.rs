//! Ganesha Orchestrator - Mini-Me Sub-Agent Architecture
//!
//! The orchestrator manages long-horizon tasks using a hierarchical agent system:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     GANESHA ORCHESTRATOR                        │
//! │                    (Primary Model - BEAST)                      │
//! │                                                                 │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
//! │  │   Mini-Me 1  │  │   Mini-Me 2  │  │   Mini-Me 3  │          │
//! │  │  (BEDROOM)   │  │  (BEDROOM)   │  │  (Anthropic) │          │
//! │  │  file search │  │  code edit   │  │  complex Q   │          │
//! │  └──────────────┘  └──────────────┘  └──────────────┘          │
//! │                                                                 │
//! │  ┌──────────────┐                                               │
//! │  │    Vision    │  (Screen analysis when needed)               │
//! │  │  (BEDROOM)   │                                               │
//! │  └──────────────┘                                               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Key principles:
//! - **Context Forking**: Mini-Me agents get minimal context, not full history
//! - **Summary Reporting**: Mini-Me returns summaries, not transcripts
//! - **Parallel Execution**: Multiple Mini-Me can run simultaneously
//! - **Escalation**: Mini-Me can escalate to paid services when needed
//! - **Ralph Wiggum Loop**: Verify results match intent, iterate if not

pub mod minime;
pub mod tools;
pub mod wiggum;
pub mod memory;
pub mod memory_db;  // SQLite-based scalable memory
pub mod engine;
pub mod mcp;
pub mod rollback;
pub mod scheduler;
pub mod vision;
pub mod providers;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Semaphore};
use uuid::Uuid;

use crate::providers::{LlmProvider, ProviderError};
use crate::core::config::{ModelTier, ProviderConfig, ConfigManager};

/// Task for a Mini-Me agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniMeTask {
    pub id: Uuid,
    pub description: String,
    pub context: ForkedContext,
    pub required_tier: ModelTier,
    pub allow_escalation: bool,
    pub timeout: Duration,
}

/// Forked context for Mini-Me (minimal, focused)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkedContext {
    /// The specific goal for this subtask
    pub goal: String,
    /// Relevant file paths (not contents, Mini-Me will read)
    pub relevant_files: Vec<String>,
    /// Key facts the parent wants Mini-Me to know
    pub facts: Vec<String>,
    /// Tools Mini-Me is allowed to use
    pub allowed_tools: Vec<String>,
    /// Working directory
    pub cwd: String,
}

/// Result from a Mini-Me task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniMeResult {
    pub task_id: Uuid,
    pub success: bool,
    /// Summary for the parent (not full transcript)
    pub summary: String,
    /// Key findings/outputs
    pub findings: Vec<String>,
    /// Files modified
    pub files_modified: Vec<String>,
    /// Errors encountered
    pub errors: Vec<String>,
    /// Time taken
    pub duration: Duration,
    /// Model used (may have escalated)
    pub model_used: String,
    /// Tokens used
    pub tokens_used: u32,
    /// Cost (if cloud)
    pub cost: f64,
}

/// The main orchestrator state
#[derive(Debug)]
pub struct Orchestrator {
    /// Available provider configurations
    providers: Vec<ProviderConfig>,
    /// Semaphores for rate limiting per provider
    semaphores: HashMap<String, Arc<Semaphore>>,
    /// Active Mini-Me tasks
    active_tasks: Arc<RwLock<HashMap<Uuid, MiniMeTask>>>,
    /// Completed results
    results: Arc<RwLock<HashMap<Uuid, MiniMeResult>>>,
    /// Session context (the orchestrator's full context)
    session_context: Arc<RwLock<SessionContext>>,
    /// Message passing for results
    result_tx: mpsc::Sender<MiniMeResult>,
    result_rx: Arc<RwLock<mpsc::Receiver<MiniMeResult>>>,
}

/// The orchestrator's session context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: Uuid,
    pub started_at: String,
    pub primary_goal: String,
    pub current_plan: Vec<PlanStep>,
    pub completed_steps: Vec<CompletedStep>,
    pub pending_minime: Vec<Uuid>,
    pub conversation_summary: String,
    pub key_decisions: Vec<String>,
    pub files_in_scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: usize,
    pub description: String,
    pub status: StepStatus,
    pub assigned_to: Option<Uuid>, // Mini-Me task ID if delegated
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    InProgress,
    Delegated,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedStep {
    pub step_id: usize,
    pub summary: String,
    pub outputs: Vec<String>,
}

impl Orchestrator {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);

        // Load providers from config
        let config_manager = ConfigManager::new();
        let config = config_manager.load();
        let providers = config.providers;

        // Create semaphores for rate limiting
        let mut semaphores = HashMap::new();
        for p in &providers {
            semaphores.insert(
                p.name.clone(),
                Arc::new(Semaphore::new(p.max_concurrent)),
            );
        }

        Self {
            providers,
            semaphores,
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            session_context: Arc::new(RwLock::new(SessionContext {
                session_id: Uuid::new_v4(),
                started_at: chrono::Utc::now().to_rfc3339(),
                primary_goal: String::new(),
                current_plan: vec![],
                completed_steps: vec![],
                pending_minime: vec![],
                conversation_summary: String::new(),
                key_decisions: vec![],
                files_in_scope: vec![],
            })),
            result_tx: tx,
            result_rx: Arc::new(RwLock::new(rx)),
        }
    }

    /// Get the best available provider for a tier
    pub fn get_provider(&self, tier: ModelTier) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| {
            p.tier == tier && p.api_key.is_some() || p.api_key.is_none()
        })
    }

    /// Spawn a Mini-Me task
    pub async fn spawn_minime(&self, task: MiniMeTask) -> Uuid {
        let task_id = task.id;

        // Add to active tasks
        self.active_tasks.write().await.insert(task_id, task.clone());

        // Add to session pending list
        self.session_context.write().await.pending_minime.push(task_id);

        // Get provider
        let provider = self.get_provider(task.required_tier)
            .cloned()
            .unwrap_or_else(|| self.providers[0].clone());

        let semaphore = self.semaphores.get(&provider.name)
            .cloned()
            .unwrap_or_else(|| Arc::new(Semaphore::new(1)));

        let result_tx = self.result_tx.clone();

        // Spawn the Mini-Me execution
        tokio::spawn(async move {
            // Acquire semaphore permit
            let _permit = semaphore.acquire().await.unwrap();

            let start = Instant::now();

            // Execute the Mini-Me task
            let result = minime::execute_task(&task, &provider).await;

            let duration = start.elapsed();

            // Send result back to orchestrator
            let _ = result_tx.send(MiniMeResult {
                task_id,
                success: result.is_ok(),
                summary: result.as_ref()
                    .map(|r| r.clone())
                    .unwrap_or_else(|e| format!("Error: {}", e)),
                findings: vec![],
                files_modified: vec![],
                errors: result.err().map(|e| vec![e.to_string()]).unwrap_or_default(),
                duration,
                model_used: provider.model.clone(),
                tokens_used: 0,
                cost: 0.0,
            }).await;
        });

        task_id
    }

    /// Fork context for a subtask
    pub fn fork_context(&self, goal: &str, relevant_files: Vec<String>) -> ForkedContext {
        ForkedContext {
            goal: goal.to_string(),
            relevant_files,
            facts: vec![],
            allowed_tools: vec![
                "read_file".into(),
                "write_file".into(),
                "bash".into(),
                "search".into(),
            ],
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".into()),
        }
    }

    /// Wait for a specific Mini-Me to complete
    pub async fn wait_for(&self, task_id: Uuid) -> Option<MiniMeResult> {
        let mut rx = self.result_rx.write().await;

        while let Some(result) = rx.recv().await {
            // Store result
            self.results.write().await.insert(result.task_id, result.clone());

            // Remove from active
            self.active_tasks.write().await.remove(&result.task_id);

            // Remove from pending
            self.session_context.write().await.pending_minime.retain(|id| *id != result.task_id);

            if result.task_id == task_id {
                return Some(result);
            }
        }
        None
    }

    /// Collect all pending results (non-blocking)
    pub async fn collect_results(&self) -> Vec<MiniMeResult> {
        let mut results = vec![];
        let mut rx = self.result_rx.write().await;

        while let Ok(result) = rx.try_recv() {
            self.results.write().await.insert(result.task_id, result.clone());
            self.active_tasks.write().await.remove(&result.task_id);
            self.session_context.write().await.pending_minime.retain(|id| *id != result.task_id);
            results.push(result);
        }

        results
    }

    /// Update session context with planner output
    pub async fn update_plan(&self, goal: &str, steps: Vec<String>) {
        let mut ctx = self.session_context.write().await;
        ctx.primary_goal = goal.to_string();
        ctx.current_plan = steps.into_iter().enumerate().map(|(i, desc)| {
            PlanStep {
                id: i,
                description: desc,
                status: StepStatus::Pending,
                assigned_to: None,
            }
        }).collect();
    }

    /// Mark a step as delegated to Mini-Me
    pub async fn delegate_step(&self, step_id: usize, minime_id: Uuid) {
        let mut ctx = self.session_context.write().await;
        if let Some(step) = ctx.current_plan.get_mut(step_id) {
            step.status = StepStatus::Delegated;
            step.assigned_to = Some(minime_id);
        }
    }

    /// Get session summary for context
    pub async fn get_summary(&self) -> String {
        let ctx = self.session_context.read().await;
        format!(
            "Session: {}\nGoal: {}\nCompleted: {}/{}\nPending Mini-Me: {}\n\nDecisions:\n{}",
            ctx.session_id,
            ctx.primary_goal,
            ctx.completed_steps.len(),
            ctx.current_plan.len(),
            ctx.pending_minime.len(),
            ctx.key_decisions.join("\n- "),
        )
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_configs() {
        let beast = ProviderConfig::lm_studio_beast();
        assert_eq!(beast.tier, ModelTier::Capable);
        assert_eq!(beast.cost_per_1k_tokens, 0.0);

        let sonnet = ProviderConfig::anthropic_sonnet();
        assert_eq!(sonnet.tier, ModelTier::Cloud);
        assert!(sonnet.cost_per_1k_tokens > 0.0);
    }

    #[test]
    fn test_forked_context() {
        let orch = Orchestrator::new();
        let ctx = orch.fork_context("Find all TODO comments", vec!["src/main.rs".into()]);
        assert_eq!(ctx.goal, "Find all TODO comments");
        assert!(!ctx.allowed_tools.is_empty());
    }

    #[tokio::test]
    async fn test_orchestrator_new() {
        let orch = Orchestrator::new();
        assert!(!orch.providers.is_empty());
        assert!(orch.get_provider(ModelTier::Capable).is_some());
    }
}
