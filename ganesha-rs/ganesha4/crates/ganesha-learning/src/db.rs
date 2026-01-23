//! Database layer for Ganesha Vision
//!
//! Provides SQLite-backed persistence for demonstrations, skills, and UI patterns.
//! This is the foundation for the learning-from-demonstration system.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::error::{Error, Result};

// ============================================================================
// Core Types
// ============================================================================

/// Type of UI element detected in a screenshot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    Button,
    TextInput,
    Checkbox,
    RadioButton,
    Dropdown,
    Menu,
    MenuItem,
    Tab,
    Slider,
    ScrollBar,
    Icon,
    Image,
    Text,
    Link,
    Window,
    Dialog,
    Toolbar,
    StatusBar,
    TreeItem,
    ListItem,
    TableCell,
    Canvas,
    Unknown,
}

impl std::fmt::Display for ElementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ElementType::Button => "button",
            ElementType::TextInput => "text_input",
            ElementType::Checkbox => "checkbox",
            ElementType::RadioButton => "radio_button",
            ElementType::Dropdown => "dropdown",
            ElementType::Menu => "menu",
            ElementType::MenuItem => "menu_item",
            ElementType::Tab => "tab",
            ElementType::Slider => "slider",
            ElementType::ScrollBar => "scroll_bar",
            ElementType::Icon => "icon",
            ElementType::Image => "image",
            ElementType::Text => "text",
            ElementType::Link => "link",
            ElementType::Window => "window",
            ElementType::Dialog => "dialog",
            ElementType::Toolbar => "toolbar",
            ElementType::StatusBar => "status_bar",
            ElementType::TreeItem => "tree_item",
            ElementType::ListItem => "list_item",
            ElementType::TableCell => "table_cell",
            ElementType::Canvas => "canvas",
            ElementType::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for ElementType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "button" => Ok(ElementType::Button),
            "text_input" | "textinput" | "input" => Ok(ElementType::TextInput),
            "checkbox" => Ok(ElementType::Checkbox),
            "radio_button" | "radiobutton" | "radio" => Ok(ElementType::RadioButton),
            "dropdown" | "select" | "combobox" => Ok(ElementType::Dropdown),
            "menu" => Ok(ElementType::Menu),
            "menu_item" | "menuitem" => Ok(ElementType::MenuItem),
            "tab" => Ok(ElementType::Tab),
            "slider" => Ok(ElementType::Slider),
            "scroll_bar" | "scrollbar" => Ok(ElementType::ScrollBar),
            "icon" => Ok(ElementType::Icon),
            "image" | "img" => Ok(ElementType::Image),
            "text" | "label" => Ok(ElementType::Text),
            "link" | "anchor" => Ok(ElementType::Link),
            "window" => Ok(ElementType::Window),
            "dialog" | "modal" => Ok(ElementType::Dialog),
            "toolbar" => Ok(ElementType::Toolbar),
            "status_bar" | "statusbar" => Ok(ElementType::StatusBar),
            "tree_item" | "treeitem" => Ok(ElementType::TreeItem),
            "list_item" | "listitem" => Ok(ElementType::ListItem),
            "table_cell" | "tablecell" | "cell" => Ok(ElementType::TableCell),
            "canvas" => Ok(ElementType::Canvas),
            _ => Ok(ElementType::Unknown),
        }
    }
}

/// Type of user action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Mouse click (left, right, middle)
    MouseClick,
    /// Mouse double-click
    MouseDoubleClick,
    /// Mouse drag operation
    MouseDrag,
    /// Mouse scroll (wheel)
    MouseScroll,
    /// Mouse move/hover
    MouseMove,
    /// Keyboard key press
    KeyPress,
    /// Keyboard key release
    KeyRelease,
    /// Text input (sequence of characters)
    TextInput,
    /// Keyboard shortcut (e.g., Ctrl+C)
    KeyboardShortcut,
    /// Wait/delay
    Wait,
    /// Screenshot capture marker
    Screenshot,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ActionType::MouseClick => "mouse_click",
            ActionType::MouseDoubleClick => "mouse_double_click",
            ActionType::MouseDrag => "mouse_drag",
            ActionType::MouseScroll => "mouse_scroll",
            ActionType::MouseMove => "mouse_move",
            ActionType::KeyPress => "key_press",
            ActionType::KeyRelease => "key_release",
            ActionType::TextInput => "text_input",
            ActionType::KeyboardShortcut => "keyboard_shortcut",
            ActionType::Wait => "wait",
            ActionType::Screenshot => "screenshot",
        };
        write!(f, "{}", s)
    }
}

/// Mouse button type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Meta, // Windows/Command key
}

