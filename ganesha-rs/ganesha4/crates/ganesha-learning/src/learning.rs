//! Learning Engine for Ganesha Vision
//!
//! This module provides the core learning-from-demonstration functionality.
//! The key insight is that Ganesha can watch what a user does, learn the pattern,
//! and then generalize it to similar situations in other applications.
//!
//! # Example
//!
//! ```ignore
//! use ganesha_learning::learning::{LearningEngine, Screenshot};
//! use ganesha_learning::Database;
//!
//! // Create the learning engine
//! let db = Database::open("ganesha.db")?;
//! let mut engine = LearningEngine::new(db);
//!
//! // Start recording a demonstration
//! let session = engine.start_recording("Blender", "Navigate to render settings")?;
//!
//! // ... user performs actions ...
//!
//! // Stop recording and get the demonstration
//! let demo = engine.stop_recording()?;
//!
//! // Extract a reusable skill from the demonstration
//! let skill = engine.extract_skill(&demo, "Navigate menu hierarchy")?;
//!
//! // Later, find relevant skills for a new task
//! let screenshot = Screenshot::capture()?;
//! let skills = engine.find_relevant_skills("open preferences", &screenshot)?;
//!
//! // Apply the skill to the current screen
//! let actions = engine.apply_skill(&skills[0].skill, &screenshot)?;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::{
    Action, ActionDetails, ActionTemplate, ActionType, Database, Demonstration,
    MouseButton, Modifier, RecordedAction, Skill, UiElement,
};
use crate::error::{Error, Result};

// ============================================================================
// Screenshot
// ============================================================================

/// A captured screenshot with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    /// Base64 encoded image data
    pub data: String,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Timestamp when captured
    pub timestamp: DateTime<Utc>,
    /// Application name if detected
    pub app_name: Option<String>,
    /// Window title if detected
    pub window_title: Option<String>,
    /// Detected UI elements
    pub elements: Vec<UiElement>,
}

impl Screenshot {
    /// Create a new screenshot from base64 data
    pub fn new(data: String, width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
            timestamp: Utc::now(),
            app_name: None,
            window_title: None,
            elements: Vec::new(),
        }
    }

    /// Create a screenshot with app information
    pub fn with_app_info(mut self, app_name: impl Into<String>, window_title: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self.window_title = Some(window_title.into());
        self
    }

    /// Set the detected UI elements
    pub fn with_elements(mut self, elements: Vec<UiElement>) -> Self {
        self.elements = elements;
        self
    }

    /// Find an element at the given coordinates
    pub fn element_at(&self, x: i32, y: i32) -> Option<&UiElement> {
        self.elements
            .iter()
            .filter(|e| e.contains_point(x, y))
            .max_by(|a, b| {
                // Return the smallest (most specific) element that contains the point
                let area_a = a.bounds.2 * a.bounds.3;
                let area_b = b.bounds.2 * b.bounds.3;
                area_b.cmp(&area_a)
            })
    }

    /// Find elements by type
    pub fn elements_of_type(&self, element_type: crate::db::ElementType) -> Vec<&UiElement> {
        self.elements
            .iter()
            .filter(|e| e.element_type == element_type)
            .collect()
    }

    /// Find elements containing the given text
    pub fn elements_with_text(&self, text: &str) -> Vec<&UiElement> {
        let text_lower = text.to_lowercase();
        self.elements
            .iter()
            .filter(|e| {
                e.text
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&text_lower))
                    .unwrap_or(false)
            })
            .collect()
    }
}

// ============================================================================
// Recording Session
// ============================================================================

/// State of a recording session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingState {
    /// Recording is active
    Active,
    /// Recording is paused
    Paused,
    /// Recording has stopped
    Stopped,
}

/// A live recording session
#[derive(Debug)]
pub struct RecordingSession {
    /// Session ID
    pub id: String,
    /// The demonstration being recorded
    pub demonstration: Demonstration,
    /// Current recording state
    pub state: RecordingState,
    /// When the session started
    pub started_at: DateTime<Utc>,
    /// Screenshots captured at key moments
    pub key_screenshots: Vec<Screenshot>,
    /// Action sender for async recording
    action_tx: Option<mpsc::UnboundedSender<RecordedAction>>,
    /// Last screenshot for change detection (used for delta computation)
    #[allow(dead_code)]
    last_screenshot: Option<String>,
    /// Sequence counter for actions
    sequence_counter: u32,
}

impl RecordingSession {
    /// Create a new recording session
    pub fn new(app_name: impl Into<String>, task_description: impl Into<String>) -> Self {
        let demo = Demonstration::new(app_name, task_description);
        Self {
            id: Uuid::new_v4().to_string(),
            demonstration: demo,
            state: RecordingState::Active,
            started_at: Utc::now(),
            key_screenshots: Vec::new(),
            action_tx: None,
            last_screenshot: None,
            sequence_counter: 0,
        }
    }

    /// Record an action
    pub fn record_action(&mut self, mut action: RecordedAction) {
        if self.state != RecordingState::Active {
            return;
        }

        action.sequence = self.sequence_counter;
        self.sequence_counter += 1;

        // Send through channel if async recording
        if let Some(ref tx) = self.action_tx {
            let _ = tx.send(action.clone());
        }

        self.demonstration.add_action(action);
    }

    /// Record a mouse click
    pub fn record_mouse_click(&mut self, x: i32, y: i32, button: MouseButton, modifiers: Vec<Modifier>) {
        let action = RecordedAction::new(
            ActionType::MouseClick,
            ActionDetails::MouseClick { x, y, button, modifiers },
        );
        self.record_action(action);
    }

