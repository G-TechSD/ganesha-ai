//! # Mini-Me Subagent System
//!
//! Enables parallel task execution using smaller/cheaper models for subtasks.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────┐
//! │                          SubAgentManager                             │
//! │                                                                      │
//! │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐        │
//! │  │ SubAgent  │  │ SubAgent  │  │ SubAgent  │  │    ...    │        │
//! │  │ (Haiku)   │  │ (GPT-mini)│  │ (Llama8B) │  │ (up to 10)│        │
//! │  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └───────────┘        │
//! │        │              │              │                               │
//! │        └──────────────┴──────────────┴───────────────────────────►  │
//! │                          Progress Channel                            │
//! └──────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Features
//!
//! - Pool of up to 10 concurrent subagents
//! - Model selection based on task complexity
//! - Automatic escalation on failure
//! - Cost and token tracking
//! - Progress updates via channels
//! - Task splitting and result aggregation

use crate::{CoreError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ganesha_providers::{GenerateOptions, Message, ModelTier, ProviderManager, Usage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// ============================================================================
// SubAgent Types
// ============================================================================

/// Unique identifier for a subagent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(Uuid);

impl AgentId {
    /// Create a new unique agent ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a subagent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is idle and ready for work
    Idle,
    /// Agent is currently working on a task
    Working,
    /// Agent has completed its task successfully
    Completed,
    /// Agent has failed its task
    Failed,
    /// Agent was cancelled
    Cancelled,
}

impl AgentStatus {
    /// Check if the agent has finished (completed, failed, or cancelled)
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// A subagent that performs a specific task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgent {
    /// Unique identifier
    pub id: AgentId,
    /// Human-readable name
    pub name: String,
    /// The task this agent is assigned to
    pub assigned_task: String,
    /// Model to use (typically smaller/faster)
    pub model: String,
    /// Current status
    pub status: AgentStatus,
    /// Input context provided to the agent
    pub input_context: String,
    /// Output results from the agent
    pub output_result: Option<String>,
    /// When the agent was spawned
    pub spawn_time: DateTime<Utc>,
    /// When the agent completed
    pub completion_time: Option<DateTime<Utc>>,
    /// Token usage
    #[serde(default)]
    pub token_usage: TokenUsage,
    /// Cost incurred (in USD cents)
    pub cost_cents: f64,
    /// Number of retries attempted
    pub retry_count: u32,
    /// Error message if failed
    pub error_message: Option<String>,
}

impl SubAgent {
    /// Create a new subagent
    pub fn new(name: impl Into<String>, task: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            id: AgentId::new(),
            name: name.into(),
            assigned_task: task.into(),
            model: model.into(),
            status: AgentStatus::Idle,
            input_context: String::new(),
            output_result: None,
            spawn_time: Utc::now(),
            completion_time: None,
            token_usage: TokenUsage::default(),
            cost_cents: 0.0,
            retry_count: 0,
            error_message: None,
        }
    }

    /// Set input context
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.input_context = context.into();
        self
    }

    /// Get the duration the agent ran for
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.completion_time.map(|end| end - self.spawn_time)
    }

    /// Mark the agent as working
    pub fn start(&mut self) {
        self.status = AgentStatus::Working;
    }

    /// Mark the agent as completed with a result
    pub fn complete(&mut self, result: String) {
        self.status = AgentStatus::Completed;
        self.output_result = Some(result);
        self.completion_time = Some(Utc::now());
    }

    /// Mark the agent as failed with an error
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = AgentStatus::Failed;
        self.error_message = Some(error.into());
        self.completion_time = Some(Utc::now());
    }

    /// Mark the agent as cancelled
    pub fn cancel(&mut self) {
        self.status = AgentStatus::Cancelled;
        self.completion_time = Some(Utc::now());
    }
}

/// Token usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Add usage from a provider response
    pub fn add(&mut self, usage: &Usage) {
        self.prompt_tokens += usage.prompt_tokens;
        self.completion_tokens += usage.completion_tokens;
        self.total_tokens += usage.total_tokens;
    }
}

// ============================================================================
// Agent Handle
// ============================================================================

