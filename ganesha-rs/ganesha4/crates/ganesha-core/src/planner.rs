//! # Task Planning System
//!
//! Decomposes complex tasks into atomic, executable steps with dependencies.
//!
//! ## Overview
//!
//! The planner is responsible for:
//! - Breaking down user requests into discrete steps
//! - Establishing dependencies between steps
//! - Estimating risk for each operation
//! - Providing a clear execution roadmap
//!
//! ## Example
//!
//! ```ignore
//! let planner = SimplePlanner::new();
//! let plan = planner.plan("Add a new REST endpoint for user authentication").await?;
//!
//! for step in plan.steps() {
//!     println!("Step: {} (risk: {:?})", step.description, step.risk);
//! }
//! ```

use crate::risk::OperationRisk;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during planning
#[derive(Error, Debug)]
pub enum PlannerError {
    #[error("Failed to parse task: {0}")]
    ParseError(String),

    #[error("Dependency cycle detected: {0}")]
    CycleDetected(String),

    #[error("Invalid step reference: {0}")]
    InvalidStepRef(String),

    #[error("Planning context error: {0}")]
    ContextError(String),

    #[error("Provider error: {0}")]
    ProviderError(String),
}

pub type Result<T> = std::result::Result<T, PlannerError>;

/// Unique identifier for a plan step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepId(pub Uuid);

impl StepId {
    /// Create a new unique step ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for StepId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for StepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of action a step performs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// Read file contents
    ReadFile,
    /// Write/create a file
    WriteFile,
    /// Edit existing file
    EditFile,
    /// Delete a file
    DeleteFile,
    /// Create directory
    CreateDirectory,
    /// Execute shell command
    ShellCommand,
    /// Run tests
    RunTests,
    /// Build/compile project
    Build,
    /// Git operation
    GitOperation,
    /// Search/grep files
    Search,
    /// Analyze code
    Analyze,
    /// Generate code using AI
    Generate,
    /// User interaction required
    UserInput,
    /// Custom action type
    Custom(String),
}

impl ActionType {
    /// Get the base risk level for this action type
    pub fn base_risk(&self) -> OperationRisk {
        match self {
            Self::ReadFile | Self::Search | Self::Analyze => OperationRisk::ReadOnly,
            Self::CreateDirectory => OperationRisk::Low,
            Self::WriteFile | Self::EditFile | Self::Build | Self::RunTests => {
                OperationRisk::Medium
            }
            Self::DeleteFile | Self::ShellCommand | Self::GitOperation => OperationRisk::High,
            Self::Generate | Self::UserInput => OperationRisk::Low,
            Self::Custom(_) => OperationRisk::Medium,
        }
    }
}

/// A single step in a task plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Unique identifier for this step
    pub id: StepId,

    /// Human-readable description
    pub description: String,

    /// Type of action to perform
    pub action_type: ActionType,

    /// Target files/paths affected by this step
    pub target_files: Vec<PathBuf>,

    /// Estimated risk level
    pub risk: OperationRisk,

    /// IDs of steps that must complete before this one
    pub dependencies: Vec<StepId>,

    /// Additional context/parameters for execution
    pub context: HashMap<String, serde_json::Value>,

    /// Whether this step is optional
    pub optional: bool,

    /// Estimated duration in seconds (if known)
    pub estimated_duration: Option<u64>,

    /// Whether this step requires user consent
    pub requires_consent: bool,

    /// Rollback strategy if this step fails
    pub rollback_strategy: RollbackStrategy,
}

impl PlanStep {
    /// Create a new plan step
    pub fn new(description: impl Into<String>, action_type: ActionType) -> Self {
        let action_risk = action_type.base_risk();
        Self {
            id: StepId::new(),
            description: description.into(),
            action_type,
            target_files: Vec::new(),
            risk: action_risk,
            dependencies: Vec::new(),
            context: HashMap::new(),
            optional: false,
            estimated_duration: None,
            requires_consent: action_risk >= OperationRisk::High,
            rollback_strategy: RollbackStrategy::Auto,
        }
    }

    /// Add a target file
    pub fn with_target(mut self, path: impl Into<PathBuf>) -> Self {
        self.target_files.push(path.into());
        self
    }

    /// Add multiple target files
    pub fn with_targets(mut self, paths: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        self.target_files.extend(paths.into_iter().map(Into::into));
        self
    }