    /// Record text input
    pub fn record_text_input(&mut self, text: impl Into<String>) {
        let action = RecordedAction::new(
            ActionType::TextInput,
            ActionDetails::TextInput { text: text.into() },
        );
        self.record_action(action);
    }

    /// Record a keyboard shortcut
    pub fn record_keyboard_shortcut(&mut self, keys: Vec<String>, modifiers: Vec<Modifier>) {
        let action = RecordedAction::new(
            ActionType::KeyboardShortcut,
            ActionDetails::KeyboardShortcut { keys, modifiers },
        );
        self.record_action(action);
    }

    /// Capture a key moment screenshot
    pub fn capture_key_moment(&mut self, screenshot: Screenshot, reason: impl Into<String>) {
        self.demonstration.add_screenshot(screenshot.data.clone());
        self.key_screenshots.push(screenshot);

        // Also record as an action marker
        let action = RecordedAction::new(
            ActionType::Screenshot,
            ActionDetails::Screenshot { reason: reason.into() },
        );
        self.record_action(action);
    }

    /// Pause the recording
    pub fn pause(&mut self) {
        self.state = RecordingState::Paused;
    }

    /// Resume the recording
    pub fn resume(&mut self) {
        self.state = RecordingState::Active;
    }

    /// Stop the recording and finalize the demonstration
    pub fn stop(mut self) -> Demonstration {
        self.state = RecordingState::Stopped;
        let duration = Utc::now()
            .signed_duration_since(self.started_at)
            .num_milliseconds() as u64;
        self.demonstration.duration_ms = duration;
        self.demonstration
    }

    /// Get the current action count
    pub fn action_count(&self) -> usize {
        self.demonstration.action_count()
    }

    /// Check if recording is active
    pub fn is_active(&self) -> bool {
        self.state == RecordingState::Active
    }
}

// ============================================================================
// Skill Matcher
// ============================================================================

/// Match result between a skill and a context
#[derive(Debug, Clone)]
pub struct SkillMatch {
    /// The matched skill
    pub skill: Skill,
    /// Match confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Matched trigger patterns
    pub matched_patterns: Vec<String>,
    /// Reason for the match
    pub reason: String,
}

impl SkillMatch {
    /// Create a new skill match
    pub fn new(skill: Skill, confidence: f32, reason: impl Into<String>) -> Self {
        Self {
            skill,
            confidence,
            matched_patterns: Vec::new(),
            reason: reason.into(),
        }
    }
}

/// Configuration for skill matching
#[derive(Debug, Clone)]
pub struct MatchConfig {
    /// Minimum confidence threshold for matches
    pub min_confidence: f32,
    /// Maximum number of matches to return
    pub max_matches: usize,
    /// Weight for text similarity
    pub text_weight: f32,
    /// Weight for visual similarity
    pub visual_weight: f32,
    /// Weight for app name matching
    pub app_weight: f32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            max_matches: 5,
            text_weight: 0.5,
            visual_weight: 0.3,
            app_weight: 0.2,
        }
    }
}

/// Skill matching engine
pub struct SkillMatcher {
    config: MatchConfig,
}

impl SkillMatcher {
    /// Create a new skill matcher
    pub fn new(config: MatchConfig) -> Self {
        Self { config }
    }

    /// Calculate text similarity between two strings (simple Jaccard-like similarity)
    fn text_similarity(a: &str, b: &str) -> f32 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();
        let a_words: std::collections::HashSet<&str> = a_lower
            .split_whitespace()
            .collect();
        let b_words: std::collections::HashSet<&str> = b_lower
            .split_whitespace()
            .collect();

        if a_words.is_empty() || b_words.is_empty() {
            return 0.0;
        }

        let intersection = a_words.intersection(&b_words).count();
        let union = a_words.union(&b_words).count();

        intersection as f32 / union as f32
    }

    /// Check if a pattern matches a context string
    fn pattern_matches(pattern: &str, context: &str) -> bool {
        let pattern_lower = pattern.to_lowercase();
        let context_lower = context.to_lowercase();

        // Simple keyword matching with wildcards
        if pattern_lower.contains('*') {
            let parts: Vec<&str> = pattern_lower.split('*').collect();
            let mut pos = 0;
            for part in parts {
                if part.is_empty() {
                    continue;
                }
                if let Some(found_pos) = context_lower[pos..].find(part) {
                    pos = pos + found_pos + part.len();
                } else {
                    return false;
                }
            }
            true
        } else {
            context_lower.contains(&pattern_lower)
        }
    }

    /// Match skills against a context
    pub fn match_skills(
        &self,
        skills: &[Skill],
        context: &str,
        screenshot: &Screenshot,
    ) -> Vec<SkillMatch> {
        let mut matches: Vec<SkillMatch> = Vec::new();

        for skill in skills {
            if !skill.enabled {
                continue;
            }

            let mut confidence = 0.0;
            let mut matched_patterns = Vec::new();
            let mut reasons = Vec::new();

            // Check trigger patterns
            for pattern in &skill.trigger_patterns {
                if Self::pattern_matches(pattern, context) {
                    matched_patterns.push(pattern.clone());
                    confidence += 0.3;
                }
            }

            // Text similarity with skill description/name
            let desc_sim = Self::text_similarity(&skill.description, context);
            let name_sim = Self::text_similarity(&skill.name, context);
            let text_score = desc_sim.max(name_sim) * self.config.text_weight;
            if text_score > 0.1 {
                confidence += text_score;
                reasons.push(format!("text similarity: {:.2}", text_score));
            }

            // App name matching
            if let Some(ref app) = screenshot.app_name {
                if skill.applicable_apps.iter().any(|a| a.to_lowercase() == app.to_lowercase()) {
                    confidence += self.config.app_weight;
                    reasons.push(format!("app match: {}", app));
                } else if skill.applicable_apps.is_empty() {
                    // Skill is generic, slight bonus
                    confidence += self.config.app_weight * 0.3;
                }
            }

            // Boost confidence based on skill's historical success
            confidence *= 0.5 + (skill.confidence * 0.5);

            // Normalize confidence
            confidence = confidence.min(1.0);

            if confidence >= self.config.min_confidence {
                let reason = if reasons.is_empty() {
                    format!("pattern match: {:?}", matched_patterns)
                } else {
                    reasons.join(", ")
                };

                let mut skill_match = SkillMatch::new(skill.clone(), confidence, reason);
                skill_match.matched_patterns = matched_patterns;
                matches.push(skill_match);
            }
        }

        // Sort by confidence descending
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        // Limit results
        matches.truncate(self.config.max_matches);

        matches
    }
}