/// Handle to a spawned subagent for tracking and control
#[derive(Debug)]
pub struct AgentHandle {
    /// The agent ID
    pub id: AgentId,
    /// Receiver for the final result
    result_rx: Option<oneshot::Receiver<AgentResult>>,
    /// Sender to cancel the agent
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl AgentHandle {
    /// Get the agent ID
    pub fn agent_id(&self) -> AgentId {
        self.id
    }

    /// Wait for the agent to complete and get the result
    pub async fn wait(mut self) -> AgentResult {
        match self.result_rx.take() {
            Some(rx) => rx.await.unwrap_or_else(|_| AgentResult {
                agent_id: self.id,
                success: false,
                output: None,
                error: Some("Agent channel closed unexpectedly".to_string()),
                token_usage: TokenUsage::default(),
                cost_cents: 0.0,
                duration_ms: 0,
            }),
            None => AgentResult {
                agent_id: self.id,
                success: false,
                output: None,
                error: Some("Result already consumed".to_string()),
                token_usage: TokenUsage::default(),
                cost_cents: 0.0,
                duration_ms: 0,
            },
        }
    }

    /// Cancel the agent
    pub fn cancel(mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Result from a completed agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// The agent that produced this result
    pub agent_id: AgentId,
    /// Whether the task succeeded
    pub success: bool,
    /// Output if successful
    pub output: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Token usage
    pub token_usage: TokenUsage,
    /// Cost in USD cents
    pub cost_cents: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

// ============================================================================
// Progress Updates
// ============================================================================

/// Progress update from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Agent that sent the update
    pub agent_id: AgentId,
    /// Agent name
    pub agent_name: String,
    /// Type of update
    pub update_type: ProgressType,
    /// Human-readable message
    pub message: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Type of progress update
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressType {
    /// Agent started
    Started,
    /// Agent is working
    Working,
    /// Agent made progress (with percentage if available)
    Progress { percent: Option<u8> },
    /// Agent completed successfully
    Completed,
    /// Agent failed
    Failed,
    /// Agent was cancelled
    Cancelled,
    /// Agent is escalating to a larger model
    Escalating,
}

// ============================================================================
// SubAgent Manager
// ============================================================================

/// Maximum number of concurrent subagents
pub const MAX_CONCURRENT_AGENTS: usize = 10;

/// Default timeout for agent tasks
pub const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Manages a pool of subagents for parallel task execution
pub struct SubAgentManager {
    /// Provider manager for LLM access
    provider_manager: Arc<ProviderManager>,
    /// Semaphore to limit concurrent agents
    semaphore: Arc<Semaphore>,
    /// Active agents
    active_agents: Arc<RwLock<HashMap<AgentId, SubAgent>>>,
    /// Progress update sender
    progress_tx: mpsc::Sender<ProgressUpdate>,
    /// Progress update receiver (owned by consumer)
    progress_rx: Mutex<Option<mpsc::Receiver<ProgressUpdate>>>,
    /// Total cost tracking (stored as cents * 100 for precision)
    total_cost_cents: Arc<AtomicU64>,
    /// Total tokens used
    total_tokens: Arc<AtomicU64>,
    /// Default timeout in seconds
    default_timeout: Duration,
    /// Model escalation enabled
    escalation_enabled: bool,
}

impl SubAgentManager {
    /// Create a new subagent manager
    pub fn new(provider_manager: Arc<ProviderManager>) -> Self {
        let (progress_tx, progress_rx) = mpsc::channel(100);
        Self {
            provider_manager,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_AGENTS)),
            active_agents: Arc::new(RwLock::new(HashMap::new())),
            progress_tx,
            progress_rx: Mutex::new(Some(progress_rx)),
            total_cost_cents: Arc::new(AtomicU64::new(0)),
            total_tokens: Arc::new(AtomicU64::new(0)),
            default_timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            escalation_enabled: true,
        }
    }

    /// Set the default timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Enable or disable model escalation on failure
    pub fn with_escalation(mut self, enabled: bool) -> Self {
        self.escalation_enabled = enabled;
        self
    }

    /// Take ownership of the progress receiver
    pub async fn take_progress_receiver(&self) -> Option<mpsc::Receiver<ProgressUpdate>> {
        self.progress_rx.lock().await.take()
    }

    /// Spawn a new subagent for a task
    pub async fn spawn_agent(
        &self,
        task: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<AgentHandle> {
        self.spawn_agent_with_context(task, model, "").await
    }

    /// Spawn a new subagent with context
    pub async fn spawn_agent_with_context(
        &self,
        task: impl Into<String>,
        model: impl Into<String>,
        context: impl Into<String>,
    ) -> Result<AgentHandle> {
        let task = task.into();
        let model = model.into();
        let context = context.into();

        // Create the agent
        let agent_name = format!("agent-{}", &Uuid::new_v4().to_string()[..8]);
        let mut agent = SubAgent::new(&agent_name, &task, &model).with_context(&context);

        let agent_id = agent.id;

        // Set up channels
        let (result_tx, result_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        // Store agent
        {
            let mut agents = self.active_agents.write().await;
            agents.insert(agent_id, agent.clone());
        }

        // Clone what we need for the spawned task
        let provider_manager = self.provider_manager.clone();
        let semaphore = self.semaphore.clone();
        let active_agents = self.active_agents.clone();
        let progress_tx = self.progress_tx.clone();
        let timeout_duration = self.default_timeout;
        let escalation_enabled = self.escalation_enabled;
        let total_cost = self.total_cost_cents.clone();
        let total_tokens = self.total_tokens.clone();

        // Spawn the agent task
        tokio::spawn(async move {
            // Acquire semaphore permit
            let _permit = semaphore.acquire().await;

            // Update status to working
            {
                let mut agents = active_agents.write().await;
                if let Some(a) = agents.get_mut(&agent_id) {
                    a.start();
                    agent = a.clone();
                }
            }

            // Send started progress
            let _ = progress_tx
                .send(ProgressUpdate {
                    agent_id,
                    agent_name: agent.name.clone(),
                    update_type: ProgressType::Started,
                    message: format!("Starting task: {}", task),
                    timestamp: Utc::now(),
                })
                .await;

            // Execute the task with timeout and cancellation
            let result = tokio::select! {
                _ = async {
                    if let Ok(()) = cancel_rx.await {
                        // Cancelled
                    }
                } => {
                    let _ = progress_tx.send(ProgressUpdate {
                        agent_id,
                        agent_name: agent.name.clone(),
                        update_type: ProgressType::Cancelled,
                        message: "Agent cancelled".to_string(),
                        timestamp: Utc::now(),
                    }).await;

                    AgentResult {
                        agent_id,
                        success: false,
                        output: None,
                        error: Some("Cancelled".to_string()),
                        token_usage: TokenUsage::default(),
                        cost_cents: 0.0,
                        duration_ms: 0,
                    }
                }
                result = timeout(timeout_duration, execute_agent_task(
                    &provider_manager,
                    &agent,
                    &progress_tx,
                    escalation_enabled,
                )) => {
                    match result {
                        Ok(r) => r,
                        Err(_) => {
                            let _ = progress_tx.send(ProgressUpdate {
                                agent_id,
                                agent_name: agent.name.clone(),
                                update_type: ProgressType::Failed,
                                message: "Agent timed out".to_string(),
                                timestamp: Utc::now(),
                            }).await;

                            AgentResult {
                                agent_id,
                                success: false,
                                output: None,
                                error: Some(format!("Timeout after {} seconds", timeout_duration.as_secs())),
                                token_usage: TokenUsage::default(),
                                cost_cents: 0.0,
                                duration_ms: timeout_duration.as_millis() as u64,
                            }
                        }
                    }
                }
            };

            // Update totals
            total_cost.fetch_add((result.cost_cents * 100.0) as u64, Ordering::Relaxed);
            total_tokens.fetch_add(result.token_usage.total_tokens as u64, Ordering::Relaxed);

            // Update final agent state
            {
                let mut agents = active_agents.write().await;
                if let Some(a) = agents.get_mut(&agent_id) {
                    if result.success {
                        a.complete(result.output.clone().unwrap_or_default());
                    } else {
                        a.fail(result.error.clone().unwrap_or_default());
                    }
                    a.token_usage = result.token_usage.clone();
                    a.cost_cents = result.cost_cents;
                }
            }

            // Send result
            let _ = result_tx.send(result);
        });

        Ok(AgentHandle {
            id: agent_id,
            result_rx: Some(result_rx),
            cancel_tx: Some(cancel_tx),
        })
    }

    /// Wait for a specific agent to complete
    pub async fn wait_for_agent(&self, handle: AgentHandle) -> Result<AgentResult> {
        Ok(handle.wait().await)
    }

    /// Wait for all active agents to complete
    pub async fn wait_for_all(&self, handles: Vec<AgentHandle>) -> Vec<AgentResult> {
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(handle.wait().await);
        }
        results
    }

    /// Cancel a specific agent
    pub fn cancel_agent(&self, handle: AgentHandle) {
        handle.cancel();
    }

    /// Get the status of all agents
    pub async fn get_all_agents(&self) -> Vec<SubAgent> {
        let agents = self.active_agents.read().await;
        agents.values().cloned().collect()
    }

    /// Get a specific agent by ID
    pub async fn get_agent(&self, id: AgentId) -> Option<SubAgent> {
        let agents = self.active_agents.read().await;
        agents.get(&id).cloned()
    }

    /// Get total cost in cents
    pub fn total_cost_cents(&self) -> f64 {
        self.total_cost_cents.load(Ordering::Relaxed) as f64 / 100.0
    }

    /// Get total tokens used
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens.load(Ordering::Relaxed)
    }

    /// Get number of currently active agents
    pub async fn active_count(&self) -> usize {
        let agents = self.active_agents.read().await;
        agents
            .values()
            .filter(|a| matches!(a.status, AgentStatus::Working))
            .count()
    }

    /// Clean up completed agents
    pub async fn cleanup_completed(&self) {
        let mut agents = self.active_agents.write().await;
        agents.retain(|_, a| !a.status.is_finished());
    }
}

/// Execute the agent's task
async fn execute_agent_task(
    provider_manager: &ProviderManager,
    agent: &SubAgent,
    progress_tx: &mpsc::Sender<ProgressUpdate>,
    escalation_enabled: bool,
) -> AgentResult {
    let start = std::time::Instant::now();
    let mut token_usage = TokenUsage::default();
    let mut cost_cents = 0.0;
    let mut current_model = agent.model.clone();
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 3;

    loop {
        attempts += 1;

        // Build the prompt
        let system_prompt = format!(
            "You are a focused assistant performing a specific subtask. \
            Complete the following task concisely and accurately.\n\n\
            Context:\n{}\n\n\
            Respond with only the result, no explanations unless necessary.",
            agent.input_context
        );

        let messages = vec![
            Message::system(&system_prompt),
            Message::user(&agent.assigned_task),
        ];

        let options = GenerateOptions {
            model: Some(current_model.clone()),
            temperature: Some(0.3), // Lower temperature for more focused output
            max_tokens: Some(2048),
            ..Default::default()
        };

        // Send working progress
        let _ = progress_tx
            .send(ProgressUpdate {
                agent_id: agent.id,
                agent_name: agent.name.clone(),
                update_type: ProgressType::Working,
                message: format!(
                    "Attempt {} with model {}",
                    attempts, current_model
                ),
                timestamp: Utc::now(),
            })
            .await;

        // Execute
        match provider_manager.chat(&messages, &options).await {
            Ok(response) => {
                // Track usage
                if let Some(usage) = &response.usage {
                    token_usage.add(usage);
                    cost_cents += calculate_cost(&current_model, usage);
                }

                let _ = progress_tx
                    .send(ProgressUpdate {
                        agent_id: agent.id,
                        agent_name: agent.name.clone(),
                        update_type: ProgressType::Completed,
                        message: "Task completed successfully".to_string(),
                        timestamp: Utc::now(),
                    })
                    .await;

                return AgentResult {
                    agent_id: agent.id,
                    success: true,
                    output: Some(response.content),
                    error: None,
                    token_usage,
                    cost_cents,
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
            Err(e) => {
                warn!(
                    "Agent {} failed with model {}: {}",
                    agent.name, current_model, e
                );

                if attempts >= MAX_ATTEMPTS {
                    let _ = progress_tx
                        .send(ProgressUpdate {
                            agent_id: agent.id,
                            agent_name: agent.name.clone(),
                            update_type: ProgressType::Failed,
                            message: format!("Failed after {} attempts: {}", attempts, e),
                            timestamp: Utc::now(),
                        })
                        .await;

                    return AgentResult {
                        agent_id: agent.id,
                        success: false,
                        output: None,
                        error: Some(e.to_string()),
                        token_usage,
                        cost_cents,
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }

                // Try escalation if enabled
                if escalation_enabled {
                    if let Some(better_model) = escalate_model(&current_model) {
                        let _ = progress_tx
                            .send(ProgressUpdate {
                                agent_id: agent.id,
                                agent_name: agent.name.clone(),
                                update_type: ProgressType::Escalating,
                                message: format!(
                                    "Escalating from {} to {}",
                                    current_model, better_model
                                ),
                                timestamp: Utc::now(),
                            })
                            .await;
                        current_model = better_model;
                    }
                }
            }
        }
    }
}

/// Calculate cost for a model based on usage
fn calculate_cost(model: &str, usage: &Usage) -> f64 {
    // Approximate costs per 1M tokens (in cents)
    let (input_cost, output_cost): (f64, f64) = if model.contains("haiku") {
        (0.25, 1.25) // Claude Haiku
    } else if model.contains("gpt-4o-mini") {
        (0.15, 0.60) // GPT-4o-mini
    } else if model.contains("gpt-4o") {
        (5.0, 15.0) // GPT-4o
    } else if model.contains("sonnet") {
        (3.0, 15.0) // Claude Sonnet
    } else if model.contains("opus") {
        (15.0, 75.0) // Claude Opus
    } else if model.contains("gemini-flash") {
        (0.075, 0.30) // Gemini Flash
    } else if model.contains("llama") || model.contains("mistral") {
        (0.0, 0.0) // Local/free models
    } else {
        (1.0, 3.0) // Default estimate
    };

    let input_tokens = usage.prompt_tokens as f64 / 1_000_000.0;
    let output_tokens = usage.completion_tokens as f64 / 1_000_000.0;

    input_cost * input_tokens + output_cost * output_tokens
}

/// Escalate to a more capable model
fn escalate_model(current: &str) -> Option<String> {
    let current_lower = current.to_lowercase();

    // Escalation paths
    if current_lower.contains("haiku") || current_lower.contains("gpt-4o-mini") {
        // Small -> Medium
        if current_lower.contains("haiku") {
            Some("claude-3-5-sonnet-latest".to_string())
        } else {
            Some("gpt-4o".to_string())
        }
    } else if current_lower.contains("flash") {
        Some("gemini-1.5-pro".to_string())
    } else if current_lower.contains("8b") || current_lower.contains("7b") {
        // Small local -> Medium local
        Some("llama-3.1-70b".to_string())
    } else {
        // Already at a capable model, no escalation
        None
    }
}

// ============================================================================
// Task Distribution
// ============================================================================

/// A unit of work to be performed by a subagent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    /// Unique identifier
    pub id: String,
    /// Description of the work to be done
    pub description: String,
    /// Context needed for the work
    pub context: String,
    /// Expected type of output
    pub expected_output_type: OutputType,
    /// Dependencies (IDs of other work items that must complete first)
    pub dependencies: Vec<String>,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Recommended model tier for this work
    pub recommended_tier: ModelTier,
}

impl WorkItem {
    /// Create a new work item
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            description: description.into(),
            context: String::new(),
            expected_output_type: OutputType::Text,
            dependencies: Vec::new(),
            priority: 100,
            recommended_tier: ModelTier::Capable,
        }
    }

    /// Set the context
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = context.into();
        self
    }

