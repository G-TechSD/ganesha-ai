//! Ralph Wiggum Loop - Trust But Verify
//!
//! Named after the Simpsons character, this implements iterative verification:
//!
//! ```text
//! while not satisfactory:
//!     1. Generate solution
//!     2. Execute (with consent)
//!     3. Verify result matches intent
//!     4. If not correct, iterate with context
//! ```
//!
//! The key insight: LLMs can check their own work better than they can do it
//! the first time. By having a verification step, we catch mistakes early.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Result of a Wiggum verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub passed: bool,
    pub confidence: f32,
    pub issues: Vec<VerificationIssue>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationIssue {
    pub severity: IssueSeverity,
    pub description: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Configuration for the Wiggum loop
#[derive(Debug, Clone)]
pub struct WiggumConfig {
    /// Maximum iterations before giving up
    pub max_iterations: usize,
    /// Minimum confidence to consider done
    pub confidence_threshold: f32,
    /// Whether to require human approval on each iteration
    pub require_human_approval: bool,
    /// Timeout for the entire loop
    pub timeout: Duration,
    /// Whether to use vision for verification
    pub use_vision: bool,
}

impl Default for WiggumConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            confidence_threshold: 0.85,
            require_human_approval: true,
            timeout: Duration::from_secs(300), // 5 minutes
            use_vision: false,
        }
    }
}

/// The Wiggum Loop executor
pub struct WiggumLoop<'a> {
    config: WiggumConfig,
    original_intent: String,
    context: Vec<IterationContext>,
    verifier: Box<dyn Verifier + 'a>,
}

/// Context accumulated across iterations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationContext {
    pub iteration: usize,
    pub action_taken: String,
    pub result: String,
    pub verification: VerificationResult,
    pub duration: Duration,
}

/// Trait for verification strategies
#[async_trait::async_trait]
pub trait Verifier: Send + Sync {
    /// Verify that the result matches the intent
    async fn verify(
        &self,
        intent: &str,
        action: &str,
        result: &str,
        previous_attempts: &[IterationContext],
    ) -> VerificationResult;

    /// Generate improvement suggestions based on failed verification
    async fn suggest_improvements(
        &self,
        intent: &str,
        issues: &[VerificationIssue],
    ) -> Vec<String>;
}

/// LLM-based verifier
pub struct LlmVerifier {
    endpoint: String,
    model: String,
}