/// Detailed information about an action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionDetails {
    MouseClick {
        x: i32,
        y: i32,
        button: MouseButton,
        modifiers: Vec<Modifier>,
    },
    MouseDoubleClick {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    MouseDrag {
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        button: MouseButton,
    },
    MouseScroll {
        x: i32,
        y: i32,
        delta_x: i32,
        delta_y: i32,
    },
    MouseMove {
        x: i32,
        y: i32,
    },
    KeyPress {
        key: String,
        modifiers: Vec<Modifier>,
    },
    KeyRelease {
        key: String,
    },
    TextInput {
        text: String,
    },
    KeyboardShortcut {
        keys: Vec<String>,
        modifiers: Vec<Modifier>,
    },
    Wait {
        duration_ms: u64,
    },
    Screenshot {
        reason: String,
    },
}

impl ActionDetails {
    /// Get the position for mouse actions
    pub fn position(&self) -> Option<(i32, i32)> {
        match self {
            ActionDetails::MouseClick { x, y, .. } => Some((*x, *y)),
            ActionDetails::MouseDoubleClick { x, y, .. } => Some((*x, *y)),
            ActionDetails::MouseDrag { start_x, start_y, .. } => Some((*start_x, *start_y)),
            ActionDetails::MouseScroll { x, y, .. } => Some((*x, *y)),
            ActionDetails::MouseMove { x, y } => Some((*x, *y)),
            _ => None,
        }
    }

    /// Check if this action requires text input
    pub fn has_text_input(&self) -> bool {
        matches!(self, ActionDetails::TextInput { .. })
    }
}

/// Outcome of a demonstration or skill application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Failure,
    Partial,
    Unknown,
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Outcome::Success => "success",
            Outcome::Failure => "failure",
            Outcome::Partial => "partial",
            Outcome::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

// ============================================================================
// Recorded Action
// ============================================================================

/// A single recorded user action during a demonstration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedAction {
    /// Unique action ID
    pub id: String,
    /// Timestamp when the action occurred
    pub timestamp: DateTime<Utc>,
    /// Type of action
    pub action_type: ActionType,
    /// Detailed action information
    pub details: ActionDetails,
    /// Screenshot before the action (base64 encoded, optional)
    pub screen_before: Option<String>,
    /// Screenshot after the action (base64 encoded, optional)
    pub screen_after: Option<String>,
    /// Detected UI element at the action location
    pub target_element: Option<UiElement>,
    /// Duration of the action in milliseconds
    pub duration_ms: Option<u64>,
    /// Sequence number within the demonstration
    pub sequence: u32,
}

impl RecordedAction {
    /// Create a new recorded action
    pub fn new(action_type: ActionType, details: ActionDetails) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            action_type,
            details,
            screen_before: None,
            screen_after: None,
            target_element: None,
            duration_ms: None,
            sequence: 0,
        }
    }

    /// Set the screenshot before the action
    pub fn with_screen_before(mut self, screenshot: String) -> Self {
        self.screen_before = Some(screenshot);
        self
    }

    /// Set the screenshot after the action
    pub fn with_screen_after(mut self, screenshot: String) -> Self {
        self.screen_after = Some(screenshot);
        self
    }

    /// Set the target UI element
    pub fn with_target_element(mut self, element: UiElement) -> Self {
        self.target_element = Some(element);
        self
    }

    /// Set the action duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Set the sequence number
    pub fn with_sequence(mut self, sequence: u32) -> Self {
        self.sequence = sequence;
        self
    }
}

// ============================================================================
// UI Element
// ============================================================================

/// A detected UI element in a screenshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElement {
    /// Element type
    pub element_type: ElementType,
    /// Bounding box (x, y, width, height)
    pub bounds: (i32, i32, i32, i32),
    /// Text content if any
    pub text: Option<String>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Parent element ID if nested
    pub parent_id: Option<String>,
    /// Additional attributes
    pub attributes: std::collections::HashMap<String, String>,
}

impl UiElement {
    /// Create a new UI element
    pub fn new(element_type: ElementType, bounds: (i32, i32, i32, i32)) -> Self {
        Self {
            element_type,
            bounds,
            text: None,
            confidence: 1.0,
            parent_id: None,
            attributes: std::collections::HashMap::new(),
        }
    }

    /// Set the text content
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Set the confidence score
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    /// Check if a point is within this element's bounds
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        let (bx, by, bw, bh) = self.bounds;
        x >= bx && x < bx + bw && y >= by && y < by + bh
    }

    /// Get the center point of this element
    pub fn center(&self) -> (i32, i32) {
        let (x, y, w, h) = self.bounds;
        (x + w / 2, y + h / 2)
    }
}

// ============================================================================
// UI Pattern
// ============================================================================

/// A recognized UI pattern that can be matched across applications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPattern {
    /// Unique pattern ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of the pattern
    pub description: String,
    /// Visual features that identify this pattern
    pub visual_features: Vec<String>,
    /// Typical element types involved
    pub element_types: Vec<ElementType>,
    /// Keywords that might appear in the pattern
    pub keywords: Vec<String>,
    /// Example screenshots (base64)
    pub examples: Vec<String>,
    /// Confidence threshold for matching
    pub match_threshold: f32,
    /// When this pattern was created
    pub created_at: DateTime<Utc>,
    /// When this pattern was last updated
    pub updated_at: DateTime<Utc>,
    /// Times this pattern has been successfully matched
    pub match_count: u32,
}

