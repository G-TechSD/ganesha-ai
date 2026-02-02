//! # Application State
//!
//! Core state management using Elm-style architecture.
//! All UI state is centralized here with immutable updates via messages.

use ganesha_core::RiskLevel;
use ganesha_providers::message::MessageRole;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Maximum number of messages to keep in history
const MAX_MESSAGES: usize = 1000;

/// Input mode - simple like Claude Code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Typing mode - DEFAULT, just type normally
    #[default]
    Insert,
    /// Navigation mode - scroll through history (press Escape to enter, any key to exit)
    Normal,
    /// Command mode - slash commands
    Command,
    /// Visual mode - text selection (unused for now)
    Visual,
}

impl InputMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            InputMode::Normal => "NORMAL",
            InputMode::Insert => "INSERT",
            InputMode::Command => "COMMAND",
            InputMode::Visual => "VISUAL",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            InputMode::Normal => Color::Blue,
            InputMode::Insert => Color::Green,
            InputMode::Command => Color::Yellow,
            InputMode::Visual => Color::Magenta,
        }
    }
}

/// Active panel in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Conversation,
    FileTree,
    Diff,
    ToolOutput,
    Help,
    CommandPalette,
}

impl Panel {
    pub fn title(&self) -> &'static str {
        match self {
            Panel::Conversation => "Conversation",
            Panel::FileTree => "Files",
            Panel::Diff => "Changes",
            Panel::ToolOutput => "Tool Output",
            Panel::Help => "Help",
            Panel::CommandPalette => "Commands",
        }
    }
}

/// A chat message with metadata
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Tool call ID if this is a tool response
    pub tool_call_id: Option<String>,
    /// Whether the message is being streamed
    pub is_streaming: bool,
    /// Tokens used (if available)
    pub tokens: Option<u32>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            timestamp: chrono::Utc::now(),
            tool_call_id: None,
            is_streaming: false,
            tokens: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: chrono::Utc::now(),
            tool_call_id: None,
            is_streaming: false,
            tokens: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            timestamp: chrono::Utc::now(),
            tool_call_id: None,
            is_streaming: false,
            tokens: None,
        }
    }

    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            timestamp: chrono::Utc::now(),
            tool_call_id: Some(tool_call_id.into()),
            is_streaming: false,
            tokens: None,
        }
    }
}

/// File tree entry
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub depth: usize,
    pub is_modified: bool,
    pub is_staged: bool,
}

/// Diff entry for pending changes
#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub path: PathBuf,
    pub status: DiffStatus,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
}

impl DiffStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            DiffStatus::Added => "+",
            DiffStatus::Modified => "~",
            DiffStatus::Deleted => "-",
            DiffStatus::Renamed => "R",
            DiffStatus::Untracked => "?",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            DiffStatus::Added => Color::Green,
            DiffStatus::Modified => Color::Yellow,
            DiffStatus::Deleted => Color::Red,
            DiffStatus::Renamed => Color::Cyan,
            DiffStatus::Untracked => Color::Gray,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
}

/// Tool output entry
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub tool_name: String,
    pub tool_call_id: String,
    pub input: String,
    pub output: String,
    pub success: bool,
    pub duration: Duration,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Command palette entry
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub shortcut: Option<String>,
    pub category: String,
}

/// Spinner state for async operations
#[derive(Debug, Clone)]
pub struct SpinnerState {
    pub message: String,
    pub frame: usize,
    pub started: Instant,
}

impl SpinnerState {
    const FRAMES: &'static [&'static str] = &["", "", "", "", "", "", "", "", "", ""];

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            frame: 0,
            started: Instant::now(),
        }
    }

    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % Self::FRAMES.len();
    }

    pub fn current_frame(&self) -> &'static str {
        Self::FRAMES[self.frame]
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

/// Progress bar state
#[derive(Debug, Clone)]
pub struct ProgressState {
    pub message: String,
    pub current: u64,
    pub total: u64,
}

impl ProgressState {
    pub fn new(message: impl Into<String>, total: u64) -> Self {
        Self {
            message: message.into(),
            current: 0,
            total,
        }
    }

    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        }
    }
}