impl Default for SkillMatcher {
    fn default() -> Self {
        Self::new(MatchConfig::default())
    }
}

// ============================================================================
// Skill Extractor
// ============================================================================

/// Configuration for skill extraction
#[derive(Debug, Clone)]
pub struct ExtractionConfig {
    /// Minimum number of actions for a valid skill
    pub min_actions: usize,
    /// Whether to generalize coordinates to element-relative
    pub generalize_coordinates: bool,
    /// Whether to detect repeating patterns
    pub detect_patterns: bool,
    /// Merge similar consecutive text inputs
    pub merge_text_inputs: bool,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            min_actions: 1,
            generalize_coordinates: true,
            detect_patterns: true,
            merge_text_inputs: true,
        }
    }
}

/// Skill extraction engine
pub struct SkillExtractor {
    config: ExtractionConfig,
}

impl SkillExtractor {
    /// Create a new skill extractor
    pub fn new(config: ExtractionConfig) -> Self {
        Self { config }
    }

    /// Extract a skill from a demonstration
    pub fn extract(&self, demo: &Demonstration, name: &str) -> Result<Skill> {
        if demo.actions.len() < self.config.min_actions {
            return Err(Error::invalid(format!(
                "Demonstration has {} actions, minimum is {}",
                demo.actions.len(),
                self.config.min_actions
            )));
        }

        let mut skill = Skill::new(name, &demo.task_description);
        skill.add_learned_from(&demo.id);
        skill.applicable_apps.push(demo.app_name.clone());
        skill.tags = demo.tags.clone();

        // Extract trigger patterns from task description
        let trigger_patterns = self.extract_trigger_patterns(&demo.task_description);
        for pattern in trigger_patterns {
            skill.add_trigger_pattern(pattern);
        }

        // Convert actions to templates
        let actions = if self.config.merge_text_inputs {
            self.merge_text_inputs(&demo.actions)
        } else {
            demo.actions.clone()
        };

        for action in &actions {
            let template = self.action_to_template(action);
            skill.add_action_template(template);
        }

        // Detect repeating patterns in the action sequence
        if self.config.detect_patterns {
            self.detect_and_annotate_patterns(&mut skill);
        }

        Ok(skill)
    }

    /// Extract trigger patterns from task description
    fn extract_trigger_patterns(&self, description: &str) -> Vec<String> {
        let mut patterns = Vec::new();

        // Add the full description as a pattern
        patterns.push(description.to_lowercase());

        // Extract key verbs and nouns (simple heuristic)
        let keywords: Vec<&str> = description
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        if keywords.len() >= 2 {
            // Create wildcard patterns
            patterns.push(format!("{}*{}", keywords[0], keywords[keywords.len() - 1]).to_lowercase());
        }

        // Add individual significant words
        let significant_words = ["open", "close", "click", "navigate", "select", "type", "enter",
                                  "menu", "settings", "preferences", "options", "file", "edit",
                                  "view", "help", "tools", "window"];
        for word in &keywords {
            let word_lower = word.to_lowercase();
            if significant_words.iter().any(|&sw| word_lower.contains(sw)) {
                patterns.push(format!("*{}*", word_lower));
            }
        }

        patterns
    }

    /// Convert a recorded action to an action template
    fn action_to_template(&self, action: &RecordedAction) -> ActionTemplate {
        let mut template = ActionTemplate::from_action(action);

        // Extract preconditions and postconditions from screenshots if available
        if action.screen_before.is_some() {
            template.preconditions.push("screen_visible".to_string());
        }

        // Add target pattern from element info
        if let Some(ref element) = action.target_element {
            template.target_pattern = Some(format!(
                "{}:{}",
                element.element_type,
                element.text.as_deref().unwrap_or("*")
            ));

            // Add precondition that element must exist
            template.preconditions.push(format!("element_exists:{}", element.element_type));
        }

        // Generalize coordinates if enabled
        if self.config.generalize_coordinates && action.target_element.is_some() {
            // Add variable for relative position within element
            template.variables.push("element_center".to_string());
        }

        template
    }