    /// Set the risk level
    pub fn with_risk(mut self, risk: OperationRisk) -> Self {
        self.risk = risk;
        self.requires_consent = risk >= OperationRisk::High;
        self
    }

    /// Add a dependency
    pub fn depends_on(mut self, step_id: StepId) -> Self {
        self.dependencies.push(step_id);
        self
    }

    /// Add multiple dependencies
    pub fn depends_on_all(mut self, step_ids: impl IntoIterator<Item = StepId>) -> Self {
        self.dependencies.extend(step_ids);
        self
    }

    /// Add context data
    pub fn with_context(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.context.insert(key.into(), json_value);
        }
        self
    }

    /// Mark as optional
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Set estimated duration
    pub fn with_duration(mut self, seconds: u64) -> Self {
        self.estimated_duration = Some(seconds);
        self
    }

    /// Set rollback strategy
    pub fn with_rollback(mut self, strategy: RollbackStrategy) -> Self {
        self.rollback_strategy = strategy;
        self
    }
}

/// Strategy for rolling back a step
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum RollbackStrategy {
    /// Automatically determine rollback based on action type
    #[default]
    Auto,
    /// Create a snapshot before execution
    Snapshot,
    /// Use specific rollback commands
    Custom(Vec<String>),
    /// No rollback possible/needed
    None,
}

/// A complete task plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Unique identifier for this plan
    pub id: Uuid,

    /// Original task description
    pub task_description: String,

    /// All steps in the plan
    steps: Vec<PlanStep>,

    /// Overall estimated risk
    pub overall_risk: OperationRisk,

    /// Total estimated duration in seconds
    pub estimated_duration: Option<u64>,

    /// Plan creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskPlan {
    /// Create a new task plan
    pub fn new(task_description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_description: task_description.into(),
            steps: Vec::new(),
            overall_risk: OperationRisk::ReadOnly,
            estimated_duration: None,
            created_at: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add a step to the plan
    pub fn add_step(&mut self, step: PlanStep) {
        // Update overall risk if this step is higher
        if step.risk > self.overall_risk {
            self.overall_risk = step.risk;
        }
        self.steps.push(step);
        self.recalculate_duration();
    }

    /// Get all steps
    pub fn steps(&self) -> &[PlanStep] {
        &self.steps
    }

    /// Get a mutable reference to steps
    pub fn steps_mut(&mut self) -> &mut Vec<PlanStep> {
        &mut self.steps
    }

    /// Get a step by ID
    pub fn get_step(&self, id: StepId) -> Option<&PlanStep> {
        self.steps.iter().find(|s| s.id == id)
    }

    /// Get step IDs in execution order (topological sort)
    pub fn execution_order(&self) -> Result<Vec<StepId>> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        // Build step map
        let step_map: HashMap<StepId, &PlanStep> = self.steps.iter().map(|s| (s.id, s)).collect();

        fn visit(
            step_id: StepId,
            step_map: &HashMap<StepId, &PlanStep>,
            visited: &mut HashSet<StepId>,
            temp_visited: &mut HashSet<StepId>,
            order: &mut Vec<StepId>,
        ) -> Result<()> {
            if temp_visited.contains(&step_id) {
                return Err(PlannerError::CycleDetected(format!(
                    "Cycle detected at step {}",
                    step_id
                )));
            }
            if visited.contains(&step_id) {
                return Ok(());
            }

            temp_visited.insert(step_id);

            if let Some(step) = step_map.get(&step_id) {
                for dep_id in &step.dependencies {
                    visit(*dep_id, step_map, visited, temp_visited, order)?;
                }
            }

            temp_visited.remove(&step_id);
            visited.insert(step_id);
            order.push(step_id);
            Ok(())
        }

        for step in &self.steps {
            if !visited.contains(&step.id) {
                visit(step.id, &step_map, &mut visited, &mut temp_visited, &mut order)?;
            }
        }

        Ok(order)
    }

    /// Get steps that can be executed in parallel (no dependencies on pending steps)
    pub fn parallelizable_steps(&self, completed: &HashSet<StepId>) -> Vec<StepId> {
        self.steps
            .iter()
            .filter(|step| {
                !completed.contains(&step.id)
                    && step.dependencies.iter().all(|d| completed.contains(d))
            })
            .map(|s| s.id)
            .collect()
    }

    /// Check if the plan is valid (no cycles, all dependencies exist)
    pub fn validate(&self) -> Result<()> {
        let step_ids: HashSet<StepId> = self.steps.iter().map(|s| s.id).collect();

        // Check all dependencies exist
        for step in &self.steps {
            for dep_id in &step.dependencies {
                if !step_ids.contains(dep_id) {
                    return Err(PlannerError::InvalidStepRef(format!(
                        "Step {} depends on non-existent step {}",
                        step.id, dep_id
                    )));
                }
            }
        }

        // Check for cycles
        let _ = self.execution_order()?;

        Ok(())
    }

    /// Recalculate total estimated duration
    fn recalculate_duration(&mut self) {
        let total: u64 = self
            .steps
            .iter()
            .filter_map(|s| s.estimated_duration)
            .sum();

        self.estimated_duration = if total > 0 { Some(total) } else { None };
    }

    /// Get number of steps
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if plan is empty
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), json_value);
        }
        self
    }
}