    /// Set the expected output type
    pub fn with_output_type(mut self, output_type: OutputType) -> Self {
        self.expected_output_type = output_type;
        self
    }

    /// Add a dependency
    pub fn depends_on(mut self, dependency_id: impl Into<String>) -> Self {
        self.dependencies.push(dependency_id.into());
        self
    }

    /// Set the priority
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the recommended model tier
    pub fn with_tier(mut self, tier: ModelTier) -> Self {
        self.recommended_tier = tier;
        self
    }
}

/// Expected output type from a work item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputType {
    /// Plain text output
    Text,
    /// JSON structured output
    Json,
    /// Code output
    Code,
    /// List of items
    List,
    /// Boolean yes/no answer
    Boolean,
    /// File paths
    FilePaths,
}

/// Trait for splitting a complex task into subtasks
#[async_trait]
pub trait TaskSplitter: Send + Sync {
    /// Split a complex task into work items
    async fn split(&self, task: &str, context: &str) -> Result<Vec<WorkItem>>;

    /// Get the name of this splitter
    fn name(&self) -> &str;
}

/// Trait for aggregating results from multiple work items
#[async_trait]
pub trait ResultAggregator: Send + Sync {
    /// Aggregate results from completed work items
    async fn aggregate(&self, results: &[(WorkItem, AgentResult)]) -> Result<String>;