impl UiPattern {
    /// Create a new UI pattern
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            visual_features: Vec::new(),
            element_types: Vec::new(),
            keywords: Vec::new(),
            examples: Vec::new(),
            match_threshold: 0.7,
            created_at: now,
            updated_at: now,
            match_count: 0,
        }
    }

    /// Add a visual feature
    pub fn with_visual_feature(mut self, feature: impl Into<String>) -> Self {
        self.visual_features.push(feature.into());
        self
    }

    /// Add element types
    pub fn with_element_types(mut self, types: Vec<ElementType>) -> Self {
        self.element_types = types;
        self
    }

    /// Add keywords
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }
}

// ============================================================================
// Demonstration
// ============================================================================

/// A recorded user demonstration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Demonstration {
    /// Unique demonstration ID
    pub id: String,
    /// When the demonstration was recorded
    pub timestamp: DateTime<Utc>,
    /// Name of the application being demonstrated
    pub app_name: String,
    /// Description of the task being demonstrated
    pub task_description: String,
    /// Key moment screenshots (base64 encoded)
    pub screenshots: Vec<String>,
    /// Recorded actions in sequence
    pub actions: Vec<RecordedAction>,
    /// Outcome of the demonstration
    pub outcome: Option<Outcome>,
    /// Duration of the entire demonstration in milliseconds
    pub duration_ms: u64,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Notes from the user
    pub notes: Option<String>,
    /// Window title during recording
    pub window_title: Option<String>,
    /// Screen resolution during recording
    pub screen_resolution: Option<(u32, u32)>,
}

impl Demonstration {
    /// Create a new demonstration
    pub fn new(app_name: impl Into<String>, task_description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            app_name: app_name.into(),
            task_description: task_description.into(),
            screenshots: Vec::new(),
            actions: Vec::new(),
            outcome: None,
            duration_ms: 0,
            tags: Vec::new(),
            notes: None,
            window_title: None,
            screen_resolution: None,
        }
    }

    /// Add an action to the demonstration
    pub fn add_action(&mut self, mut action: RecordedAction) {
        action.sequence = self.actions.len() as u32;
        self.actions.push(action);
    }

    /// Add a screenshot
    pub fn add_screenshot(&mut self, screenshot: String) {
        self.screenshots.push(screenshot);
    }

    /// Set the outcome
    pub fn set_outcome(&mut self, outcome: Outcome) {
        self.outcome = Some(outcome);
    }

    /// Get action count
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Get the action types used in this demonstration
    pub fn action_types(&self) -> Vec<ActionType> {
        let mut types: Vec<ActionType> = self.actions.iter().map(|a| a.action_type).collect();
        types.sort_by_key(|t| format!("{:?}", t));
        types.dedup();
        types
    }
}

// ============================================================================
// Action Template
// ============================================================================

/// A templated action that can be adapted to new contexts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTemplate {
    /// Template ID
    pub id: String,
    /// Action type
    pub action_type: ActionType,
    /// Template for the action details (with placeholders)
    pub details_template: ActionDetails,
    /// Target pattern to match
    pub target_pattern: Option<String>,
    /// Required preconditions
    pub preconditions: Vec<String>,
    /// Expected postconditions
    pub postconditions: Vec<String>,
    /// Variables that can be substituted
    pub variables: Vec<String>,
    /// Sequence order
    pub sequence: u32,
}

impl ActionTemplate {
    /// Create from a recorded action
    pub fn from_action(action: &RecordedAction) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            action_type: action.action_type,
            details_template: action.details.clone(),
            target_pattern: action.target_element.as_ref().map(|e| {
                format!("{}:{}", e.element_type, e.text.as_deref().unwrap_or("*"))
            }),
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            variables: Vec::new(),
            sequence: action.sequence,
        }
    }
}

// ============================================================================
// Skill
// ============================================================================

/// A learned skill that can be applied to similar situations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique skill ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this skill does
    pub description: String,
    /// IDs of demonstrations this skill was learned from
    pub learned_from: Vec<String>,
    /// Trigger patterns - when to suggest this skill
    pub trigger_patterns: Vec<String>,
    /// Action templates for executing this skill
    pub action_template: Vec<ActionTemplate>,
    /// Number of successful applications
    pub success_count: u32,
    /// Number of failed applications
    pub failure_count: u32,
    /// Applications where this skill is relevant
    pub applicable_apps: Vec<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// When this skill was created
    pub created_at: DateTime<Utc>,
    /// When this skill was last used
    pub last_used_at: Option<DateTime<Utc>>,
    /// Confidence score based on success rate
    pub confidence: f32,
    /// Whether this skill is enabled
    pub enabled: bool,
}