impl LlmVerifier {
    pub fn new(endpoint: &str, model: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Verifier for LlmVerifier {
    async fn verify(
        &self,
        intent: &str,
        action: &str,
        result: &str,
        previous_attempts: &[IterationContext],
    ) -> VerificationResult {
        let prompt = format!(
            r#"You are a verification agent. Your job is to check if an action achieved the intended goal.

ORIGINAL INTENT: {}

ACTION TAKEN: {}

RESULT:
{}

PREVIOUS ATTEMPTS: {}

Analyze whether the result satisfies the original intent.
Respond in this exact JSON format:
{{
    "passed": true/false,
    "confidence": 0.0-1.0,
    "issues": [
        {{"severity": "info/warning/error/critical", "description": "...", "location": "optional"}}
    ],
    "suggestions": ["improvement 1", "improvement 2"]
}}
"#,
            intent,
            action,
            result,
            previous_attempts.len()
        );

        // Call LLM for verification
        match self.call_verifier(&prompt).await {
            Ok(response) => parse_verification_response(&response),
            Err(_) => VerificationResult {
                passed: false,
                confidence: 0.0,
                issues: vec![VerificationIssue {
                    severity: IssueSeverity::Error,
                    description: "Verification failed to execute".into(),
                    location: None,
                }],
                suggestions: vec!["Retry the verification".into()],
            },
        }
    }

    async fn suggest_improvements(
        &self,
        intent: &str,
        issues: &[VerificationIssue],
    ) -> Vec<String> {
        let issues_text: Vec<String> = issues
            .iter()
            .map(|i| format!("[{:?}] {}", i.severity, i.description))
            .collect();

        let prompt = format!(
            r#"Given this intent: {}

And these issues:
{}

Suggest specific improvements to achieve the intent. Be concise.
"#,
            intent,
            issues_text.join("\n")
        );

        match self.call_verifier(&prompt).await {
            Ok(response) => response.lines().map(|l| l.trim().to_string()).collect(),
            Err(_) => vec!["Review the issues and try again".into()],
        }
    }
}

impl LlmVerifier {
    async fn call_verifier(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();

        let response = client
            .post(format!("{}/v1/chat/completions", self.endpoint))
            .json(&serde_json::json!({
                "model": self.model,
                "messages": [
                    {"role": "system", "content": "You are a precise verification agent. Respond only in the requested format."},
                    {"role": "user", "content": prompt}
                ],
                "temperature": 0.1,
                "max_tokens": 1000
            }))
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        Ok(json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}

fn parse_verification_response(response: &str) -> VerificationResult {
    // Try to extract JSON from the response
    let json_start = response.find('{');
    let json_end = response.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &response[start..=end];
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
            return VerificationResult {
                passed: parsed["passed"].as_bool().unwrap_or(false),
                confidence: parsed["confidence"].as_f64().unwrap_or(0.0) as f32,
                issues: parsed["issues"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|i| {
                                Some(VerificationIssue {
                                    severity: match i["severity"].as_str()? {
                                        "info" => IssueSeverity::Info,
                                        "warning" => IssueSeverity::Warning,
                                        "error" => IssueSeverity::Error,
                                        "critical" => IssueSeverity::Critical,
                                        _ => IssueSeverity::Warning,
                                    },
                                    description: i["description"].as_str()?.to_string(),
                                    location: i["location"].as_str().map(|s| s.to_string()),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                suggestions: parsed["suggestions"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            };
        }
    }

    // Fallback: simple keyword analysis
    let passed = response.to_lowercase().contains("passed")
        || response.to_lowercase().contains("success")
        || response.to_lowercase().contains("complete");

    VerificationResult {
        passed,
        confidence: if passed { 0.7 } else { 0.3 },
        issues: vec![],
        suggestions: vec![],
    }
}

impl<'a> WiggumLoop<'a> {
    pub fn new(intent: &str, verifier: Box<dyn Verifier + 'a>, config: WiggumConfig) -> Self {
        Self {
            config,
            original_intent: intent.to_string(),
            context: vec![],
            verifier,
        }
    }

    /// Run the Wiggum loop
    pub async fn run<F, Fut>(
        &mut self,
        mut executor: F,
    ) -> Result<WiggumOutcome, WiggumError>
    where
        F: FnMut(&str, &[IterationContext]) -> Fut,
        Fut: std::future::Future<Output = Result<(String, String), Box<dyn std::error::Error + Send + Sync>>>,
    {
        let start = Instant::now();

        for iteration in 0..self.config.max_iterations {
            // Check timeout
            if start.elapsed() > self.config.timeout {
                return Err(WiggumError::Timeout);
            }

            // Build prompt with context from previous iterations
            let prompt = if self.context.is_empty() {
                self.original_intent.clone()
            } else {
                let last = self.context.last().unwrap();
                format!(
                    "{}\n\nPREVIOUS ATTEMPT FAILED. Issues:\n{}\n\nSuggestions:\n{}\n\nTry again with these improvements.",
                    self.original_intent,
                    last.verification.issues.iter()
                        .map(|i| format!("- {:?}: {}", i.severity, i.description))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    last.verification.suggestions.join("\n- "),
                )
            };

            // Execute the action
            let iter_start = Instant::now();
            let (action, result) = match executor(&prompt, &self.context).await {
                Ok(r) => r,
                Err(e) => {
                    return Err(WiggumError::ExecutionFailed(e.to_string()));
                }
            };

            // Verify the result
            let verification = self.verifier
                .verify(&self.original_intent, &action, &result, &self.context)
                .await;

            let iter_context = IterationContext {
                iteration,
                action_taken: action,
                result: result.clone(),
                verification: verification.clone(),
                duration: iter_start.elapsed(),
            };

            self.context.push(iter_context);

            // Check if we're done
            if verification.passed && verification.confidence >= self.config.confidence_threshold {
                return Ok(WiggumOutcome {
                    success: true,
                    iterations: iteration + 1,
                    final_result: result,
                    total_duration: start.elapsed(),
                    context: self.context.clone(),
                });
            }

            // Check for critical issues that should abort
            if verification.issues.iter().any(|i| i.severity == IssueSeverity::Critical) {
                return Err(WiggumError::CriticalIssue(
                    verification.issues
                        .iter()
                        .filter(|i| i.severity == IssueSeverity::Critical)
                        .map(|i| i.description.clone())
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
        }

        // Max iterations reached
        Err(WiggumError::MaxIterationsReached(self.context.clone()))
    }

    /// Get the current context
    pub fn context(&self) -> &[IterationContext] {
        &self.context
    }
}

/// Outcome of a successful Wiggum loop
#[derive(Debug, Clone)]
pub struct WiggumOutcome {
    pub success: bool,
    pub iterations: usize,
    pub final_result: String,
    pub total_duration: Duration,
    pub context: Vec<IterationContext>,
}

/// Errors that can occur in the Wiggum loop
#[derive(Debug)]
pub enum WiggumError {
    Timeout,
    ExecutionFailed(String),
    CriticalIssue(String),
    MaxIterationsReached(Vec<IterationContext>),
}

impl std::fmt::Display for WiggumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WiggumError::Timeout => write!(f, "Wiggum loop timed out"),
            WiggumError::ExecutionFailed(e) => write!(f, "Execution failed: {}", e),
            WiggumError::CriticalIssue(e) => write!(f, "Critical issue: {}", e),
            WiggumError::MaxIterationsReached(ctx) => {
                write!(f, "Max iterations ({}) reached", ctx.len())
            }
        }
    }
}

impl std::error::Error for WiggumError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_verification_response() {
        let response = r#"
{
    "passed": true,
    "confidence": 0.95,
    "issues": [],
    "suggestions": []
}
"#;
        let result = parse_verification_response(response);
        assert!(result.passed);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_parse_verification_with_issues() {
        let response = r#"
{
    "passed": false,
    "confidence": 0.4,
    "issues": [
        {"severity": "error", "description": "Missing semicolon", "location": "line 42"}
    ],
    "suggestions": ["Add semicolon at line 42"]
}
"#;
        let result = parse_verification_response(response);
        assert!(!result.passed);
        assert_eq!(result.issues.len(), 1);
        assert_eq!(result.issues[0].severity, IssueSeverity::Error);
    }

    #[test]
    fn test_config_default() {
        let config = WiggumConfig::default();
        assert_eq!(config.max_iterations, 5);
        assert!(config.confidence_threshold > 0.8);
    }
}