    /// Merge consecutive text input actions
    fn merge_text_inputs(&self, actions: &[RecordedAction]) -> Vec<RecordedAction> {
        let mut result = Vec::new();
        let mut current_text = String::new();
        let mut text_start_action: Option<RecordedAction> = None;

        for action in actions {
            if let ActionDetails::TextInput { ref text } = action.details {
                if text_start_action.is_none() {
                    text_start_action = Some(action.clone());
                }
                current_text.push_str(text);
            } else {
                // Flush any accumulated text
                if !current_text.is_empty() {
                    if let Some(mut start_action) = text_start_action.take() {
                        start_action.details = ActionDetails::TextInput {
                            text: current_text.clone(),
                        };
                        result.push(start_action);
                        current_text.clear();
                    }
                }
                result.push(action.clone());
            }
        }

        // Flush any remaining text
        if !current_text.is_empty() {
            if let Some(mut start_action) = text_start_action {
                start_action.details = ActionDetails::TextInput { text: current_text };
                result.push(start_action);
            }
        }

        result
    }

    /// Detect repeating patterns in action sequences
    fn detect_and_annotate_patterns(&self, skill: &mut Skill) {
        // Simple pattern detection: look for repeated action type sequences
        if skill.action_template.len() < 4 {
            return;
        }

        let action_types: Vec<ActionType> = skill
            .action_template
            .iter()
            .map(|t| t.action_type)
            .collect();

        // Look for repeated pairs or triples
        for window_size in [2, 3] {
            let mut counts: HashMap<String, usize> = HashMap::new();

            for window in action_types.windows(window_size) {
                let key = window
                    .iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join("-");
                *counts.entry(key).or_insert(0) += 1;
            }

            // Add patterns that appear multiple times
            for (pattern, count) in counts {
                if count >= 2 {
                    skill.trigger_patterns.push(format!("action_pattern:{}", pattern));
                }
            }
        }
    }

    /// Generalize a skill by finding common patterns across multiple demonstrations
    pub fn generalize_skill(&self, skill: &Skill, additional_demo: &Demonstration) -> Result<Skill> {
        let mut generalized = skill.clone();

        // Add the new demonstration as a source
        generalized.add_learned_from(&additional_demo.id);

        // Add the app if it's different
        if !generalized.applicable_apps.contains(&additional_demo.app_name) {
            generalized.applicable_apps.push(additional_demo.app_name.clone());
        }

        // Extract trigger patterns from the new demo
        let new_patterns = self.extract_trigger_patterns(&additional_demo.task_description);
        for pattern in new_patterns {
            if !generalized.trigger_patterns.contains(&pattern) {
                generalized.add_trigger_pattern(pattern);
            }
        }

        // Find common action patterns between the skill and new demo
        // (simplified - just verify similar structure)
        let new_action_types: Vec<ActionType> = additional_demo
            .actions
            .iter()
            .map(|a| a.action_type)
            .collect();

        let skill_action_types: Vec<ActionType> = generalized
            .action_template
            .iter()
            .map(|t| t.action_type)
            .collect();

        // Check for structural similarity
        let similarity = self.action_sequence_similarity(&skill_action_types, &new_action_types);
        if similarity < 0.5 {
            warn!(
                "Demonstration action sequence is quite different (similarity: {:.2})",
                similarity
            );
        }

        Ok(generalized)
    }

    /// Calculate similarity between two action sequences
    fn action_sequence_similarity(&self, a: &[ActionType], b: &[ActionType]) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        // Simple longest common subsequence ratio
        let lcs_len = self.lcs_length(a, b);
        let max_len = a.len().max(b.len());

        lcs_len as f32 / max_len as f32
    }

    /// Calculate longest common subsequence length
    fn lcs_length(&self, a: &[ActionType], b: &[ActionType]) -> usize {
        let m = a.len();
        let n = b.len();
        let mut dp = vec![vec![0; n + 1]; m + 1];

        for i in 1..=m {
            for j in 1..=n {
                if a[i - 1] == b[j - 1] {
                    dp[i][j] = dp[i - 1][j - 1] + 1;
                } else {
                    dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
                }
            }
        }

        dp[m][n]
    }
}

impl Default for SkillExtractor {
    fn default() -> Self {
        Self::new(ExtractionConfig::default())
    }
}

// ============================================================================
// Skill Applicator
// ============================================================================

/// Configuration for skill application
#[derive(Debug, Clone)]
pub struct ApplicationConfig {
    /// Default delay between actions (ms)
    pub default_delay_ms: u64,
    /// Whether to verify preconditions
    pub verify_preconditions: bool,
    /// Whether to adapt coordinates to current screen
    pub adapt_coordinates: bool,
    /// Timeout for finding target elements (ms)
    pub element_timeout_ms: u64,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            default_delay_ms: 100,
            verify_preconditions: true,
            adapt_coordinates: true,
            element_timeout_ms: 5000,
        }
    }
}

/// Result of skill application
#[derive(Debug, Clone)]
pub struct ApplicationResult {
    /// Generated actions to execute
    pub actions: Vec<Action>,
    /// Warnings during application
    pub warnings: Vec<String>,
    /// Whether all preconditions were met
    pub preconditions_met: bool,
    /// Confidence in the application
    pub confidence: f32,
}

/// Skill application engine
pub struct SkillApplicator {
    config: ApplicationConfig,
}

impl SkillApplicator {
    /// Create a new skill applicator
    pub fn new(config: ApplicationConfig) -> Self {
        Self { config }
    }