/// Color theme
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn toggle(&self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

/// Theme colors
#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub bg: ratatui::style::Color,
    pub fg: ratatui::style::Color,
    pub border: ratatui::style::Color,
    pub border_focused: ratatui::style::Color,
    pub accent: ratatui::style::Color,
    pub user_msg: ratatui::style::Color,
    pub assistant_msg: ratatui::style::Color,
    pub system_msg: ratatui::style::Color,
    pub error: ratatui::style::Color,
    pub warning: ratatui::style::Color,
    pub success: ratatui::style::Color,
    pub muted: ratatui::style::Color,
}

impl Theme {
    pub fn colors(&self) -> ThemeColors {
        use ratatui::style::Color;
        match self {
            Theme::Dark => ThemeColors {
                bg: Color::Rgb(30, 30, 46),
                fg: Color::Rgb(205, 214, 244),
                border: Color::Rgb(69, 71, 90),
                border_focused: Color::Rgb(137, 180, 250),
                accent: Color::Rgb(137, 180, 250),
                user_msg: Color::Rgb(116, 199, 236),
                assistant_msg: Color::Rgb(166, 227, 161),
                system_msg: Color::Rgb(249, 226, 175),
                error: Color::Rgb(243, 139, 168),
                warning: Color::Rgb(250, 179, 135),
                success: Color::Rgb(166, 227, 161),
                muted: Color::Rgb(108, 112, 134),
            },
            Theme::Light => ThemeColors {
                bg: Color::Rgb(239, 241, 245),
                fg: Color::Rgb(76, 79, 105),
                border: Color::Rgb(172, 176, 190),
                border_focused: Color::Rgb(30, 102, 245),
                accent: Color::Rgb(30, 102, 245),
                user_msg: Color::Rgb(4, 165, 229),
                assistant_msg: Color::Rgb(64, 160, 43),
                system_msg: Color::Rgb(223, 142, 29),
                error: Color::Rgb(210, 15, 57),
                warning: Color::Rgb(254, 100, 11),
                success: Color::Rgb(64, 160, 43),
                muted: Color::Rgb(140, 143, 161),
            },
        }
    }
}

/// Main application state
#[derive(Debug)]
pub struct AppState {
    // Application lifecycle
    pub running: bool,
    pub needs_redraw: bool,

    // Input state
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub input_history: VecDeque<String>,
    pub input_history_index: Option<usize>,

    // Selection state (for visual mode)
    pub selection_start: Option<usize>,

    // Panel state
    pub active_panel: Panel,
    pub show_side_panel: bool,
    pub side_panel_width: u16,

    // Conversation state
    pub messages: VecDeque<ChatMessage>,
    pub conversation_scroll: usize,
    pub selected_message: Option<usize>,

    // File tree state
    pub file_entries: Vec<FileEntry>,
    pub file_tree_scroll: usize,
    pub selected_file: Option<usize>,
    pub working_directory: PathBuf,

    // Diff state
    pub diff_entries: Vec<DiffEntry>,
    pub diff_scroll: usize,
    pub selected_diff: Option<usize>,

    // Tool output state
    pub tool_outputs: VecDeque<ToolOutput>,
    pub tool_output_scroll: usize,
    pub selected_tool_output: Option<usize>,

    // Command palette state
    pub command_palette_open: bool,
    pub command_palette_input: String,
    pub command_palette_entries: Vec<CommandEntry>,
    pub command_palette_filtered: Vec<usize>,
    pub command_palette_selected: usize,

    // Model state
    pub model_name: String,
    pub risk_level: RiskLevel,
    pub total_tokens: u64,
    pub session_tokens: u64,

    // Git state
    pub git_branch: Option<String>,
    pub git_status: Option<String>,

    // Async operation state
    pub spinner: Option<SpinnerState>,
    pub progress: Option<ProgressState>,
    pub is_ai_thinking: bool,

    // Theme
    pub theme: Theme,

    // Terminal size
    pub terminal_width: u16,
    pub terminal_height: u16,

    // Session info
    pub session_id: String,
    pub session_start: chrono::DateTime<chrono::Utc>,

    // Error state
    pub last_error: Option<String>,
    pub status_message: Option<(String, Instant)>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running: true,
            needs_redraw: true,

            input_mode: InputMode::Insert,  // Start in insert mode for user-friendly UX
            input_buffer: String::new(),
            input_cursor: 0,
            input_history: VecDeque::with_capacity(100),
            input_history_index: None,

            selection_start: None,

            active_panel: Panel::Conversation,
            show_side_panel: true,
            side_panel_width: 30,

            messages: VecDeque::with_capacity(MAX_MESSAGES),
            conversation_scroll: 0,
            selected_message: None,