    /// Get the name of this aggregator
    fn name(&self) -> &str;
}

/// Default task splitter that uses an LLM to split tasks
pub struct LlmTaskSplitter {
    provider_manager: Arc<ProviderManager>,
}

impl LlmTaskSplitter {
    pub fn new(provider_manager: Arc<ProviderManager>) -> Self {
        Self { provider_manager }
    }
}

#[async_trait]
impl TaskSplitter for LlmTaskSplitter {
    async fn split(&self, task: &str, context: &str) -> Result<Vec<WorkItem>> {
        let system = "You are a task decomposition expert. Break down the given task into \
            smaller, independent subtasks that can be executed in parallel where possible. \
            Output a JSON array of subtasks, each with: description, dependencies (array of indices), \
            priority (1-10, lower is higher priority), and complexity (simple/medium/complex).";

        let user = format!(
            "Task: {}\n\nContext:\n{}\n\nDecompose this into subtasks (JSON array):",
            task, context
        );

        let options = GenerateOptions {
            json_mode: true,
            temperature: Some(0.3),
            ..Default::default()
        };

        let messages = vec![Message::system(system), Message::user(&user)];

        let response = self
            .provider_manager
            .chat(&messages, &options)
            .await
            .map_err(|e| CoreError::MiniMeError(e.to_string()))?;

        // Parse the JSON response
        let subtasks: Vec<serde_json::Value> = serde_json::from_str(&response.content)
            .map_err(|e| CoreError::MiniMeError(format!("Failed to parse subtasks: {}", e)))?;

        let mut work_items: Vec<WorkItem> = Vec::new();
        for (i, subtask) in subtasks.iter().enumerate() {
            let description = subtask["description"]
                .as_str()
                .unwrap_or("Unknown task")
                .to_string();

            let complexity = subtask["complexity"].as_str().unwrap_or("medium");
            let tier = match complexity {
                "simple" => ModelTier::Limited,
                "complex" => ModelTier::Exceptional,
                _ => ModelTier::Capable,
            };

            let priority = subtask["priority"].as_u64().unwrap_or(5) as u32;

            let mut item = WorkItem::new(description)
                .with_context(context)
                .with_priority(priority)
                .with_tier(tier);

            // Handle dependencies
            if let Some(deps) = subtask["dependencies"].as_array() {
                for dep in deps {
                    if let Some(dep_idx) = dep.as_u64() {
                        if dep_idx < i as u64 {
                            if let Some(dep_item) = work_items.get(dep_idx as usize) {
                                item = item.depends_on(dep_item.id.clone());
                            }
                        }
                    }
                }
            }

            work_items.push(item);
        }

        Ok(work_items)
    }

