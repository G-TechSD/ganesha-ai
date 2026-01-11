//! Ganesha Workflow Mode System
//!
//! Manages different operational modes:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                        GANESHA WORKFLOW                                 │
//! │                                                                         │
//! │  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐         │
//! │  │ PLANNING │───▶│  DEV     │───▶│ TESTING  │───▶│  EVAL    │         │
//! │  │   MODE   │    │  MODE    │    │   MODE   │    │  MODE    │         │
//! │  └──────────┘    └──────────┘    └────┬─────┘    └──────────┘         │
//! │                                       │                                 │
//! │                                       ▼                                 │
//! │                                  ┌──────────┐                           │
//! │                                  │FIX/REFINE│◀─────┐                   │
//! │                                  │   MODE   │──────┘ (iterate)         │
//! │                                  └──────────┘                           │
//! │                                                                         │
//! │  ┌──────────┐    ┌──────────┐                                          │
//! │  │ SYSADMIN │    │   CHAT   │    (Independent modes)                   │
//! │  │   MODE   │    │   MODE   │                                          │
//! │  └──────────┘    └──────────┘                                          │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```

use console::style;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Ganesha operational modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GaneshaMode {
    /// Basic conversational mode - knowledge sharing, Q&A
    Chat,
    /// Careful planning before development
    Planning,
    /// Long-horizon development with Wiggum verification
    Development,
    /// Running tests and validation
    Testing,
    /// Fixing issues found in testing
    FixRefine,
    /// Final evaluation - does it meet user standards?
    Evaluation,
    /// System administration tasks (install, configure)
    SysAdmin,
}

impl GaneshaMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            GaneshaMode::Chat => "Chat",
            GaneshaMode::Planning => "Planning",
            GaneshaMode::Development => "Development",
            GaneshaMode::Testing => "Testing",
            GaneshaMode::FixRefine => "Fix/Refine",
            GaneshaMode::Evaluation => "Evaluation",
            GaneshaMode::SysAdmin => "SysAdmin",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            GaneshaMode::Chat => "💬",
            GaneshaMode::Planning => "📋",
            GaneshaMode::Development => "🔨",
            GaneshaMode::Testing => "🧪",
            GaneshaMode::FixRefine => "🔧",
            GaneshaMode::Evaluation => "✅",
            GaneshaMode::SysAdmin => "⚙️",
        }
    }

    /// Get valid transitions from this mode
    pub fn valid_transitions(&self) -> Vec<GaneshaMode> {
        match self {
            GaneshaMode::Chat => vec![
                GaneshaMode::Planning,
                GaneshaMode::SysAdmin,
            ],
            GaneshaMode::Planning => vec![
                GaneshaMode::Development,
                GaneshaMode::Chat,
            ],
            GaneshaMode::Development => vec![
                GaneshaMode::Testing,
                GaneshaMode::Planning, // Can go back to re-plan
            ],
            GaneshaMode::Testing => vec![
                GaneshaMode::FixRefine,
                GaneshaMode::Evaluation,
            ],
            GaneshaMode::FixRefine => vec![
                GaneshaMode::Testing, // Loop back to test fixes
            ],
            GaneshaMode::Evaluation => vec![
                GaneshaMode::Chat,
                GaneshaMode::FixRefine, // If not acceptable
                GaneshaMode::Planning,  // Major rework needed
            ],
            GaneshaMode::SysAdmin => vec![
                GaneshaMode::Chat,
            ],
        }
    }
}