            file_entries: Vec::new(),
            file_tree_scroll: 0,
            selected_file: None,
            working_directory: std::env::current_dir().unwrap_or_default(),

            diff_entries: Vec::new(),
            diff_scroll: 0,
            selected_diff: None,

            tool_outputs: VecDeque::with_capacity(100),
            tool_output_scroll: 0,
            selected_tool_output: None,

            command_palette_open: false,
            command_palette_input: String::new(),
            command_palette_entries: Self::default_commands(),
            command_palette_filtered: Vec::new(),
            command_palette_selected: 0,

            model_name: "default".to_string(),
            risk_level: RiskLevel::Normal,
            total_tokens: 0,
            session_tokens: 0,

            git_branch: None,
            git_status: None,

            spinner: None,
            progress: None,
            is_ai_thinking: false,

            theme: Theme::Dark,

            terminal_width: 80,
            terminal_height: 24,

            session_id: uuid::Uuid::new_v4().to_string(),
            session_start: chrono::Utc::now(),

            last_error: None,
            status_message: None,
        }
    }
}

impl AppState {
    /// Create a new app state with a welcome message
    pub fn new() -> Self {
        let mut state = Self::default();
        state.messages.push_back(ChatMessage::system(
            "Welcome to Ganesha 4.0! Start typing to chat. Press Esc for navigation mode, /help for commands.",
        ));
        state.filter_command_palette();
        state
    }

    /// Default command palette entries
    fn default_commands() -> Vec<CommandEntry> {
        vec![
            CommandEntry {
                name: "/help".to_string(),
                description: "Show help and keybindings".to_string(),
                shortcut: Some("?".to_string()),
                category: "General".to_string(),
            },
            CommandEntry {
                name: "/clear".to_string(),
                description: "Clear conversation".to_string(),
                shortcut: None,
                category: "Conversation".to_string(),
            },
            CommandEntry {
                name: "/model".to_string(),
                description: "Switch AI model".to_string(),
                shortcut: None,
                category: "Model".to_string(),
            },
            CommandEntry {
                name: "/risk".to_string(),
                description: "Set risk level".to_string(),
                shortcut: None,
                category: "Safety".to_string(),
            },
            CommandEntry {
                name: "/save".to_string(),
                description: "Save conversation".to_string(),
                shortcut: Some("Ctrl+S".to_string()),
                category: "Conversation".to_string(),
            },
            CommandEntry {
                name: "/load".to_string(),
                description: "Load conversation".to_string(),
                shortcut: Some("Ctrl+O".to_string()),
                category: "Conversation".to_string(),
            },
            CommandEntry {
                name: "/theme".to_string(),
                description: "Toggle light/dark theme".to_string(),
                shortcut: Some("Ctrl+T".to_string()),
                category: "Display".to_string(),
            },
            CommandEntry {
                name: "/files".to_string(),
                description: "Toggle file panel".to_string(),
                shortcut: Some("Ctrl+B".to_string()),
                category: "Display".to_string(),
            },
            CommandEntry {
                name: "/diff".to_string(),
                description: "Show pending changes".to_string(),
                shortcut: Some("Ctrl+D".to_string()),
                category: "Git".to_string(),
            },
            CommandEntry {
                name: "/commit".to_string(),
                description: "Commit staged changes".to_string(),
                shortcut: None,
                category: "Git".to_string(),
            },
            CommandEntry {
                name: "/rollback".to_string(),
                description: "Rollback last changes".to_string(),
                shortcut: Some("Ctrl+Z".to_string()),
                category: "Git".to_string(),
            },
            CommandEntry {
                name: "/quit".to_string(),
                description: "Exit Ganesha".to_string(),
                shortcut: Some("Ctrl+Q".to_string()),
                category: "General".to_string(),
            },
        ]
    }