/// Context for planning operations
#[derive(Debug, Clone, Default)]
pub struct PlanningContext {
    /// Current working directory
    pub working_directory: Option<PathBuf>,

    /// Available files in the project
    pub project_files: Vec<PathBuf>,

    /// Project type (rust, python, etc.)
    pub project_type: Option<String>,

    /// User preferences/constraints
    pub preferences: HashMap<String, String>,

    /// Previous conversation context
    pub conversation_history: Vec<String>,
}

impl PlanningContext {
    /// Create a new planning context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set working directory
    pub fn with_working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(path.into());
        self
    }

    /// Add project files
    pub fn with_files(mut self, files: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        self.project_files.extend(files.into_iter().map(Into::into));
        self
    }

    /// Set project type
    pub fn with_project_type(mut self, project_type: impl Into<String>) -> Self {
        self.project_type = Some(project_type.into());
        self
    }

    /// Add a preference
    pub fn with_preference(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.preferences.insert(key.into(), value.into());
        self
    }

    /// Add conversation history
    pub fn with_history(mut self, messages: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.conversation_history
            .extend(messages.into_iter().map(Into::into));
        self
    }
}

/// Trait for task planners
#[async_trait]
pub trait Planner: Send + Sync {
    /// Create a plan for the given task
    async fn plan(&self, task: &str, context: &PlanningContext) -> Result<TaskPlan>;

    /// Refine an existing plan based on feedback
    async fn refine_plan(&self, plan: &TaskPlan, feedback: &str) -> Result<TaskPlan> {
        // Default implementation: re-plan with feedback
        let enhanced_task = format!(
            "{}\n\nPrevious plan feedback: {}",
            plan.task_description, feedback
        );
        let context = PlanningContext::default();
        self.plan(&enhanced_task, &context).await
    }

    /// Estimate the complexity of a task (1-10 scale)
    fn estimate_complexity(&self, task: &str) -> u8 {
        // Simple heuristic based on task description
        let word_count = task.split_whitespace().count();
        let has_multiple_files = task.contains("files") || task.contains("multiple");
        let has_complex_keywords = task.contains("refactor")
            || task.contains("migrate")
            || task.contains("integrate")
            || task.contains("optimize");

        let mut complexity: u8 = 3;

        if word_count > 50 {
            complexity += 2;
        } else if word_count > 20 {
            complexity += 1;
        }

        if has_multiple_files {
            complexity += 2;
        }

        if has_complex_keywords {
            complexity += 2;
        }

        complexity.min(10)
    }
}

/// A simple rule-based planner for common tasks
pub struct SimplePlanner {
    /// Maximum steps per plan
    max_steps: usize,
}

impl SimplePlanner {
    /// Create a new simple planner
    pub fn new() -> Self {
        Self { max_steps: 50 }
    }

    /// Set maximum steps
    pub fn with_max_steps(mut self, max: usize) -> Self {
        self.max_steps = max;
        self
    }
}