    /// Apply a skill to the current screenshot
    pub fn apply(&self, skill: &Skill, screenshot: &Screenshot) -> Result<ApplicationResult> {
        let mut actions = Vec::new();
        let mut warnings = Vec::new();
        let mut preconditions_met = true;
        let mut confidence = skill.confidence;

        for template in &skill.action_template {
            // Check preconditions
            if self.config.verify_preconditions {
                for precondition in &template.preconditions {
                    if !self.check_precondition(precondition, screenshot) {
                        warnings.push(format!("Precondition not met: {}", precondition));
                        preconditions_met = false;
                        confidence *= 0.8;
                    }
                }
            }

            // Adapt the action to the current screen
            let action = self.adapt_template(template, screenshot)?;
            actions.push(action);
        }

        // Add delays between actions
        for (i, action) in actions.iter_mut().enumerate() {
            if i > 0 {
                action.delay_ms = self.config.default_delay_ms;
            }
        }

        Ok(ApplicationResult {
            actions,
            warnings,
            preconditions_met,
            confidence,
        })
    }

    /// Check if a precondition is met
    fn check_precondition(&self, precondition: &str, screenshot: &Screenshot) -> bool {
        if precondition == "screen_visible" {
            return !screenshot.data.is_empty();
        }

        if precondition.starts_with("element_exists:") {
            let element_type = &precondition["element_exists:".len()..];
            return screenshot
                .elements
                .iter()
                .any(|e| e.element_type.to_string() == element_type);
        }

        // Unknown precondition, assume met
        true
    }

    /// Adapt a template to the current screenshot
    fn adapt_template(&self, template: &ActionTemplate, screenshot: &Screenshot) -> Result<Action> {
        let mut details = template.details_template.clone();

        // Adapt coordinates if we have a target pattern
        if self.config.adapt_coordinates {
            if let Some(ref pattern) = template.target_pattern {
                if let Some(element) = self.find_matching_element(pattern, screenshot) {
                    details = self.update_coordinates(details, element);
                }
            }
        }

        let description = format!(
            "{:?} action from skill template",
            template.action_type
        );

        Ok(Action::new(template.action_type, details, description))
    }

    /// Find an element matching a pattern
    fn find_matching_element<'a>(&self, pattern: &str, screenshot: &'a Screenshot) -> Option<&'a UiElement> {
        // Pattern format: "element_type:text" or "element_type:*"
        let parts: Vec<&str> = pattern.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let target_type = parts[0];
        let target_text = parts[1];

        screenshot.elements.iter().find(|e| {
            let type_matches = e.element_type.to_string() == target_type;
            let text_matches = if target_text == "*" {
                true
            } else {
                e.text
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&target_text.to_lowercase()))
                    .unwrap_or(false)
            };
            type_matches && text_matches
        })
    }

    /// Update action coordinates to target an element
    fn update_coordinates(&self, details: ActionDetails, element: &UiElement) -> ActionDetails {
        let (cx, cy) = element.center();

        match details {
            ActionDetails::MouseClick { button, modifiers, .. } => {
                ActionDetails::MouseClick {
                    x: cx,
                    y: cy,
                    button,
                    modifiers,
                }
            }
            ActionDetails::MouseDoubleClick { button, .. } => {
                ActionDetails::MouseDoubleClick {
                    x: cx,
                    y: cy,
                    button,
                }
            }
            ActionDetails::MouseMove { .. } => {
                ActionDetails::MouseMove { x: cx, y: cy }
            }
            other => other,
        }
    }
}

impl Default for SkillApplicator {
    fn default() -> Self {
        Self::new(ApplicationConfig::default())
    }
}

// ============================================================================
// Learning Engine
// ============================================================================

/// The main learning engine that coordinates recording, extraction, and application
pub struct LearningEngine {
    /// Database for persistence
    db: Arc<Database>,
    /// Current recording session
    current_session: Arc<Mutex<Option<RecordingSession>>>,
    /// Skill matcher
    matcher: SkillMatcher,
    /// Skill extractor
    extractor: SkillExtractor,
    /// Skill applicator
    applicator: SkillApplicator,
    /// Cached skills (for faster matching)
    skill_cache: Arc<RwLock<Vec<Skill>>>,
    /// Last cache refresh time
    cache_refresh_time: Arc<Mutex<DateTime<Utc>>>,
}

impl LearningEngine {
    /// Create a new learning engine
    pub fn new(db: Database) -> Self {
        let db = Arc::new(db);
        Self {
            db,
            current_session: Arc::new(Mutex::new(None)),
            matcher: SkillMatcher::default(),
            extractor: SkillExtractor::default(),
            applicator: SkillApplicator::default(),
            skill_cache: Arc::new(RwLock::new(Vec::new())),
            cache_refresh_time: Arc::new(Mutex::new(Utc::now())),
        }
    }

    /// Create with custom components
    pub fn with_components(
        db: Database,
        matcher: SkillMatcher,
        extractor: SkillExtractor,
        applicator: SkillApplicator,
    ) -> Self {
        let db = Arc::new(db);
        Self {
            db,
            current_session: Arc::new(Mutex::new(None)),
            matcher,
            extractor,
            applicator,
            skill_cache: Arc::new(RwLock::new(Vec::new())),
            cache_refresh_time: Arc::new(Mutex::new(Utc::now())),
        }
    }

    /// Get a reference to the database
    pub fn database(&self) -> &Database {
        &self.db
    }

    // ========================================================================
    // Recording
    // ========================================================================

    /// Start recording a demonstration
    pub fn start_recording(
        &self,
        app_name: impl Into<String>,
        task_description: impl Into<String>,
    ) -> Result<String> {
        let mut session_guard = self.current_session.lock()
            .map_err(|_| Error::invalid("Lock poisoned"))?;

        if session_guard.is_some() {
            return Err(Error::invalid("Recording already in progress"));
        }

        let session = RecordingSession::new(app_name, task_description);
        let session_id = session.id.clone();

        info!("Starting recording session: {}", session_id);

        *session_guard = Some(session);
        Ok(session_id)
    }