impl Skill {
    /// Create a new skill
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            learned_from: Vec::new(),
            trigger_patterns: Vec::new(),
            action_template: Vec::new(),
            success_count: 0,
            failure_count: 0,
            applicable_apps: Vec::new(),
            tags: Vec::new(),
            created_at: Utc::now(),
            last_used_at: None,
            confidence: 0.5,
            enabled: true,
        }
    }

    /// Add a demonstration that this skill was learned from
    pub fn add_learned_from(&mut self, demo_id: impl Into<String>) {
        self.learned_from.push(demo_id.into());
    }

    /// Add a trigger pattern
    pub fn add_trigger_pattern(&mut self, pattern: impl Into<String>) {
        self.trigger_patterns.push(pattern.into());
    }

    /// Add an action template
    pub fn add_action_template(&mut self, template: ActionTemplate) {
        self.action_template.push(template);
    }

    /// Record a successful application
    pub fn record_success(&mut self) {
        self.success_count += 1;
        self.last_used_at = Some(Utc::now());
        self.update_confidence();
    }

    /// Record a failed application
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_used_at = Some(Utc::now());
        self.update_confidence();
    }

    /// Update confidence based on success/failure rate
    fn update_confidence(&mut self) {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            self.confidence = 0.5;
        } else {
            // Bayesian update with prior of 0.5
            self.confidence = (self.success_count as f32 + 1.0) / (total as f32 + 2.0);
        }
    }

    /// Get the success rate
    pub fn success_rate(&self) -> f32 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.5
        } else {
            self.success_count as f32 / total as f32
        }
    }
}

// ============================================================================
// Session
// ============================================================================

/// A recording session that tracks demonstrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session ID
    pub id: String,
    /// When the session started
    pub started_at: DateTime<Utc>,
    /// When the session ended
    pub ended_at: Option<DateTime<Utc>>,
    /// Demonstrations recorded in this session
    pub demonstration_ids: Vec<String>,
    /// Current application being recorded
    pub current_app: Option<String>,
    /// Notes about this session
    pub notes: Option<String>,
}

impl Session {
    /// Create a new session
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            started_at: Utc::now(),
            ended_at: None,
            demonstration_ids: Vec::new(),
            current_app: None,
            notes: None,
        }
    }

    /// End the session
    pub fn end(&mut self) {
        self.ended_at = Some(Utc::now());
    }

    /// Add a demonstration to this session
    pub fn add_demonstration(&mut self, demo_id: impl Into<String>) {
        self.demonstration_ids.push(demo_id.into());
    }

    /// Check if the session is active
    pub fn is_active(&self) -> bool {
        self.ended_at.is_none()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Action (for skill application)
// ============================================================================

/// An action to be executed (output from skill application)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Action type
    pub action_type: ActionType,
    /// Action details
    pub details: ActionDetails,
    /// Description of what this action does
    pub description: String,
    /// Expected result
    pub expected_result: Option<String>,
    /// Delay before executing (ms)
    pub delay_ms: u64,
}

impl Action {
    /// Create a new action
    pub fn new(action_type: ActionType, details: ActionDetails, description: impl Into<String>) -> Self {
        Self {
            action_type,
            details,
            description: description.into(),
            expected_result: None,
            delay_ms: 0,
        }
    }

    /// Set the expected result
    pub fn with_expected_result(mut self, result: impl Into<String>) -> Self {
        self.expected_result = Some(result.into());
        self
    }

    /// Set the delay before execution
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }
}

// ============================================================================
// Database
// ============================================================================