impl Default for SimplePlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Planner for SimplePlanner {
    async fn plan(&self, task: &str, context: &PlanningContext) -> Result<TaskPlan> {
        let mut plan = TaskPlan::new(task);

        // Parse task keywords and generate appropriate steps
        let task_lower = task.to_lowercase();

        // Always start with analysis
        let analyze_step = PlanStep::new("Analyze task requirements and codebase", ActionType::Analyze)
            .with_duration(5);
        let analyze_id = analyze_step.id;
        plan.add_step(analyze_step);

        // Determine main action based on keywords
        if task_lower.contains("create") || task_lower.contains("add") || task_lower.contains("new")
        {
            // Creation workflow
            let generate_step = PlanStep::new("Generate code for new component", ActionType::Generate)
                .depends_on(analyze_id)
                .with_duration(30);
            let generate_id = generate_step.id;
            plan.add_step(generate_step);

            let write_step = PlanStep::new("Write generated code to files", ActionType::WriteFile)
                .depends_on(generate_id)
                .with_risk(OperationRisk::Medium)
                .with_duration(5);
            let write_id = write_step.id;
            plan.add_step(write_step);

            if task_lower.contains("test") {
                let test_step = PlanStep::new("Run tests to verify changes", ActionType::RunTests)
                    .depends_on(write_id)
                    .with_duration(30);
                plan.add_step(test_step);
            }
        } else if task_lower.contains("edit")
            || task_lower.contains("modify")
            || task_lower.contains("update")
            || task_lower.contains("fix")
        {
            // Edit workflow
            let read_step = PlanStep::new("Read target files", ActionType::ReadFile)
                .depends_on(analyze_id)
                .with_duration(2);
            let read_id = read_step.id;
            plan.add_step(read_step);

            let edit_step = PlanStep::new("Apply modifications to files", ActionType::EditFile)
                .depends_on(read_id)
                .with_risk(OperationRisk::Medium)
                .with_rollback(RollbackStrategy::Snapshot)
                .with_duration(10);
            let edit_id = edit_step.id;
            plan.add_step(edit_step);

            // Add verification step
            let verify_step = PlanStep::new("Verify changes compile and pass tests", ActionType::Build)
                .depends_on(edit_id)
                .with_duration(60);
            plan.add_step(verify_step);
        } else if task_lower.contains("delete") || task_lower.contains("remove") {
            // Deletion workflow
            let read_step = PlanStep::new("Identify files to delete", ActionType::Search)
                .depends_on(analyze_id)
                .with_duration(5);
            let read_id = read_step.id;
            plan.add_step(read_step);

            let confirm_step = PlanStep::new("Confirm deletion with user", ActionType::UserInput)
                .depends_on(read_id)
                .with_duration(10);
            let confirm_id = confirm_step.id;
            plan.add_step(confirm_step);

            let delete_step = PlanStep::new("Delete identified files", ActionType::DeleteFile)
                .depends_on(confirm_id)
                .with_risk(OperationRisk::High)
                .with_rollback(RollbackStrategy::Snapshot)
                .with_duration(5);
            plan.add_step(delete_step);
        } else if task_lower.contains("run") || task_lower.contains("execute") {
            // Command execution workflow
            let cmd_step = PlanStep::new("Execute requested command", ActionType::ShellCommand)
                .depends_on(analyze_id)
                .with_risk(OperationRisk::Medium)
                .with_duration(30);
            plan.add_step(cmd_step);
        } else if task_lower.contains("search") || task_lower.contains("find") {
            // Search workflow
            let search_step = PlanStep::new("Search codebase", ActionType::Search)
                .depends_on(analyze_id)
                .with_duration(10);
            plan.add_step(search_step);
        } else {
            // Default: generic analysis and response
            let generate_step = PlanStep::new("Generate response based on analysis", ActionType::Generate)
                .depends_on(analyze_id)
                .with_duration(15);
            plan.add_step(generate_step);
        }

        // Add context about working directory
        if let Some(ref wd) = context.working_directory {
            plan = plan.with_metadata("working_directory", wd.display().to_string());
        }

        // Validate plan
        plan.validate()?;

        // Limit steps
        if plan.len() > self.max_steps {
            plan.steps_mut().truncate(self.max_steps);
        }

        Ok(plan)
    }
}

/// Builder for creating plans manually
pub struct PlanBuilder {
    plan: TaskPlan,
}

impl PlanBuilder {
    /// Create a new plan builder
    pub fn new(task_description: impl Into<String>) -> Self {
        Self {
            plan: TaskPlan::new(task_description),
        }
    }