    /// Stop the current recording and return the demonstration
    pub fn stop_recording(&self) -> Result<Demonstration> {
        let mut session_guard = self.current_session.lock()
            .map_err(|_| Error::invalid("Lock poisoned"))?;

        let session = session_guard.take()
            .ok_or_else(|| Error::invalid("No active recording session"))?;

        let demo = session.stop();

        info!(
            "Stopped recording session. Captured {} actions.",
            demo.action_count()
        );

        // Save to database
        self.db.save_demonstration(&demo)?;

        Ok(demo)
    }

    /// Record an action in the current session
    pub fn record_action(&self, action: RecordedAction) -> Result<()> {
        let mut session_guard = self.current_session.lock()
            .map_err(|_| Error::invalid("Lock poisoned"))?;

        let session = session_guard.as_mut()
            .ok_or_else(|| Error::invalid("No active recording session"))?;

        session.record_action(action);
        Ok(())
    }

    /// Record a mouse click
    pub fn record_click(&self, x: i32, y: i32, button: MouseButton) -> Result<()> {
        let action = RecordedAction::new(
            ActionType::MouseClick,
            ActionDetails::MouseClick {
                x,
                y,
                button,
                modifiers: vec![],
            },
        );
        self.record_action(action)
    }

    /// Record text input
    pub fn record_text(&self, text: impl Into<String>) -> Result<()> {
        let action = RecordedAction::new(
            ActionType::TextInput,
            ActionDetails::TextInput { text: text.into() },
        );
        self.record_action(action)
    }