/// Database connection wrapper with persistence methods
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (useful for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Initialize the database schema
    fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        conn.execute_batch(
            r#"
            -- Demonstrations table
            CREATE TABLE IF NOT EXISTS demonstrations (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                app_name TEXT NOT NULL,
                task_description TEXT NOT NULL,
                screenshots TEXT NOT NULL,  -- JSON array of base64 strings
                actions TEXT NOT NULL,      -- JSON array of RecordedAction
                outcome TEXT,
                duration_ms INTEGER NOT NULL,
                tags TEXT NOT NULL,         -- JSON array of strings
                notes TEXT,
                window_title TEXT,
                screen_resolution TEXT,     -- JSON tuple
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- Skills table
            CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                learned_from TEXT NOT NULL,     -- JSON array of demo IDs
                trigger_patterns TEXT NOT NULL, -- JSON array of patterns
                action_template TEXT NOT NULL,  -- JSON array of ActionTemplate
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                applicable_apps TEXT NOT NULL,  -- JSON array of app names
                tags TEXT NOT NULL,             -- JSON array of strings
                created_at TEXT NOT NULL,
                last_used_at TEXT,
                confidence REAL NOT NULL DEFAULT 0.5,
                enabled INTEGER NOT NULL DEFAULT 1
            );

            -- UI Patterns table
            CREATE TABLE IF NOT EXISTS ui_patterns (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                visual_features TEXT NOT NULL,  -- JSON array
                element_types TEXT NOT NULL,    -- JSON array
                keywords TEXT NOT NULL,         -- JSON array
                examples TEXT NOT NULL,         -- JSON array of base64 screenshots
                match_threshold REAL NOT NULL DEFAULT 0.7,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                match_count INTEGER NOT NULL DEFAULT 0
            );

            -- Sessions table
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                demonstration_ids TEXT NOT NULL, -- JSON array
                current_app TEXT,
                notes TEXT
            );

            -- Skill applications log (for tracking success/failure)
            CREATE TABLE IF NOT EXISTS skill_applications (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                success INTEGER NOT NULL,
                context TEXT,               -- JSON with context info
                notes TEXT,
                FOREIGN KEY (skill_id) REFERENCES skills(id)
            );

            -- Indexes for faster queries
            CREATE INDEX IF NOT EXISTS idx_demonstrations_app ON demonstrations(app_name);
            CREATE INDEX IF NOT EXISTS idx_demonstrations_timestamp ON demonstrations(timestamp);
            CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
            CREATE INDEX IF NOT EXISTS idx_skills_confidence ON skills(confidence);
            CREATE INDEX IF NOT EXISTS idx_ui_patterns_name ON ui_patterns(name);
            CREATE INDEX IF NOT EXISTS idx_skill_applications_skill_id ON skill_applications(skill_id);
            "#,
        )?;

        Ok(())
    }

    // ========================================================================
    // Demonstration methods
    // ========================================================================

    /// Save a demonstration
    pub fn save_demonstration(&self, demo: &Demonstration) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let screenshots_json = serde_json::to_string(&demo.screenshots)?;
        let actions_json = serde_json::to_string(&demo.actions)?;
        let tags_json = serde_json::to_string(&demo.tags)?;
        let resolution_json = demo.screen_resolution.map(|r| serde_json::to_string(&r).unwrap_or_default());

        conn.execute(
            r#"
            INSERT OR REPLACE INTO demonstrations
            (id, timestamp, app_name, task_description, screenshots, actions, outcome,
             duration_ms, tags, notes, window_title, screen_resolution)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                demo.id,
                demo.timestamp.to_rfc3339(),
                demo.app_name,
                demo.task_description,
                screenshots_json,
                actions_json,
                demo.outcome.map(|o| o.to_string()),
                demo.duration_ms as i64,
                tags_json,
                demo.notes,
                demo.window_title,
                resolution_json,
            ],
        )?;

        Ok(())
    }

    /// Get a demonstration by ID
    pub fn get_demonstration(&self, id: &str) -> Result<Option<Demonstration>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let result = conn.query_row(
            r#"
            SELECT id, timestamp, app_name, task_description, screenshots, actions,
                   outcome, duration_ms, tags, notes, window_title, screen_resolution
            FROM demonstrations WHERE id = ?1
            "#,
            params![id],
            |row| {
                let id: String = row.get(0)?;
                let timestamp: String = row.get(1)?;
                let app_name: String = row.get(2)?;
                let task_description: String = row.get(3)?;
                let screenshots_json: String = row.get(4)?;
                let actions_json: String = row.get(5)?;
                let outcome: Option<String> = row.get(6)?;
                let duration_ms: i64 = row.get(7)?;
                let tags_json: String = row.get(8)?;
                let notes: Option<String> = row.get(9)?;
                let window_title: Option<String> = row.get(10)?;
                let resolution_json: Option<String> = row.get(11)?;

                Ok((id, timestamp, app_name, task_description, screenshots_json,
                    actions_json, outcome, duration_ms, tags_json, notes,
                    window_title, resolution_json))
            },
        ).optional()?;

        match result {
            Some((id, timestamp, app_name, task_description, screenshots_json,
                  actions_json, outcome, duration_ms, tags_json, notes,
                  window_title, resolution_json)) => {
                let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                    .map_err(|e| Error::invalid(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);

                let screenshots: Vec<String> = serde_json::from_str(&screenshots_json)?;
                let actions: Vec<RecordedAction> = serde_json::from_str(&actions_json)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json)?;

                let outcome = outcome.map(|o| match o.as_str() {
                    "success" => Outcome::Success,
                    "failure" => Outcome::Failure,
                    "partial" => Outcome::Partial,
                    _ => Outcome::Unknown,
                });

                let screen_resolution = resolution_json
                    .and_then(|json| serde_json::from_str(&json).ok());

                Ok(Some(Demonstration {
                    id,
                    timestamp,
                    app_name,
                    task_description,
                    screenshots,
                    actions,
                    outcome,
                    duration_ms: duration_ms as u64,
                    tags,
                    notes,
                    window_title,
                    screen_resolution,
                }))
            }
            None => Ok(None),
        }
    }

    /// List demonstrations, optionally filtered by app
    pub fn list_demonstrations(&self, app_filter: Option<&str>, limit: usize) -> Result<Vec<Demonstration>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let query = match app_filter {
            Some(_) => {
                "SELECT id FROM demonstrations WHERE app_name = ?1 ORDER BY timestamp DESC LIMIT ?2"
            }
            None => {
                "SELECT id FROM demonstrations ORDER BY timestamp DESC LIMIT ?1"
            }
        };

        let ids: Vec<String> = if let Some(app) = app_filter {
            let mut stmt = conn.prepare(query)?;
            let rows = stmt.query_map(params![app, limit as i64], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(query)?;
            let rows = stmt.query_map(params![limit as i64], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        drop(conn);

        let mut demos = Vec::new();
        for id in ids {
            if let Some(demo) = self.get_demonstration(&id)? {
                demos.push(demo);
            }
        }

        Ok(demos)
    }

    /// Delete a demonstration
    pub fn delete_demonstration(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;
        let rows = conn.execute("DELETE FROM demonstrations WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// Search demonstrations by task description
    pub fn search_demonstrations(&self, query: &str, limit: usize) -> Result<Vec<Demonstration>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let search_pattern = format!("%{}%", query.to_lowercase());

        let ids: Vec<String> = {
            let mut stmt = conn.prepare(
                r#"
                SELECT id FROM demonstrations
                WHERE LOWER(task_description) LIKE ?1 OR LOWER(app_name) LIKE ?1
                ORDER BY timestamp DESC LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![search_pattern, limit as i64], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        drop(conn);

        let mut demos = Vec::new();
        for id in ids {
            if let Some(demo) = self.get_demonstration(&id)? {
                demos.push(demo);
            }
        }

        Ok(demos)
    }

    // ========================================================================
    // Skill methods
    // ========================================================================

    /// Save a skill
    pub fn save_skill(&self, skill: &Skill) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let learned_from_json = serde_json::to_string(&skill.learned_from)?;
        let trigger_patterns_json = serde_json::to_string(&skill.trigger_patterns)?;
        let action_template_json = serde_json::to_string(&skill.action_template)?;
        let applicable_apps_json = serde_json::to_string(&skill.applicable_apps)?;
        let tags_json = serde_json::to_string(&skill.tags)?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO skills
            (id, name, description, learned_from, trigger_patterns, action_template,
             success_count, failure_count, applicable_apps, tags, created_at,
             last_used_at, confidence, enabled)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                skill.id,
                skill.name,
                skill.description,
                learned_from_json,
                trigger_patterns_json,
                action_template_json,
                skill.success_count as i64,
                skill.failure_count as i64,
                applicable_apps_json,
                tags_json,
                skill.created_at.to_rfc3339(),
                skill.last_used_at.map(|t| t.to_rfc3339()),
                skill.confidence as f64,
                skill.enabled as i32,
            ],
        )?;

        Ok(())
    }

    /// Get a skill by ID
    pub fn get_skill(&self, id: &str) -> Result<Option<Skill>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let result = conn.query_row(
            r#"
            SELECT id, name, description, learned_from, trigger_patterns, action_template,
                   success_count, failure_count, applicable_apps, tags, created_at,
                   last_used_at, confidence, enabled
            FROM skills WHERE id = ?1
            "#,
            params![id],
            |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let description: String = row.get(2)?;
                let learned_from_json: String = row.get(3)?;
                let trigger_patterns_json: String = row.get(4)?;
                let action_template_json: String = row.get(5)?;
                let success_count: i64 = row.get(6)?;
                let failure_count: i64 = row.get(7)?;
                let applicable_apps_json: String = row.get(8)?;
                let tags_json: String = row.get(9)?;
                let created_at: String = row.get(10)?;
                let last_used_at: Option<String> = row.get(11)?;
                let confidence: f64 = row.get(12)?;
                let enabled: i32 = row.get(13)?;

                Ok((id, name, description, learned_from_json, trigger_patterns_json,
                    action_template_json, success_count, failure_count, applicable_apps_json,
                    tags_json, created_at, last_used_at, confidence, enabled))
            },
        ).optional()?;

        match result {
            Some((id, name, description, learned_from_json, trigger_patterns_json,
                  action_template_json, success_count, failure_count, applicable_apps_json,
                  tags_json, created_at, last_used_at, confidence, enabled)) => {
                let created_at = DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| Error::invalid(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);

                let last_used_at = last_used_at.map(|t| {
                    DateTime::parse_from_rfc3339(&t)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                }).flatten();

                let learned_from: Vec<String> = serde_json::from_str(&learned_from_json)?;
                let trigger_patterns: Vec<String> = serde_json::from_str(&trigger_patterns_json)?;
                let action_template: Vec<ActionTemplate> = serde_json::from_str(&action_template_json)?;
                let applicable_apps: Vec<String> = serde_json::from_str(&applicable_apps_json)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json)?;

                Ok(Some(Skill {
                    id,
                    name,
                    description,
                    learned_from,
                    trigger_patterns,
                    action_template,
                    success_count: success_count as u32,
                    failure_count: failure_count as u32,
                    applicable_apps,
                    tags,
                    created_at,
                    last_used_at,
                    confidence: confidence as f32,
                    enabled: enabled != 0,
                }))
            }
            None => Ok(None),
        }
    }

    /// List all skills
    pub fn list_skills(&self, enabled_only: bool) -> Result<Vec<Skill>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let query = if enabled_only {
            "SELECT id FROM skills WHERE enabled = 1 ORDER BY confidence DESC"
        } else {
            "SELECT id FROM skills ORDER BY confidence DESC"
        };

        let ids: Vec<String> = {
            let mut stmt = conn.prepare(query)?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        drop(conn);

        let mut skills = Vec::new();
        for id in ids {
            if let Some(skill) = self.get_skill(&id)? {
                skills.push(skill);
            }
        }

        Ok(skills)
    }

    /// Delete a skill
    pub fn delete_skill(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;
        let rows = conn.execute("DELETE FROM skills WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// Update skill success/failure counts
    pub fn update_skill_outcome(&self, id: &str, success: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        if success {
            conn.execute(
                "UPDATE skills SET success_count = success_count + 1, last_used_at = ?2 WHERE id = ?1",
                params![id, Utc::now().to_rfc3339()],
            )?;
        } else {
            conn.execute(
                "UPDATE skills SET failure_count = failure_count + 1, last_used_at = ?2 WHERE id = ?1",
                params![id, Utc::now().to_rfc3339()],
            )?;
        }

        // Recalculate confidence
        let (success_count, failure_count): (i64, i64) = conn.query_row(
            "SELECT success_count, failure_count FROM skills WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let total = success_count + failure_count;
        let confidence = if total == 0 {
            0.5
        } else {
            (success_count as f64 + 1.0) / (total as f64 + 2.0)
        };

        conn.execute(
            "UPDATE skills SET confidence = ?2 WHERE id = ?1",
            params![id, confidence],
        )?;

        Ok(())
    }

    // ========================================================================
    // UI Pattern methods
    // ========================================================================

    /// Save a UI pattern
    pub fn save_ui_pattern(&self, pattern: &UiPattern) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let visual_features_json = serde_json::to_string(&pattern.visual_features)?;
        let element_types_json = serde_json::to_string(&pattern.element_types)?;
        let keywords_json = serde_json::to_string(&pattern.keywords)?;
        let examples_json = serde_json::to_string(&pattern.examples)?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO ui_patterns
            (id, name, description, visual_features, element_types, keywords,
             examples, match_threshold, created_at, updated_at, match_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                pattern.id,
                pattern.name,
                pattern.description,
                visual_features_json,
                element_types_json,
                keywords_json,
                examples_json,
                pattern.match_threshold as f64,
                pattern.created_at.to_rfc3339(),
                pattern.updated_at.to_rfc3339(),
                pattern.match_count as i64,
            ],
        )?;

        Ok(())
    }

    /// Get a UI pattern by ID
    pub fn get_ui_pattern(&self, id: &str) -> Result<Option<UiPattern>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let result = conn.query_row(
            r#"
            SELECT id, name, description, visual_features, element_types, keywords,
                   examples, match_threshold, created_at, updated_at, match_count
            FROM ui_patterns WHERE id = ?1
            "#,
            params![id],
            |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let description: String = row.get(2)?;
                let visual_features_json: String = row.get(3)?;
                let element_types_json: String = row.get(4)?;
                let keywords_json: String = row.get(5)?;
                let examples_json: String = row.get(6)?;
                let match_threshold: f64 = row.get(7)?;
                let created_at: String = row.get(8)?;
                let updated_at: String = row.get(9)?;
                let match_count: i64 = row.get(10)?;

                Ok((id, name, description, visual_features_json, element_types_json,
                    keywords_json, examples_json, match_threshold, created_at,
                    updated_at, match_count))
            },
        ).optional()?;

        match result {
            Some((id, name, description, visual_features_json, element_types_json,
                  keywords_json, examples_json, match_threshold, created_at,
                  updated_at, match_count)) => {
                let created_at = DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| Error::invalid(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);
                let updated_at = DateTime::parse_from_rfc3339(&updated_at)
                    .map_err(|e| Error::invalid(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);

                let visual_features: Vec<String> = serde_json::from_str(&visual_features_json)?;
                let element_types: Vec<ElementType> = serde_json::from_str(&element_types_json)?;
                let keywords: Vec<String> = serde_json::from_str(&keywords_json)?;
                let examples: Vec<String> = serde_json::from_str(&examples_json)?;

                Ok(Some(UiPattern {
                    id,
                    name,
                    description,
                    visual_features,
                    element_types,
                    keywords,
                    examples,
                    match_threshold: match_threshold as f32,
                    created_at,
                    updated_at,
                    match_count: match_count as u32,
                }))
            }
            None => Ok(None),
        }
    }

    /// List all UI patterns
    pub fn list_ui_patterns(&self) -> Result<Vec<UiPattern>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let ids: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id FROM ui_patterns ORDER BY match_count DESC")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        drop(conn);

        let mut patterns = Vec::new();
        for id in ids {
            if let Some(pattern) = self.get_ui_pattern(&id)? {
                patterns.push(pattern);
            }
        }

        Ok(patterns)
    }

    // ========================================================================
    // Session methods
    // ========================================================================

    /// Save a session
    pub fn save_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let demo_ids_json = serde_json::to_string(&session.demonstration_ids)?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO sessions
            (id, started_at, ended_at, demonstration_ids, current_app, notes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                session.id,
                session.started_at.to_rfc3339(),
                session.ended_at.map(|t| t.to_rfc3339()),
                demo_ids_json,
                session.current_app,
                session.notes,
            ],
        )?;

        Ok(())
    }

    /// Get a session by ID
    pub fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let result = conn.query_row(
            r#"
            SELECT id, started_at, ended_at, demonstration_ids, current_app, notes
            FROM sessions WHERE id = ?1
            "#,
            params![id],
            |row| {
                let id: String = row.get(0)?;
                let started_at: String = row.get(1)?;
                let ended_at: Option<String> = row.get(2)?;
                let demo_ids_json: String = row.get(3)?;
                let current_app: Option<String> = row.get(4)?;
                let notes: Option<String> = row.get(5)?;

                Ok((id, started_at, ended_at, demo_ids_json, current_app, notes))
            },
        ).optional()?;

        match result {
            Some((id, started_at, ended_at, demo_ids_json, current_app, notes)) => {
                let started_at = DateTime::parse_from_rfc3339(&started_at)
                    .map_err(|e| Error::invalid(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);

                let ended_at = ended_at.map(|t| {
                    DateTime::parse_from_rfc3339(&t)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                }).flatten();

                let demonstration_ids: Vec<String> = serde_json::from_str(&demo_ids_json)?;

                Ok(Some(Session {
                    id,
                    started_at,
                    ended_at,
                    demonstration_ids,
                    current_app,
                    notes,
                }))
            }
            None => Ok(None),
        }
    }

    /// Get the currently active session
    pub fn get_active_session(&self) -> Result<Option<Session>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let id: Option<String> = conn.query_row(
            "SELECT id FROM sessions WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        ).optional()?;

        drop(conn);

        match id {
            Some(id) => self.get_session(&id),
            None => Ok(None),
        }
    }

    // ========================================================================
    // Skill application logging
    // ========================================================================

    /// Log a skill application
    pub fn log_skill_application(
        &self,
        skill_id: &str,
        success: bool,
        context: Option<&str>,
        notes: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        conn.execute(
            r#"
            INSERT INTO skill_applications (id, skill_id, timestamp, success, context, notes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                Uuid::new_v4().to_string(),
                skill_id,
                Utc::now().to_rfc3339(),
                success as i32,
                context,
                notes,
            ],
        )?;

        Ok(())
    }

    /// Get skill application history
    pub fn get_skill_applications(&self, skill_id: &str, limit: usize) -> Result<Vec<(DateTime<Utc>, bool, Option<String>)>> {
        let conn = self.conn.lock().map_err(|_| Error::invalid("Lock poisoned"))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT timestamp, success, notes FROM skill_applications
            WHERE skill_id = ?1 ORDER BY timestamp DESC LIMIT ?2
            "#,
        )?;

        let results = stmt.query_map(params![skill_id, limit as i64], |row| {
            let timestamp: String = row.get(0)?;
            let success: i32 = row.get(1)?;
            let notes: Option<String> = row.get(2)?;
            Ok((timestamp, success != 0, notes))
        })?;

        let mut applications = Vec::new();
        for result in results {
            let (timestamp, success, notes) = result?;
            let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                .map_err(|e| Error::invalid(format!("Invalid timestamp: {}", e)))?
                .with_timezone(&Utc);
            applications.push((timestamp, success, notes));
        }

        Ok(applications)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demonstration_creation() {
        let demo = Demonstration::new("Firefox", "Open settings menu");
        assert_eq!(demo.app_name, "Firefox");
        assert_eq!(demo.task_description, "Open settings menu");
        assert!(demo.actions.is_empty());
    }

    #[test]
    fn test_skill_confidence() {
        let mut skill = Skill::new("test_skill", "A test skill");
        assert_eq!(skill.confidence, 0.5);

        skill.record_success();
        skill.record_success();
        skill.record_failure();

        // (2 + 1) / (3 + 2) = 0.6
        assert!((skill.confidence - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_database_operations() -> Result<()> {
        let db = Database::in_memory()?;

        // Test demonstration
        let demo = Demonstration::new("TestApp", "Test task");
        db.save_demonstration(&demo)?;

        let loaded = db.get_demonstration(&demo.id)?;
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.app_name, "TestApp");

        // Test skill
        let skill = Skill::new("test_skill", "A test skill");
        db.save_skill(&skill)?;

        let loaded = db.get_skill(&skill.id)?;
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.name, "test_skill");

        Ok(())
    }

    #[test]
    fn test_action_details() {
        let details = ActionDetails::MouseClick {
            x: 100,
            y: 200,
            button: MouseButton::Left,
            modifiers: vec![],
        };

        assert_eq!(details.position(), Some((100, 200)));
        assert!(!details.has_text_input());

        let text_details = ActionDetails::TextInput {
            text: "hello".to_string(),
        };
        assert!(text_details.has_text_input());
    }
}