    fn name(&self) -> &str {
        "llm-splitter"
    }
}

/// Default result aggregator that concatenates results
pub struct SimpleAggregator;

#[async_trait]
impl ResultAggregator for SimpleAggregator {
    async fn aggregate(&self, results: &[(WorkItem, AgentResult)]) -> Result<String> {
        let mut output = String::new();

        for (item, result) in results {
            output.push_str(&format!("## {}\n", item.description));
            if result.success {
                if let Some(ref out) = result.output {
                    output.push_str(out);
                }
            } else {
                output.push_str(&format!(
                    "Failed: {}",
                    result.error.as_deref().unwrap_or("Unknown error")
                ));
            }
            output.push_str("\n\n");
        }

        Ok(output)
    }

    fn name(&self) -> &str {
        "simple-aggregator"
    }
}

/// LLM-based aggregator that synthesizes results
pub struct LlmAggregator {
    provider_manager: Arc<ProviderManager>,
}

impl LlmAggregator {
    pub fn new(provider_manager: Arc<ProviderManager>) -> Self {
        Self { provider_manager }
    }
}

#[async_trait]
impl ResultAggregator for LlmAggregator {
    async fn aggregate(&self, results: &[(WorkItem, AgentResult)]) -> Result<String> {
        let mut input = String::new();
        input.push_str("Results from subtasks:\n\n");

        for (item, result) in results {
            input.push_str(&format!("### Task: {}\n", item.description));
            input.push_str(&format!("Status: {}\n", if result.success { "Success" } else { "Failed" }));
            if let Some(ref out) = result.output {
                input.push_str(&format!("Output:\n{}\n", out));
            }
            if let Some(ref err) = result.error {
                input.push_str(&format!("Error: {}\n", err));
            }
            input.push('\n');
        }

        let system = "You are a result synthesizer. Combine the results from multiple subtasks \
            into a coherent, unified response. Highlight any failures or issues.";

        let response = self
            .provider_manager
            .generate(system, &input)
            .await
            .map_err(|e| CoreError::MiniMeError(e.to_string()))?;

        Ok(response)
    }

    fn name(&self) -> &str {
        "llm-aggregator"
    }
}