/// Development plan created during Planning mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentPlan {
    pub id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub task_description: String,
    pub analysis: PlanAnalysis,
    pub phases: Vec<PlanPhase>,
    pub risks: Vec<String>,
    pub success_criteria: Vec<String>,
    pub estimated_complexity: Complexity,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanAnalysis {
    pub understanding: String,
    pub requirements: Vec<String>,
    pub constraints: Vec<String>,
    pub assumptions: Vec<String>,
    pub questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPhase {
    pub id: usize,
    pub name: String,
    pub description: String,
    pub tasks: Vec<PlanTask>,
    pub status: PhaseStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTask {
    pub id: usize,
    pub description: String,
    pub files_involved: Vec<String>,
    pub dependencies: Vec<usize>,
    pub status: TaskStatus,
    pub verification: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Complexity {
    Trivial,    // Single file, few lines
    Simple,     // Single component, clear scope
    Moderate,   // Multiple files, some dependencies
    Complex,    // Many files, significant changes
    Major,      // Architectural changes, high risk
}

/// Test results from Testing mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    pub run_id: Uuid,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub test_suites: Vec<TestSuiteResult>,
    pub overall_passed: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteResult {
    pub name: String,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failures: Vec<TestFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFailure {
    pub test_name: String,
    pub error: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
}

/// Final evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub criteria_results: Vec<CriterionResult>,
    pub overall_score: f32,
    pub verdict: EvalVerdict,
    pub summary: String,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub criterion: String,
    pub passed: bool,
    pub score: f32,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvalVerdict {
    Excellent,    // Exceeds expectations
    Acceptable,   // Meets requirements
    NeedsWork,    // Minor issues to fix
    Unacceptable, // Major rework needed
}

/// Vision model availability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    pub enabled: bool,
    pub primary_has_vision: bool,
    pub cloud_vision_available: bool,
    pub cloud_vision_provider: Option<String>,
    pub cloud_vision_model: Option<String>,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            primary_has_vision: false,
            cloud_vision_available: false,
            cloud_vision_provider: None,
            cloud_vision_model: None,
        }
    }
}

impl VisionConfig {
    /// Check if vision is available (either primary or cloud fallback)
    pub fn is_available(&self) -> bool {
        self.enabled && (self.primary_has_vision || self.cloud_vision_available)
    }

    /// Get provider to use for vision
    pub fn vision_provider(&self) -> Option<&str> {
        if !self.enabled {
            return None;
        }
        if self.primary_has_vision {
            Some("primary")
        } else if self.cloud_vision_available {
            self.cloud_vision_provider.as_deref()
        } else {
            None
        }
    }
}

/// The workflow state machine
pub struct WorkflowEngine {
    pub current_mode: GaneshaMode,
    pub session_id: Uuid,
    pub started_at: Instant,
    pub mode_history: Vec<(GaneshaMode, chrono::DateTime<chrono::Utc>)>,
    pub current_plan: Option<DevelopmentPlan>,
    pub test_results: Vec<TestResults>,
    pub evaluation: Option<EvaluationResult>,
    pub fix_iterations: usize,
    pub max_fix_iterations: usize,
    pub vision_config: VisionConfig,
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self {
            current_mode: GaneshaMode::Chat,
            session_id: Uuid::new_v4(),
            started_at: Instant::now(),
            mode_history: vec![(GaneshaMode::Chat, chrono::Utc::now())],
            current_plan: None,
            test_results: vec![],
            evaluation: None,
            fix_iterations: 0,
            max_fix_iterations: 5,
            vision_config: VisionConfig::default(),
        }
    }

    /// Transition to a new mode
    pub fn transition(&mut self, new_mode: GaneshaMode) -> Result<(), String> {
        let valid = self.current_mode.valid_transitions();
        if !valid.contains(&new_mode) {
            return Err(format!(
                "Cannot transition from {} to {}. Valid transitions: {:?}",
                self.current_mode.display_name(),
                new_mode.display_name(),
                valid.iter().map(|m| m.display_name()).collect::<Vec<_>>()
            ));
        }

        println!("\n{} {} → {} {}",
            self.current_mode.emoji(),
            style(self.current_mode.display_name()).dim(),
            style(new_mode.display_name()).cyan().bold(),
            new_mode.emoji()
        );

        self.mode_history.push((new_mode, chrono::Utc::now()));
        self.current_mode = new_mode;

        // Reset fix iterations when entering Testing mode
        if new_mode == GaneshaMode::Testing {
            self.fix_iterations = 0;
        }

        // Increment fix iterations when entering FixRefine mode
        if new_mode == GaneshaMode::FixRefine {
            self.fix_iterations += 1;
            if self.fix_iterations > self.max_fix_iterations {
                println!("{} Max fix iterations ({}) reached. Consider re-planning.",
                    style("⚠").yellow(),
                    self.max_fix_iterations
                );
            }
        }

        Ok(())
    }

    /// Force transition (bypass validation)
    pub fn force_transition(&mut self, new_mode: GaneshaMode) {
        self.mode_history.push((new_mode, chrono::Utc::now()));
        self.current_mode = new_mode;
    }

    /// Get system prompt for current mode
    pub fn get_mode_prompt(&self) -> String {
        match self.current_mode {
            GaneshaMode::Chat => self.chat_prompt(),
            GaneshaMode::Planning => self.planning_prompt(),
            GaneshaMode::Development => self.development_prompt(),
            GaneshaMode::Testing => self.testing_prompt(),
            GaneshaMode::FixRefine => self.fix_refine_prompt(),
            GaneshaMode::Evaluation => self.evaluation_prompt(),
            GaneshaMode::SysAdmin => self.sysadmin_prompt(),
        }
    }

    fn chat_prompt(&self) -> String {
        format!(r#"MODE: CHAT 💬

You are Ganesha in conversational mode. Share knowledge, ideas, and information.

CAPABILITIES:
- Answer questions about programming, system administration, and technology
- Explain concepts and provide guidance
- Discuss architecture and design decisions
- Help brainstorm solutions

WHEN TO SUGGEST MODE CHANGE:
- If user wants to BUILD something → suggest PLANNING mode
- If user wants to INSTALL/CONFIGURE → suggest SYSADMIN mode
- Stay in CHAT for discussion, Q&A, and knowledge sharing

{}

Respond naturally and helpfully."#,
            self.vision_note()
        )
    }

    fn planning_prompt(&self) -> String {
        format!(r#"MODE: PLANNING 📋

You are Ganesha in careful planning mode. Before ANY development:

1. UNDERSTAND the task thoroughly
   - What exactly does the user want?
   - What are the requirements and constraints?
   - What assumptions are we making?
   - What questions need answers?

2. ANALYZE the codebase
   - What files will be affected?
   - What dependencies exist?
   - What could break?

3. CREATE a detailed plan
   - Break into phases
   - List specific tasks
   - Define success criteria
   - Identify risks

4. GET APPROVAL before proceeding

OUTPUT FORMAT:
```
## Understanding
[Your understanding of the task]

## Requirements
- [Requirement 1]
- [Requirement 2]

## Plan
### Phase 1: [Name]
- Task 1.1: [Description]
- Task 1.2: [Description]

### Phase 2: [Name]
...

## Success Criteria
- [ ] [Criterion 1]
- [ ] [Criterion 2]

## Risks
- [Risk 1]
- [Risk 2]

## Questions (if any)
- [Question 1]
```

Do NOT write code in planning mode. Focus on understanding and planning.
When plan is complete, ask user to approve before transitioning to DEVELOPMENT mode."#)
    }

    fn development_prompt(&self) -> String {
        let plan_context = if let Some(ref plan) = self.current_plan {
            format!("\nCURRENT PLAN:\n{}\n\nFollow this plan. Check off tasks as completed.",
                plan.phases.iter()
                    .map(|p| format!("- {}: {}", p.name, p.description))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            String::new()
        };

        format!(r#"MODE: DEVELOPMENT 🔨 (with Wiggum Verification)

You are Ganesha in long-horizon development mode.
{}

WORKFLOW:
1. Execute tasks from the plan
2. VERIFY each action (Wiggum loop)
3. If verification fails, retry with different approach
4. Track progress against the plan

RULES:
- ALWAYS read files before editing
- ALWAYS verify changes after making them
- If stuck, consider going back to PLANNING
- Document what you're doing
- When development phase complete → transition to TESTING

{}

Focus on quality over speed. Verify everything."#,
            plan_context,
            self.vision_note()
        )
    }

    fn testing_prompt(&self) -> String {
        format!(r#"MODE: TESTING 🧪

You are Ganesha in testing mode. Thoroughly validate the work.

TESTING APPROACH:
1. Run existing test suites
   - `cargo test`, `npm test`, `pytest`, etc.

2. Manual verification
   - Does the feature work as expected?
   - Edge cases handled?
   - Error messages clear?

3. Integration check
   - Does it work with existing code?
   - Any regressions?

4. Code quality
   - Linting passes?
   - Type checks pass?
   - Build succeeds?

OUTPUT FORMAT:
```
## Test Results

### Automated Tests
- [Test suite]: PASS/FAIL (X passed, Y failed)

### Manual Verification
- [ ] [Check 1]: PASS/FAIL
- [ ] [Check 2]: PASS/FAIL

### Issues Found
1. [Issue description]
   - Location: [file:line]
   - Severity: High/Medium/Low

## Verdict
[PASS - ready for evaluation] or [FAIL - needs fixes]
```

If tests PASS → transition to EVALUATION
If tests FAIL → transition to FIX/REFINE"#)
    }

    fn fix_refine_prompt(&self) -> String {
        let iteration_note = format!(
            "Fix iteration: {}/{}",
            self.fix_iterations,
            self.max_fix_iterations
        );

        let issues = if let Some(ref results) = self.test_results.last() {
            results.test_suites.iter()
                .flat_map(|s| &s.failures)
                .map(|f| format!("- {}: {}", f.test_name, f.error))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            "No test results available".to_string()
        };

        format!(r#"MODE: FIX/REFINE 🔧

You are Ganesha in fix/refine mode. Fix the issues found in testing.

{}

ISSUES TO FIX:
{}

APPROACH:
1. Prioritize by severity (High → Medium → Low)
2. Fix one issue at a time
3. VERIFY each fix
4. Don't introduce new issues

RULES:
- Focus on the specific issues
- Don't refactor unrelated code
- Keep changes minimal and targeted
- Test your fix before moving on

When all issues fixed → transition to TESTING (to verify fixes)
If max iterations reached → consider going back to PLANNING"#,
            iteration_note,
            issues
        )
    }

    fn evaluation_prompt(&self) -> String {
        let criteria = if let Some(ref plan) = self.current_plan {
            plan.success_criteria.iter()
                .map(|c| format!("- [ ] {}", c))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            "- [ ] Functionality works as expected\n- [ ] Code quality is acceptable\n- [ ] No regressions".to_string()
        };

        format!(r#"MODE: EVALUATION ✅

You are Ganesha in final evaluation mode. Determine if the work meets user standards.

SUCCESS CRITERIA:
{}

EVALUATION CHECKLIST:
- [ ] All requirements met
- [ ] Tests passing
- [ ] Code quality acceptable
- [ ] Documentation adequate
- [ ] No security issues
- [ ] Performance acceptable

SCORING:
- Excellent (90-100%): Exceeds expectations
- Acceptable (70-89%): Meets requirements
- Needs Work (50-69%): Minor issues remain
- Unacceptable (<50%): Major rework needed

OUTPUT FORMAT:
```
## Final Evaluation

### Criteria Assessment
| Criterion | Status | Score | Notes |
|-----------|--------|-------|-------|
| [Criterion] | PASS/FAIL | X/10 | [Notes] |

### Overall Score: X/100

### Verdict: [EXCELLENT/ACCEPTABLE/NEEDS_WORK/UNACCEPTABLE]

### Summary
[Overall assessment]

### Recommendations
- [Recommendation 1]
- [Recommendation 2]
```

EXCELLENT/ACCEPTABLE → Complete, return to CHAT
NEEDS_WORK → transition to FIX/REFINE
UNACCEPTABLE → transition to PLANNING (major rework)"#,
            criteria
        )
    }

    fn sysadmin_prompt(&self) -> String {
        format!(r#"MODE: SYSADMIN ⚙️

You are Ganesha in system administration mode.

CAPABILITIES:
- Install software and packages
- Configure services and applications
- Manage system settings
- Set up development environments
- Troubleshoot system issues

SAFETY RULES:
1. ALWAYS explain what a command will do
2. Use dry-run flags when available
3. Back up configs before modifying
4. Check system state before and after
5. Never run destructive commands without confirmation

COMMON TASKS:
- Package management: apt, dnf, pacman, brew
- Service management: systemctl, service
- User management: useradd, usermod
- Network: ip, ss, netstat, firewall
- Disk: df, du, mount, fdisk
- Process: ps, top, htop, kill

{}

When task complete → transition to CHAT"#,
            self.vision_note()
        )
    }

    fn vision_note(&self) -> String {
        if self.vision_config.is_available() {
            format!("VISION: Enabled via {} (Alt+V to paste screenshot)",
                self.vision_config.vision_provider().unwrap_or("unknown")
            )
        } else {
            "VISION: Disabled (no vision-capable model configured)".to_string()
        }
    }

    /// Configure vision based on available models
    pub fn configure_vision(&mut self, primary_has_vision: bool, cloud_provider: Option<(String, String)>) {
        self.vision_config.primary_has_vision = primary_has_vision;

        if let Some((provider, model)) = cloud_provider {
            self.vision_config.cloud_vision_available = true;
            self.vision_config.cloud_vision_provider = Some(provider);
            self.vision_config.cloud_vision_model = Some(model);
        }

        // Only enable if we have a vision-capable model
        self.vision_config.enabled = self.vision_config.is_available();
    }

    /// Get status summary
    pub fn status(&self) -> String {
        let elapsed = self.started_at.elapsed();
        format!(
            "{} {} | Session: {} | Elapsed: {:?} | Vision: {}",
            self.current_mode.emoji(),
            self.current_mode.display_name(),
            &self.session_id.to_string()[..8],
            elapsed,
            if self.vision_config.is_available() { "ON" } else { "OFF" }
        )
    }

    /// Detect appropriate mode from user input
    pub fn detect_mode(&self, input: &str) -> Option<GaneshaMode> {
        let lower = input.to_lowercase();

        // Testing triggers - check first since "test" could be in other contexts
        if lower.starts_with("run test") || lower.starts_with("test ")
            || lower.contains("run the tests") || lower.contains("execute tests")
            || lower.contains("verify it works") || lower.contains("check if it works")
            || lower == "test" || lower == "tests"
        {
            return Some(GaneshaMode::Testing);
        }

        // Fix/Refine triggers
        if lower.starts_with("fix ") || lower.contains("debug") || lower.contains("doesn't work")
            || lower.contains("not working") || lower.contains("broken") || lower.contains("error")
            || lower.contains("bug") || lower.contains("issue") || lower.contains("failed")
            || lower.contains("failing") || lower.contains("refactor")
        {
            return Some(GaneshaMode::FixRefine);
        }

        // Evaluation triggers
        if lower.contains("evaluate") || lower.contains("review") || lower.contains("assess")
            || lower.contains("is it ready") || lower.contains("good enough") || lower.contains("done?")
            || lower.contains("finished?") || lower.contains("complete?")
        {
            return Some(GaneshaMode::Evaluation);
        }

        // Planning/Development triggers - complex tasks that need planning first
        if lower.contains("build") || lower.contains("create") || lower.contains("implement")
            || lower.contains("develop") || lower.contains("write code") || lower.contains("add feature")
            || lower.contains("new feature") || lower.contains("make a") || lower.contains("design")
            || lower.contains("architect") || lower.contains("plan") || lower.contains("help me with")
            || lower.contains("i need") || lower.contains("i want") || lower.contains("can you make")
            || lower.contains("refactor") || lower.contains("rewrite")
        {
            return Some(GaneshaMode::Planning);
        }

        // Sysadmin triggers
        if lower.contains("install") || lower.contains("configure") || lower.contains("setup")
            || lower.contains("set up") || lower.contains("service") || lower.contains("package")
            || lower.starts_with("apt ") || lower.starts_with("sudo ")
            || lower.starts_with("systemctl") || lower.starts_with("docker ")
            || lower.contains("permission") || lower.contains("firewall") || lower.contains("network")
            || lower.contains("update system") || lower.contains("upgrade")
        {
            return Some(GaneshaMode::SysAdmin);
        }

        // Chat mode - questions, info requests (stay in current or switch to chat)
        if lower.starts_with("what ") || lower.starts_with("who ") || lower.starts_with("when ")
            || lower.starts_with("where ") || lower.starts_with("why ") || lower.starts_with("how ")
            || lower.starts_with("explain ") || lower.starts_with("tell me")
            || lower.ends_with("?")
        {
            // Only suggest Chat if not already in a workflow
            if self.current_mode != GaneshaMode::Chat {
                return None; // Stay in current mode for questions during workflow
            }
        }

        // No clear mode detected
        None
    }

    /// Auto-transition to detected mode (allows jumping from Chat to any mode)
    pub fn auto_transition(&mut self, new_mode: GaneshaMode) -> bool {
        // From Chat, we can go to any mode
        if self.current_mode == GaneshaMode::Chat {
            self.force_transition(new_mode);
            return true;
        }

        // Otherwise use normal transition rules
        if self.transition(new_mode).is_ok() {
            return true;
        }

        false
    }
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_transitions() {
        let mut engine = WorkflowEngine::new();
        assert_eq!(engine.current_mode, GaneshaMode::Chat);

        // Chat -> Planning (valid)
        assert!(engine.transition(GaneshaMode::Planning).is_ok());
        assert_eq!(engine.current_mode, GaneshaMode::Planning);

        // Planning -> Development (valid)
        assert!(engine.transition(GaneshaMode::Development).is_ok());
        assert_eq!(engine.current_mode, GaneshaMode::Development);

        // Development -> Chat (invalid)
        assert!(engine.transition(GaneshaMode::Chat).is_err());
    }

    #[test]
    fn test_vision_config() {
        let mut config = VisionConfig::default();
        assert!(!config.is_available());

        config.enabled = true;
        config.cloud_vision_available = true;
        config.cloud_vision_provider = Some("anthropic".to_string());
        assert!(config.is_available());
        assert_eq!(config.vision_provider(), Some("anthropic"));
    }

    #[test]
    fn test_mode_detection() {
        let engine = WorkflowEngine::new();

        assert_eq!(engine.detect_mode("build a new feature"), Some(GaneshaMode::Planning));
        assert_eq!(engine.detect_mode("install docker"), Some(GaneshaMode::SysAdmin));
        assert_eq!(engine.detect_mode("what is rust"), None);
    }
}