    /// Capture a key moment screenshot
    pub fn capture_key_moment(&self, screenshot: Screenshot, reason: impl Into<String>) -> Result<()> {
        let mut session_guard = self.current_session.lock()
            .map_err(|_| Error::invalid("Lock poisoned"))?;

        let session = session_guard.as_mut()
            .ok_or_else(|| Error::invalid("No active recording session"))?;

        session.capture_key_moment(screenshot, reason);
        Ok(())
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.current_session.lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Get the current action count
    pub fn current_action_count(&self) -> usize {
        self.current_session.lock()
            .map(|guard| {
                guard.as_ref().map(|s| s.action_count()).unwrap_or(0)
            })
            .unwrap_or(0)
    }

    // ========================================================================
    // Skill Extraction
    // ========================================================================

    /// Extract a skill from a demonstration
    pub fn extract_skill(&self, demo: &Demonstration, name: &str) -> Result<Skill> {
        let skill = self.extractor.extract(demo, name)?;

        info!(
            "Extracted skill '{}' with {} action templates",
            skill.name,
            skill.action_template.len()
        );

        // Save to database
        self.db.save_skill(&skill)?;

        // Invalidate cache
        self.invalidate_cache();

        Ok(skill)
    }

    /// Generalize an existing skill with a new demonstration
    pub fn generalize_skill(&self, skill_id: &str, demo: &Demonstration) -> Result<Skill> {
        let skill = self.db.get_skill(skill_id)?
            .ok_or_else(|| Error::not_found(format!("Skill not found: {}", skill_id)))?;

        let generalized = self.extractor.generalize_skill(&skill, demo)?;

        info!(
            "Generalized skill '{}', now applicable to {} apps",
            generalized.name,
            generalized.applicable_apps.len()
        );

        // Save updated skill
        self.db.save_skill(&generalized)?;

        // Invalidate cache
        self.invalidate_cache();

        Ok(generalized)
    }

    // ========================================================================
    // Skill Matching
    // ========================================================================

    /// Find relevant skills for a context and screenshot
    pub fn find_relevant_skills(
        &self,
        context: &str,
        screenshot: &Screenshot,
    ) -> Result<Vec<SkillMatch>> {
        // Ensure cache is fresh
        self.refresh_cache_if_needed()?;

        let skills = self.skill_cache.read()
            .map_err(|_| Error::invalid("Lock poisoned"))?;

        let matches = self.matcher.match_skills(&skills, context, screenshot);

        debug!(
            "Found {} relevant skills for context: '{}'",
            matches.len(),
            context
        );

        Ok(matches)
    }

    /// Refresh the skill cache
    fn refresh_cache_if_needed(&self) -> Result<()> {
        let refresh_time = self.cache_refresh_time.lock()
            .map_err(|_| Error::invalid("Lock poisoned"))?;

        // Refresh if cache is older than 5 minutes
        let cache_age = Utc::now().signed_duration_since(*refresh_time);
        if cache_age.num_seconds() > 300 {
            drop(refresh_time);
            self.refresh_cache()?;
        }

        Ok(())
    }

    /// Force refresh the skill cache
    pub fn refresh_cache(&self) -> Result<()> {
        let skills = self.db.list_skills(true)?;

        {
            let mut cache = self.skill_cache.write()
                .map_err(|_| Error::invalid("Lock poisoned"))?;
            *cache = skills;
        }

        {
            let mut refresh_time = self.cache_refresh_time.lock()
                .map_err(|_| Error::invalid("Lock poisoned"))?;
            *refresh_time = Utc::now();
        }

        debug!("Refreshed skill cache");
        Ok(())
    }

    /// Invalidate the skill cache
    fn invalidate_cache(&self) {
        if let Ok(mut refresh_time) = self.cache_refresh_time.lock() {
            // Set to epoch to force refresh
            *refresh_time = DateTime::from_timestamp(0, 0)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
        }
    }

    // ========================================================================
    // Skill Application
    // ========================================================================

    /// Apply a skill to the current screen
    pub fn apply_skill(&self, skill: &Skill, screenshot: &Screenshot) -> Result<Vec<Action>> {
        let result = self.applicator.apply(skill, screenshot)?;

        if !result.warnings.is_empty() {
            for warning in &result.warnings {
                warn!("Skill application warning: {}", warning);
            }
        }

        info!(
            "Applied skill '{}' with {} actions (confidence: {:.2})",
            skill.name,
            result.actions.len(),
            result.confidence
        );

        Ok(result.actions)
    }

    /// Apply a skill and get detailed result
    pub fn apply_skill_detailed(
        &self,
        skill: &Skill,
        screenshot: &Screenshot,
    ) -> Result<ApplicationResult> {
        self.applicator.apply(skill, screenshot)
    }

    // ========================================================================
    // Outcome Tracking
    // ========================================================================

    /// Report the outcome of a skill application
    pub fn report_outcome(&self, skill_id: &str, success: bool) -> Result<()> {
        self.db.update_skill_outcome(skill_id, success)?;
        self.db.log_skill_application(skill_id, success, None, None)?;

        info!(
            "Recorded {} for skill {}",
            if success { "success" } else { "failure" },
            skill_id
        );

        // Invalidate cache to reflect updated confidence
        self.invalidate_cache();

        Ok(())
    }

    /// Report outcome with additional context
    pub fn report_outcome_with_context(
        &self,
        skill_id: &str,
        success: bool,
        context: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        self.db.update_skill_outcome(skill_id, success)?;
        self.db.log_skill_application(skill_id, success, Some(context), notes)?;

        // Invalidate cache
        self.invalidate_cache();

        Ok(())
    }

    // ========================================================================
    // Demonstration Management
    // ========================================================================

    /// Get a demonstration by ID
    pub fn get_demonstration(&self, id: &str) -> Result<Option<Demonstration>> {
        self.db.get_demonstration(id)
    }

    /// List demonstrations
    pub fn list_demonstrations(&self, app_filter: Option<&str>, limit: usize) -> Result<Vec<Demonstration>> {
        self.db.list_demonstrations(app_filter, limit)
    }

    /// Search demonstrations
    pub fn search_demonstrations(&self, query: &str, limit: usize) -> Result<Vec<Demonstration>> {
        self.db.search_demonstrations(query, limit)
    }

    /// Delete a demonstration
    pub fn delete_demonstration(&self, id: &str) -> Result<bool> {
        self.db.delete_demonstration(id)
    }

    // ========================================================================
    // Skill Management
    // ========================================================================

    /// Get a skill by ID
    pub fn get_skill(&self, id: &str) -> Result<Option<Skill>> {
        self.db.get_skill(id)
    }

    /// List all skills
    pub fn list_skills(&self, enabled_only: bool) -> Result<Vec<Skill>> {
        self.db.list_skills(enabled_only)
    }

    /// Delete a skill
    pub fn delete_skill(&self, id: &str) -> Result<bool> {
        let result = self.db.delete_skill(id)?;
        if result {
            self.invalidate_cache();
        }
        Ok(result)
    }

    /// Enable or disable a skill
    pub fn set_skill_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        if let Some(mut skill) = self.db.get_skill(id)? {
            skill.enabled = enabled;
            self.db.save_skill(&skill)?;
            self.invalidate_cache();
        }
        Ok(())
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get learning statistics
    pub fn get_statistics(&self) -> Result<LearningStatistics> {
        let demonstrations = self.db.list_demonstrations(None, 1000)?;
        let skills = self.db.list_skills(false)?;

        let total_demonstrations = demonstrations.len();
        let total_skills = skills.len();
        let enabled_skills = skills.iter().filter(|s| s.enabled).count();

        let total_actions: usize = demonstrations.iter()
            .map(|d| d.action_count())
            .sum();

        let total_success: u32 = skills.iter().map(|s| s.success_count).sum();
        let total_failure: u32 = skills.iter().map(|s| s.failure_count).sum();

        let average_confidence = if skills.is_empty() {
            0.0
        } else {
            skills.iter().map(|s| s.confidence).sum::<f32>() / skills.len() as f32
        };

        let apps: std::collections::HashSet<String> = demonstrations
            .iter()
            .map(|d| d.app_name.clone())
            .collect();

        Ok(LearningStatistics {
            total_demonstrations,
            total_skills,
            enabled_skills,
            total_actions,
            total_applications: total_success + total_failure,
            total_successes: total_success,
            total_failures: total_failure,
            average_skill_confidence: average_confidence,
            unique_apps: apps.len(),
            apps: apps.into_iter().collect(),
        })
    }
}

/// Learning system statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStatistics {
    /// Total number of demonstrations recorded
    pub total_demonstrations: usize,
    /// Total number of skills extracted
    pub total_skills: usize,
    /// Number of enabled skills
    pub enabled_skills: usize,
    /// Total actions across all demonstrations
    pub total_actions: usize,
    /// Total skill applications
    pub total_applications: u32,
    /// Total successful applications
    pub total_successes: u32,
    /// Total failed applications
    pub total_failures: u32,
    /// Average skill confidence
    pub average_skill_confidence: f32,
    /// Number of unique apps with demonstrations
    pub unique_apps: usize,
    /// List of apps with demonstrations
    pub apps: Vec<String>,
}