// ============================================================================
// Model Selection
// ============================================================================

/// Selects appropriate models for tasks based on complexity
pub struct ModelSelector {
    /// Fast models for simple tasks
    pub fast_models: Vec<String>,
    /// Capable models for medium tasks
    pub capable_models: Vec<String>,
    /// Powerful models for complex tasks
    pub powerful_models: Vec<String>,
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self {
            fast_models: vec![
                "claude-3-5-haiku-latest".to_string(),
                "gpt-4o-mini".to_string(),
                "gemini-1.5-flash".to_string(),
            ],
            capable_models: vec![
                "claude-3-5-sonnet-latest".to_string(),
                "gpt-4o".to_string(),
                "gemini-1.5-pro".to_string(),
            ],
            powerful_models: vec![
                "claude-3-opus-latest".to_string(),
                "gpt-4-turbo".to_string(),
            ],
        }
    }
}

impl ModelSelector {
    /// Create a new model selector with custom models
    pub fn new(fast: Vec<String>, capable: Vec<String>, powerful: Vec<String>) -> Self {
        Self {
            fast_models: fast,
            capable_models: capable,
            powerful_models: powerful,
        }
    }

    /// Select a model based on tier
    pub fn select(&self, tier: ModelTier) -> &str {
        match tier {
            ModelTier::Limited | ModelTier::Unsafe => {
                self.fast_models.first().map(|s| s.as_str()).unwrap_or("gpt-4o-mini")
            }
            ModelTier::Capable | ModelTier::Unknown => {
                self.capable_models.first().map(|s| s.as_str()).unwrap_or("gpt-4o")
            }
            ModelTier::Exceptional => {
                self.powerful_models.first().map(|s| s.as_str()).unwrap_or("claude-3-opus")
            }
        }
    }

    /// Select a fast model
    pub fn select_fast(&self) -> &str {
        self.fast_models.first().map(|s| s.as_str()).unwrap_or("gpt-4o-mini")
    }

    /// Select a capable model
    pub fn select_capable(&self) -> &str {
        self.capable_models.first().map(|s| s.as_str()).unwrap_or("gpt-4o")
    }

    /// Select a powerful model
    pub fn select_powerful(&self) -> &str {
        self.powerful_models.first().map(|s| s.as_str()).unwrap_or("claude-3-opus")
    }
}

// ============================================================================
// Common Subtask Agent Types
// ============================================================================

/// Agent specialization for common subtask types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    /// Find relevant files in a codebase
    FileSearch,
    /// Analyze code structure and patterns
    CodeAnalysis,
    /// Run and interpret tests
    TestRunner,
    /// Generate documentation
    Documentation,
    /// Apply refactoring patterns
    Refactor,
    /// General purpose agent
    General,
}

impl AgentType {
    /// Get the system prompt for this agent type
    pub fn system_prompt(&self) -> &'static str {
        match self {
            Self::FileSearch => {
                "You are a file search specialist. Given a description of what to find, \
                identify relevant file paths. Output a JSON array of file paths."
            }
            Self::CodeAnalysis => {
                "You are a code analysis expert. Analyze the given code and provide \
                structured insights about its architecture, patterns, and potential issues. \
                Be thorough but concise."
            }
            Self::TestRunner => {
                "You are a test analysis expert. Interpret test results and provide \
                clear summaries of what passed, failed, and why. Suggest fixes for failures."
            }
            Self::Documentation => {
                "You are a documentation specialist. Generate clear, comprehensive \
                documentation for the given code or API. Follow standard documentation \
                conventions."
            }
            Self::Refactor => {
                "You are a refactoring expert. Apply the requested refactoring pattern \
                to the given code. Output the refactored code with explanations of changes."
            }
            Self::General => {
                "You are a helpful assistant. Complete the requested task accurately and concisely."
            }
        }
    }

    /// Get the recommended model tier for this agent type
    pub fn recommended_tier(&self) -> ModelTier {
        match self {
            Self::FileSearch => ModelTier::Limited,
            Self::CodeAnalysis => ModelTier::Capable,
            Self::TestRunner => ModelTier::Capable,
            Self::Documentation => ModelTier::Limited,
            Self::Refactor => ModelTier::Exceptional,
            Self::General => ModelTier::Capable,
        }
    }
}

/// Specialized agent builder for common subtask types
pub struct SpecializedAgentBuilder {
    agent_type: AgentType,
    task: String,
    context: String,
    model_override: Option<String>,
}

impl SpecializedAgentBuilder {
    /// Create a new specialized agent builder
    pub fn new(agent_type: AgentType) -> Self {
        Self {
            agent_type,
            task: String::new(),
            context: String::new(),
            model_override: None,
        }
    }

    /// Create a file search agent
    pub fn file_search() -> Self {
        Self::new(AgentType::FileSearch)
    }

    /// Create a code analysis agent
    pub fn code_analysis() -> Self {
        Self::new(AgentType::CodeAnalysis)
    }

    /// Create a test runner agent
    pub fn test_runner() -> Self {
        Self::new(AgentType::TestRunner)
    }

    /// Create a documentation agent
    pub fn documentation() -> Self {
        Self::new(AgentType::Documentation)
    }

    /// Create a refactor agent
    pub fn refactor() -> Self {
        Self::new(AgentType::Refactor)
    }