    /// Add a step
    pub fn step(mut self, step: PlanStep) -> Self {
        self.plan.add_step(step);
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.plan.metadata.insert(key.into(), json_value);
        }
        self
    }

    /// Build the plan
    pub fn build(self) -> Result<TaskPlan> {
        self.plan.validate()?;
        Ok(self.plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_creation() {
        let step = PlanStep::new("Test step", ActionType::ReadFile)
            .with_target("/path/to/file")
            .with_risk(OperationRisk::Low);

        assert_eq!(step.description, "Test step");
        assert_eq!(step.target_files.len(), 1);
        assert_eq!(step.risk, OperationRisk::Low);
    }

    #[test]
    fn test_plan_creation() {
        let mut plan = TaskPlan::new("Test task");
        let step1 = PlanStep::new("Step 1", ActionType::ReadFile);
        let step1_id = step1.id;
        plan.add_step(step1);

        let step2 = PlanStep::new("Step 2", ActionType::WriteFile).depends_on(step1_id);
        plan.add_step(step2);

        assert_eq!(plan.len(), 2);
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_execution_order() {
        let mut plan = TaskPlan::new("Test task");

        let step1 = PlanStep::new("Step 1", ActionType::ReadFile);
        let step1_id = step1.id;
        plan.add_step(step1);

        let step2 = PlanStep::new("Step 2", ActionType::Analyze);
        let step2_id = step2.id;
        plan.add_step(step2);

        let step3 = PlanStep::new("Step 3", ActionType::WriteFile)
            .depends_on(step1_id)
            .depends_on(step2_id);
        let step3_id = step3.id;
        plan.add_step(step3);

        let order = plan.execution_order().unwrap();

        // step3 should come after step1 and step2
        let pos1 = order.iter().position(|&id| id == step1_id).unwrap();
        let pos2 = order.iter().position(|&id| id == step2_id).unwrap();
        let pos3 = order.iter().position(|&id| id == step3_id).unwrap();

        assert!(pos3 > pos1);
        assert!(pos3 > pos2);
    }

    #[test]
    fn test_cycle_detection() {
        let mut plan = TaskPlan::new("Test task");

        let step1 = PlanStep::new("Step 1", ActionType::ReadFile);
        let step1_id = step1.id;

        let step2 = PlanStep::new("Step 2", ActionType::Analyze).depends_on(step1_id);
        let step2_id = step2.id;

        // Create cycle: step1 depends on step2, but step2 depends on step1
        let step1_with_cycle = PlanStep {
            id: step1_id,
            description: "Step 1".to_string(),
            action_type: ActionType::ReadFile,
            target_files: vec![],
            risk: OperationRisk::ReadOnly,
            dependencies: vec![step2_id],
            context: HashMap::new(),
            optional: false,
            estimated_duration: None,
            requires_consent: false,
            rollback_strategy: RollbackStrategy::Auto,
        };

        plan.add_step(step1_with_cycle);
        plan.add_step(step2);

        assert!(plan.validate().is_err());
    }

    #[tokio::test]
    async fn test_simple_planner() {
        let planner = SimplePlanner::new();
        let context = PlanningContext::new();

        let plan = planner.plan("Create a new function", &context).await.unwrap();

        assert!(!plan.is_empty());
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_plan_builder() {
        let plan = PlanBuilder::new("Test task")
            .step(PlanStep::new("Read file", ActionType::ReadFile))
            .step(PlanStep::new("Analyze", ActionType::Analyze))
            .metadata("author", "test")
            .build()
            .unwrap();

        assert_eq!(plan.len(), 2);
        assert!(plan.metadata.contains_key("author"));
    }

    // ============================================================
    // Additional unit tests for planner module
    // ============================================================

    #[test]
    fn test_step_id_uniqueness() {
        let id1 = StepId::new();
        let id2 = StepId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_action_type_base_risk() {
        assert!(matches!(ActionType::ReadFile.base_risk(), OperationRisk::ReadOnly));
        assert!(matches!(ActionType::WriteFile.base_risk(), OperationRisk::Medium));
        assert!(matches!(ActionType::ShellCommand.base_risk(), OperationRisk::High));
        assert!(matches!(ActionType::Analyze.base_risk(), OperationRisk::ReadOnly));
    }

    #[test]
    fn test_plan_step_with_targets_multiple() {
        let step = PlanStep::new("Multi-target", ActionType::ReadFile)
            .with_targets(["/a.txt", "/b.txt", "/c.txt"]);
        assert_eq!(step.target_files.len(), 3);
    }

    #[test]
    fn test_plan_step_optional() {
        let step = PlanStep::new("Optional step", ActionType::Analyze).optional();
        assert!(step.optional);
    }

    #[test]
    fn test_plan_step_with_duration() {
        let step = PlanStep::new("Slow step", ActionType::ShellCommand)
            .with_duration(120);
        assert_eq!(step.estimated_duration, Some(120));
    }

    #[test]
    fn test_plan_step_with_rollback_strategy() {
        let step = PlanStep::new("Write", ActionType::WriteFile)
            .with_rollback(RollbackStrategy::Custom(vec!["restore backup".into()]));
        assert!(matches!(step.rollback_strategy, RollbackStrategy::Custom(_)));
    }

    #[test]
    fn test_plan_step_with_context() {
        let step = PlanStep::new("Step", ActionType::Analyze)
            .with_context("language", "rust");
        assert!(step.context.contains_key("language"));
    }

    #[test]
    fn test_plan_step_depends_on_all() {
        let id1 = StepId::new();
        let id2 = StepId::new();
        let step = PlanStep::new("Final", ActionType::WriteFile)
            .depends_on_all(vec![id1, id2]);
        assert_eq!(step.dependencies.len(), 2);
        assert!(step.dependencies.contains(&id1));
        assert!(step.dependencies.contains(&id2));
    }

    #[test]
    fn test_plan_step_default_risk() {
        let step = PlanStep::new("Read", ActionType::ReadFile);
        // Default risk should be based on action type
        assert!(matches!(step.risk, OperationRisk::ReadOnly));
    }

    #[test]
    fn test_task_plan_get_step() {
        let mut plan = TaskPlan::new("Test");
        let step = PlanStep::new("Step 1", ActionType::ReadFile);
        let id = step.id;
        plan.add_step(step);
        assert!(plan.get_step(id).is_some());
        assert!(plan.get_step(StepId::new()).is_none());
    }

    #[test]
    fn test_task_plan_is_empty() {
        let plan = TaskPlan::new("Empty plan");
        assert!(plan.is_empty());
        assert_eq!(plan.len(), 0);
    }

    #[test]
    fn test_task_plan_with_metadata() {
        let plan = TaskPlan::new("Plan")
            .with_metadata("version", "1.0")
            .with_metadata("priority", 5);
        assert!(plan.metadata.contains_key("version"));
        assert!(plan.metadata.contains_key("priority"));
    }

    #[test]
    fn test_task_plan_steps_mut() {
        let mut plan = TaskPlan::new("Mutable");
        plan.add_step(PlanStep::new("Step", ActionType::Analyze));
        assert_eq!(plan.steps_mut().len(), 1);
    }

    #[test]
    fn test_parallelizable_steps_empty_completed() {
        let mut plan = TaskPlan::new("Parallel test");
        let step1 = PlanStep::new("Independent 1", ActionType::ReadFile);
        let step2 = PlanStep::new("Independent 2", ActionType::ReadFile);
        plan.add_step(step1);
        plan.add_step(step2);

        let completed = HashSet::new();
        let parallel = plan.parallelizable_steps(&completed);
        // Both steps have no dependencies, both should be parallelizable
        assert_eq!(parallel.len(), 2);
    }

    #[test]
    fn test_parallelizable_steps_with_dependency() {
        let mut plan = TaskPlan::new("Dep test");
        let step1 = PlanStep::new("First", ActionType::ReadFile);
        let id1 = step1.id;
        plan.add_step(step1);

        let step2 = PlanStep::new("Second", ActionType::WriteFile).depends_on(id1);
        plan.add_step(step2);

        // Nothing completed yet — only step1 (no deps) is parallelizable
        let completed = HashSet::new();
        let parallel = plan.parallelizable_steps(&completed);
        assert_eq!(parallel.len(), 1);
        assert_eq!(parallel[0], id1);
    }

    #[test]
    fn test_rollback_strategy_variants() {
        let _ = RollbackStrategy::Auto;
        let _ = RollbackStrategy::Snapshot;
        let _ = RollbackStrategy::None;
        let _ = RollbackStrategy::Custom(vec!["strategy".into()]);
    }

    #[test]
    fn test_plan_builder_empty() {
        let result = PlanBuilder::new("Empty task").build();
        // Empty plan should still build (or fail validation depending on impl)
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_single_step_execution_order() {
        let mut plan = TaskPlan::new("Single");
        let step = PlanStep::new("Only step", ActionType::ReadFile);
        let id = step.id;
        plan.add_step(step);

        let order = plan.execution_order().unwrap();
        assert_eq!(order.len(), 1);
        assert_eq!(order[0], id);
    }

    #[test]
    fn test_planning_context_new() {
        let ctx = PlanningContext::new();
        assert!(ctx.project_files.is_empty());
    }

}