impl LearningStatistics {
    /// Get the overall success rate
    pub fn success_rate(&self) -> f32 {
        if self.total_applications == 0 {
            0.5
        } else {
            self.total_successes as f32 / self.total_applications as f32
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_engine() -> LearningEngine {
        let db = Database::in_memory().expect("Failed to create in-memory database");
        LearningEngine::new(db)
    }

    fn create_test_screenshot() -> Screenshot {
        Screenshot::new("base64_data".to_string(), 1920, 1080)
            .with_app_info("TestApp", "Test Window")
    }

    #[test]
    fn test_recording_session() {
        let engine = create_test_engine();

        // Start recording
        let session_id = engine.start_recording("Firefox", "Open settings").unwrap();
        assert!(!session_id.is_empty());
        assert!(engine.is_recording());

        // Record some actions
        engine.record_click(100, 200, MouseButton::Left).unwrap();
        engine.record_text("Hello").unwrap();

        assert_eq!(engine.current_action_count(), 2);

        // Stop recording
        let demo = engine.stop_recording().unwrap();
        assert_eq!(demo.app_name, "Firefox");
        assert_eq!(demo.action_count(), 2);
        assert!(!engine.is_recording());
    }

    #[test]
    fn test_skill_extraction() {
        let engine = create_test_engine();

        // Create a demonstration
        let mut demo = Demonstration::new("Blender", "Navigate to render settings");
        demo.add_action(RecordedAction::new(
            ActionType::MouseClick,
            ActionDetails::MouseClick {
                x: 100,
                y: 50,
                button: MouseButton::Left,
                modifiers: vec![],
            },
        ));
        demo.add_action(RecordedAction::new(
            ActionType::MouseClick,
            ActionDetails::MouseClick {
                x: 150,
                y: 100,
                button: MouseButton::Left,
                modifiers: vec![],
            },
        ));

        engine.database().save_demonstration(&demo).unwrap();

        // Extract a skill
        let skill = engine.extract_skill(&demo, "Navigate menu").unwrap();

        assert_eq!(skill.name, "Navigate menu");
        assert_eq!(skill.action_template.len(), 2);
        assert!(skill.learned_from.contains(&demo.id));
        assert!(skill.applicable_apps.contains(&"Blender".to_string()));
    }

    #[test]
    fn test_skill_matching() {
        let engine = create_test_engine();

        // Create and save a skill
        let mut skill = Skill::new("Open Preferences", "Navigate to application preferences");
        skill.trigger_patterns.push("*preferences*".to_string());
        skill.trigger_patterns.push("*settings*".to_string());
        skill.applicable_apps.push("TestApp".to_string());
        skill.enabled = true;

        engine.database().save_skill(&skill).unwrap();
        engine.refresh_cache().unwrap();

        // Find relevant skills
        let screenshot = create_test_screenshot();
        let matches = engine.find_relevant_skills("open preferences dialog", &screenshot).unwrap();

        assert!(!matches.is_empty());
        assert_eq!(matches[0].skill.id, skill.id);
    }

    #[test]
    fn test_outcome_tracking() {
        let engine = create_test_engine();

        // Create a skill
        let skill = Skill::new("Test Skill", "A test skill");
        engine.database().save_skill(&skill).unwrap();

        // Report outcomes
        engine.report_outcome(&skill.id, true).unwrap();
        engine.report_outcome(&skill.id, true).unwrap();
        engine.report_outcome(&skill.id, false).unwrap();

        // Check updated skill
        let updated = engine.get_skill(&skill.id).unwrap().unwrap();
        assert_eq!(updated.success_count, 2);
        assert_eq!(updated.failure_count, 1);
        assert!(updated.confidence > 0.5);
    }

    #[test]
    fn test_text_similarity() {
        let similarity = SkillMatcher::text_similarity(
            "open the settings menu",
            "navigate to settings dialog",
        );
        assert!(similarity > 0.0);
        assert!(similarity < 1.0);

        let exact = SkillMatcher::text_similarity(
            "open settings",
            "open settings",
        );
        assert!((exact - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_statistics() {
        let engine = create_test_engine();

        // Create some data
        let demo = Demonstration::new("App1", "Task 1");
        engine.database().save_demonstration(&demo).unwrap();

        let mut skill = Skill::new("Skill 1", "Description");
        skill.success_count = 5;
        skill.failure_count = 1;
        engine.database().save_skill(&skill).unwrap();

        // Get statistics
        let stats = engine.get_statistics().unwrap();
        assert_eq!(stats.total_demonstrations, 1);
        assert_eq!(stats.total_skills, 1);
        assert_eq!(stats.unique_apps, 1);
    }

    #[test]
    fn test_screenshot_element_search() {
        let mut screenshot = Screenshot::new("data".to_string(), 1920, 1080);

        screenshot.elements.push(
            UiElement::new(crate::db::ElementType::Button, (100, 100, 50, 30))
                .with_text("OK")
        );
        screenshot.elements.push(
            UiElement::new(crate::db::ElementType::TextInput, (200, 100, 150, 30))
                .with_text("Search...")
        );

        // Find element at point
        let element = screenshot.element_at(125, 115);
        assert!(element.is_some());
        assert_eq!(element.unwrap().text, Some("OK".to_string()));

        // Find elements with text
        let elements = screenshot.elements_with_text("search");
        assert_eq!(elements.len(), 1);

        // Find elements by type
        let buttons = screenshot.elements_of_type(crate::db::ElementType::Button);
        assert_eq!(buttons.len(), 1);
    }
}