    /// Set the task
    pub fn with_task(mut self, task: impl Into<String>) -> Self {
        self.task = task.into();
        self
    }

    /// Set the context
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = context.into();
        self
    }

    /// Override the model selection
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model_override = Some(model.into());
        self
    }

    /// Build the work item
    pub fn build(self) -> WorkItem {
        let full_task = format!(
            "{}\n\n{}",
            self.agent_type.system_prompt(),
            self.task
        );

        WorkItem::new(full_task)
            .with_context(self.context)
            .with_tier(self.agent_type.recommended_tier())
    }

    /// Spawn the agent using a manager
    pub async fn spawn(self, manager: &SubAgentManager, selector: &ModelSelector) -> Result<AgentHandle> {
        let model = self
            .model_override
            .clone()
            .unwrap_or_else(|| selector.select(self.agent_type.recommended_tier()).to_string());

        let full_task = format!(
            "{}\n\nTask: {}",
            self.agent_type.system_prompt(),
            self.task
        );

        manager
            .spawn_agent_with_context(full_task, model, self.context)
            .await
    }
}

// ============================================================================
// Orchestrator
// ============================================================================

/// High-level orchestrator for parallel task execution
pub struct TaskOrchestrator {
    manager: Arc<SubAgentManager>,
    splitter: Arc<dyn TaskSplitter>,
    aggregator: Arc<dyn ResultAggregator>,
    model_selector: ModelSelector,
}

impl TaskOrchestrator {
    /// Create a new task orchestrator
    pub fn new(
        manager: Arc<SubAgentManager>,
        splitter: Arc<dyn TaskSplitter>,
        aggregator: Arc<dyn ResultAggregator>,
    ) -> Self {
        Self {
            manager,
            splitter,
            aggregator,
            model_selector: ModelSelector::default(),
        }
    }

    /// Set a custom model selector
    pub fn with_model_selector(mut self, selector: ModelSelector) -> Self {
        self.model_selector = selector;
        self
    }