    /// Filter command palette based on current input
    pub fn filter_command_palette(&mut self) {
        let query = self.command_palette_input.to_lowercase();
        self.command_palette_filtered = self
            .command_palette_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if query.is_empty() {
                    true
                } else {
                    entry.name.to_lowercase().contains(&query)
                        || entry.description.to_lowercase().contains(&query)
                        || entry.category.to_lowercase().contains(&query)
                }
            })
            .map(|(i, _)| i)
            .collect();
        self.command_palette_selected = 0;
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push_back(message);
        // Trim old messages if we exceed the limit
        while self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
        // Scroll to bottom
        self.conversation_scroll = self.messages.len().saturating_sub(1);
        self.needs_redraw = true;
    }

    /// Update the last assistant message (for streaming)
    pub fn update_last_assistant_message(&mut self, content: &str) {
        if let Some(msg) = self.messages.back_mut() {
            if msg.role == MessageRole::Assistant {
                msg.content = content.to_string();
                self.needs_redraw = true;
            }
        }
    }

    /// Set status message (auto-clears after a few seconds)
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
        self.needs_redraw = true;
    }

    /// Clear expired status message
    pub fn clear_expired_status(&mut self) {
        if let Some((_, instant)) = &self.status_message {
            if instant.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
                self.needs_redraw = true;
            }
        }
    }

    /// Move cursor left in input buffer
    pub fn move_cursor_left(&mut self) {
        if self.input_cursor > 0 {
            // Find the previous character boundary
            let new_pos = self.input_buffer[..self.input_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_cursor = new_pos;
        }
    }

    /// Move cursor right in input buffer
    pub fn move_cursor_right(&mut self) {
        if self.input_cursor < self.input_buffer.len() {
            // Find the next character boundary
            let new_pos = self.input_buffer[self.input_cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.input_cursor + i)
                .unwrap_or(self.input_buffer.len());
            self.input_cursor = new_pos;
        }
    }

    /// Move cursor to start of input
    pub fn move_cursor_start(&mut self) {
        self.input_cursor = 0;
    }

    /// Move cursor to end of input
    pub fn move_cursor_end(&mut self) {
        self.input_cursor = self.input_buffer.len();
    }

    /// Insert character at cursor position
    pub fn insert_char(&mut self, c: char) {
        self.input_buffer.insert(self.input_cursor, c);
        self.input_cursor += c.len_utf8();
        self.needs_redraw = true;
    }

    /// Delete character before cursor
    pub fn delete_char_before(&mut self) {
        if self.input_cursor > 0 {
            // Find the previous character boundary
            let prev_pos = self.input_buffer[..self.input_cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_buffer.drain(prev_pos..self.input_cursor);
            self.input_cursor = prev_pos;
            self.needs_redraw = true;
        }
    }

    /// Delete character at cursor
    pub fn delete_char_at(&mut self) {
        if self.input_cursor < self.input_buffer.len() {
            // Find the next character boundary
            let next_pos = self.input_buffer[self.input_cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.input_cursor + i)
                .unwrap_or(self.input_buffer.len());
            self.input_buffer.drain(self.input_cursor..next_pos);
            self.needs_redraw = true;
        }
    }

    /// Delete word before cursor
    pub fn delete_word_before(&mut self) {
        if self.input_cursor == 0 {
            return;
        }

        // Find start of previous word
        let before = &self.input_buffer[..self.input_cursor];
        let word_start = before
            .trim_end()
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);

        self.input_buffer.drain(word_start..self.input_cursor);
        self.input_cursor = word_start;
        self.needs_redraw = true;
    }

    /// Clear input buffer
    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.needs_redraw = true;
    }

    /// Submit current input
    pub fn submit_input(&mut self) -> Option<String> {
        if self.input_buffer.is_empty() {
            return None;
        }

        let input = self.input_buffer.clone();

        // Add to history if non-empty and different from last
        if !input.is_empty() {
            if self.input_history.front() != Some(&input) {
                self.input_history.push_front(input.clone());
                if self.input_history.len() > 100 {
                    self.input_history.pop_back();
                }
            }
        }

        self.clear_input();
        self.input_history_index = None;
        Some(input)
    }

    /// Navigate input history up
    pub fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }

        let new_index = match self.input_history_index {
            None => 0,
            Some(i) => (i + 1).min(self.input_history.len() - 1),
        };

        self.input_history_index = Some(new_index);
        if let Some(entry) = self.input_history.get(new_index) {
            self.input_buffer = entry.clone();
            self.input_cursor = self.input_buffer.len();
            self.needs_redraw = true;
        }
    }

    /// Navigate input history down
    pub fn history_next(&mut self) {
        match self.input_history_index {
            None => {}
            Some(0) => {
                self.input_history_index = None;
                self.clear_input();
            }
            Some(i) => {
                let new_index = i - 1;
                self.input_history_index = Some(new_index);
                if let Some(entry) = self.input_history.get(new_index) {
                    self.input_buffer = entry.clone();
                    self.input_cursor = self.input_buffer.len();
                    self.needs_redraw = true;
                }
            }
        }
    }

    /// Scroll conversation up
    pub fn scroll_up(&mut self, amount: usize) {
        self.conversation_scroll = self.conversation_scroll.saturating_sub(amount);
        self.needs_redraw = true;
    }

    /// Scroll conversation down
    pub fn scroll_down(&mut self, amount: usize) {
        let max_scroll = self.messages.len().saturating_sub(1);
        self.conversation_scroll = (self.conversation_scroll + amount).min(max_scroll);
        self.needs_redraw = true;
    }

    /// Scroll to top of conversation
    pub fn scroll_to_top(&mut self) {
        self.conversation_scroll = 0;
        self.needs_redraw = true;
    }

    /// Scroll to bottom of conversation
    pub fn scroll_to_bottom(&mut self) {
        self.conversation_scroll = self.messages.len().saturating_sub(1);
        self.needs_redraw = true;
    }

    /// Switch to next panel
    pub fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Conversation => {
                if self.show_side_panel {
                    Panel::FileTree
                } else {
                    Panel::Conversation
                }
            }
            Panel::FileTree => Panel::Diff,
            Panel::Diff => Panel::ToolOutput,
            Panel::ToolOutput => Panel::Conversation,
            Panel::Help => Panel::Conversation,
            Panel::CommandPalette => Panel::Conversation,
        };
        self.needs_redraw = true;
    }

    /// Switch to previous panel
    pub fn prev_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Conversation => {
                if self.show_side_panel {
                    Panel::ToolOutput
                } else {
                    Panel::Conversation
                }
            }
            Panel::FileTree => Panel::Conversation,
            Panel::Diff => Panel::FileTree,
            Panel::ToolOutput => Panel::Diff,
            Panel::Help => Panel::Conversation,
            Panel::CommandPalette => Panel::Conversation,
        };
        self.needs_redraw = true;
    }

    /// Toggle side panel visibility
    pub fn toggle_side_panel(&mut self) {
        self.show_side_panel = !self.show_side_panel;
        if !self.show_side_panel && self.active_panel != Panel::Conversation {
            self.active_panel = Panel::Conversation;
        }
        self.needs_redraw = true;
    }

    /// Open command palette
    pub fn open_command_palette(&mut self) {
        self.command_palette_open = true;
        self.command_palette_input.clear();
        self.filter_command_palette();
        self.needs_redraw = true;
    }

    /// Close command palette
    pub fn close_command_palette(&mut self) {
        self.command_palette_open = false;
        self.command_palette_input.clear();
        self.needs_redraw = true;
    }

    /// Move command palette selection up
    pub fn command_palette_up(&mut self) {
        if !self.command_palette_filtered.is_empty() {
            self.command_palette_selected = self
                .command_palette_selected
                .saturating_sub(1);
            self.needs_redraw = true;
        }
    }

    /// Move command palette selection down
    pub fn command_palette_down(&mut self) {
        if !self.command_palette_filtered.is_empty() {
            self.command_palette_selected = (self.command_palette_selected + 1)
                .min(self.command_palette_filtered.len().saturating_sub(1));
            self.needs_redraw = true;
        }
    }

    /// Get the currently selected command
    pub fn get_selected_command(&self) -> Option<&CommandEntry> {
        self.command_palette_filtered
            .get(self.command_palette_selected)
            .and_then(|&idx| self.command_palette_entries.get(idx))
    }

    /// Start thinking animation
    pub fn start_thinking(&mut self, message: impl Into<String>) {
        self.is_ai_thinking = true;
        self.spinner = Some(SpinnerState::new(message));
        self.needs_redraw = true;
    }

    /// Stop thinking animation
    pub fn stop_thinking(&mut self) {
        self.is_ai_thinking = false;
        self.spinner = None;
        self.needs_redraw = true;
    }

    /// Tick spinner animation
    pub fn tick_spinner(&mut self) {
        if let Some(ref mut spinner) = self.spinner {
            spinner.tick();
            self.needs_redraw = true;
        }
    }

    /// Update terminal size
    pub fn update_terminal_size(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;

        // Collapse side panel if terminal is too narrow
        if width < 100 {
            self.show_side_panel = false;
        }

        self.needs_redraw = true;
    }

    /// Check if terminal is narrow
    pub fn is_narrow(&self) -> bool {
        self.terminal_width < 80
    }

    /// Check if terminal is very narrow
    pub fn is_very_narrow(&self) -> bool {
        self.terminal_width < 60
    }
}