    /// Execute a complex task by splitting, running in parallel, and aggregating
    pub async fn execute(&self, task: &str, context: &str) -> Result<String> {
        info!("Orchestrating task: {}", task);

        // Split the task
        let work_items = self.splitter.split(task, context).await?;
        info!("Split into {} work items", work_items.len());

        // Track completed items and their results
        let mut completed: HashMap<String, AgentResult> = HashMap::new();
        let mut pending: Vec<WorkItem> = work_items.clone();
        let mut handles: HashMap<String, AgentHandle> = HashMap::new();

        // Process items respecting dependencies
        while !pending.is_empty() || !handles.is_empty() {
            // Find items that can be started (all dependencies satisfied)
            let ready: Vec<WorkItem> = pending
                .iter()
                .filter(|item| {
                    item.dependencies
                        .iter()
                        .all(|dep| completed.contains_key(dep))
                })
                .cloned()
                .collect();

            // Remove ready items from pending
            pending.retain(|item| !ready.iter().any(|r| r.id == item.id));

            // Spawn agents for ready items
            for item in ready {
                let model = self.model_selector.select(item.recommended_tier).to_string();
                debug!("Spawning agent for: {} with model {}", item.description, model);

                match self
                    .manager
                    .spawn_agent_with_context(&item.description, &model, &item.context)
                    .await
                {
                    Ok(handle) => {
                        handles.insert(item.id.clone(), handle);
                    }
                    Err(e) => {
                        error!("Failed to spawn agent for {}: {}", item.description, e);
                        completed.insert(
                            item.id.clone(),
                            AgentResult {
                                agent_id: AgentId::new(),
                                success: false,
                                output: None,
                                error: Some(e.to_string()),
                                token_usage: TokenUsage::default(),
                                cost_cents: 0.0,
                                duration_ms: 0,
                            },
                        );
                    }
                }
            }

            // Wait for at least one handle to complete
            if !handles.is_empty() {
                // For simplicity, wait for all current handles
                // A more sophisticated implementation would use select! to process as they complete
                let current_handles: Vec<(String, AgentHandle)> =
                    handles.drain().collect();

                for (id, handle) in current_handles {
                    let result = handle.wait().await;
                    completed.insert(id, result);
                }
            }
        }

        // Aggregate results in order
        let ordered_results: Vec<(WorkItem, AgentResult)> = work_items
            .into_iter()
            .filter_map(|item| {
                completed
                    .remove(&item.id)
                    .map(|result| (item, result))
            })
            .collect();

        self.aggregator.aggregate(&ordered_results).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_creation() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_agent_status_is_finished() {
        assert!(!AgentStatus::Idle.is_finished());
        assert!(!AgentStatus::Working.is_finished());
        assert!(AgentStatus::Completed.is_finished());
        assert!(AgentStatus::Failed.is_finished());
        assert!(AgentStatus::Cancelled.is_finished());
    }

    #[test]
    fn test_subagent_lifecycle() {
        let mut agent = SubAgent::new("test", "do something", "gpt-4o-mini");
        assert_eq!(agent.status, AgentStatus::Idle);

        agent.start();
        assert_eq!(agent.status, AgentStatus::Working);

        agent.complete("result".to_string());
        assert_eq!(agent.status, AgentStatus::Completed);
        assert_eq!(agent.output_result, Some("result".to_string()));
        assert!(agent.completion_time.is_some());
    }

    #[test]
    fn test_work_item_builder() {
        let item = WorkItem::new("Test task")
            .with_context("test context")
            .with_priority(1)
            .with_tier(ModelTier::Capable)
            .depends_on("other-id");

        assert_eq!(item.description, "Test task");
        assert_eq!(item.context, "test context");
        assert_eq!(item.priority, 1);
        assert_eq!(item.recommended_tier, ModelTier::Capable);
        assert_eq!(item.dependencies, vec!["other-id".to_string()]);
    }

    #[test]
    fn test_model_selector() {
        let selector = ModelSelector::default();

        assert!(selector.select_fast().contains("haiku") || selector.select_fast().contains("mini"));
        assert!(!selector.select_powerful().is_empty());
    }

    #[test]
    fn test_agent_type_system_prompts() {
        assert!(AgentType::FileSearch.system_prompt().contains("file"));
        assert!(AgentType::CodeAnalysis.system_prompt().contains("analysis"));
        assert!(AgentType::TestRunner.system_prompt().contains("test"));
        assert!(AgentType::Documentation.system_prompt().contains("documentation"));
        assert!(AgentType::Refactor.system_prompt().contains("refactoring"));
    }

    #[test]
    fn test_token_usage_add() {
        let mut usage = TokenUsage::default();
        let provider_usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        usage.add(&provider_usage);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_escalate_model() {
        assert_eq!(
            escalate_model("claude-3-haiku"),
            Some("claude-3-5-sonnet-latest".to_string())
        );
        assert_eq!(
            escalate_model("gpt-4o-mini"),
            Some("gpt-4o".to_string())
        );
        assert_eq!(
            escalate_model("gemini-1.5-flash"),
            Some("gemini-1.5-pro".to_string())
        );
        assert_eq!(escalate_model("claude-3-opus"), None);
    }

    #[test]
    fn test_calculate_cost() {
        let usage = Usage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };

        // Haiku should be cheap
        let haiku_cost = calculate_cost("claude-3-haiku", &usage);
        let opus_cost = calculate_cost("claude-3-opus", &usage);

        assert!(haiku_cost < opus_cost);
    }

    
    #[test]
    fn test_agent_id_unique() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id1.as_uuid(), id2.as_uuid());
    }

    #[test]
    fn test_agent_status_is_finished_idle() {
        assert!(!AgentStatus::Idle.is_finished());
    }

    #[test]
    fn test_agent_status_is_finished_working() {
        assert!(!AgentStatus::Working.is_finished());
    }

    #[test]
    fn test_agent_status_is_finished_completed() {
        assert!(AgentStatus::Completed.is_finished());
    }

    #[test]
    fn test_agent_status_is_finished_failed() {
        assert!(AgentStatus::Failed.is_finished());
    }

    #[test]
    fn test_agent_status_is_finished_cancelled() {
        assert!(AgentStatus::Cancelled.is_finished());
    }

    #[test]
    fn test_subagent_new_fields() {
        let agent = SubAgent::new("coder", "write tests", "claude-3");
        assert_eq!(agent.name, "coder");
        assert_eq!(agent.assigned_task, "write tests");
        assert_eq!(agent.model, "claude-3");
        assert_eq!(agent.status, AgentStatus::Idle);
        assert!(agent.output_result.is_none());
        assert!(agent.completion_time.is_none());
    }

    #[test]
    fn test_subagent_with_context_field() {
        let agent = SubAgent::new("reviewer", "review PR", "gpt-4")
            .with_context("This is a Rust project");
        assert_eq!(agent.input_context, "This is a Rust project");
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_progress_type_variants() {
        let _ = ProgressType::Started;
        let _ = ProgressType::Working;
        let _ = ProgressType::Completed;
        let _ = ProgressType::Failed;
        let _ = ProgressType::Cancelled;
        let _ = ProgressType::Escalating;
        let _ = ProgressType::Progress { percent: Some(50) };
    }


    #[test]
    fn test_agent_id_as_uuid() {
        let id = AgentId::new();
        let uuid = id.as_uuid();
        assert!(!uuid.is_nil());
    }

    #[test]
    fn test_work_item_new() {
        let item = WorkItem::new("test task");
        assert_eq!(item.description, "test task");
    }

    #[test]
    fn test_work_item_with_context() {
        let item = WorkItem::new("task").with_context("ctx");
        assert!(!item.context.is_empty());
    }

    #[test]
    fn test_work_item_with_output_type() {
        let item = WorkItem::new("task").with_output_type(OutputType::Code);
        assert!(matches!(item.expected_output_type, OutputType::Code));
    }

    #[test]
    fn test_output_type_variants() {
        let _ = OutputType::Text;
        let _ = OutputType::Code;
        let _ = OutputType::Json;
    }

    #[test]
    fn test_model_selector_select() {
        let selector = ModelSelector::new(
            vec!["fast-model".to_string()],
            vec!["capable-model".to_string()],
            vec!["powerful-model".to_string()],
        );
        assert_eq!(selector.select_fast(), "fast-model");
    }

    #[test]
    fn test_token_usage_arithmetic() {
        let mut u = TokenUsage::default();
        u.prompt_tokens = 100;
        u.completion_tokens = 50;
        assert_eq!(u.prompt_tokens + u.completion_tokens, 150);
    }
}
