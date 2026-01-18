//! # Event Handling
//!
//! Elm-style message passing for UI updates.
//! All user interactions and async events are converted to messages.

use super::app::{AppState, ChatMessage, InputMode, Panel};
use ganesha_core::RiskLevel;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::path::PathBuf;

/// All possible UI messages/events
#[derive(Debug, Clone)]
pub enum Msg {
    // Lifecycle
    Quit,
    Tick,
    Resize(u16, u16),

    // Input mode
    SetMode(InputMode),
    EnterInsertMode,
    EnterNormalMode,
    EnterCommandMode,
    EnterVisualMode,

    // Input buffer
    InsertChar(char),
    DeleteCharBefore,
    DeleteCharAt,
    DeleteWordBefore,
    ClearInput,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorStart,
    MoveCursorEnd,
    Submit,

    // History navigation
    HistoryPrev,
    HistoryNext,

    // Scrolling
    ScrollUp(usize),
    ScrollDown(usize),
    ScrollToTop,
    ScrollToBottom,
    PageUp,
    PageDown,

    // Panel navigation
    NextPanel,
    PrevPanel,
    FocusPanel(Panel),
    ToggleSidePanel,

    // Command palette
    OpenCommandPalette,
    CloseCommandPalette,
    CommandPaletteInput(char),
    CommandPaletteBackspace,
    CommandPaletteUp,
    CommandPaletteDown,
    CommandPaletteSelect,

    // Help
    ShowHelp,
    HideHelp,
    ToggleHelp,

    // Conversation
    AddUserMessage(String),
    AddAssistantMessage(String),
    AddSystemMessage(String),
    AddToolOutput { tool_name: String, output: String },
    UpdateStreamingMessage(String),
    ClearConversation,

    // AI operations
    StartThinking(String),
    StopThinking,
    SendMessage,

    // File tree
    RefreshFileTree,
    ExpandDirectory(PathBuf),
    CollapseDirectory(PathBuf),
    SelectFile(usize),
    FileTreeUp,
    FileTreeDown,

    // Diff
    RefreshDiff,
    SelectDiff(usize),
    DiffUp,
    DiffDown,
    ToggleDiffExpand,

    // Model/Settings
    SetModel(String),
    SetRiskLevel(RiskLevel),
    ToggleTheme,

    // Commands
    ExecuteCommand(String),

    // Errors
    ShowError(String),
    ClearError,

    // Status
    SetStatus(String),
    ClearStatus,

    // No-op
    None,
}

/// Update function - handles all state transitions
pub fn update(state: &mut AppState, msg: Msg) -> Option<Msg> {
    state.needs_redraw = true;

    match msg {
        // Lifecycle
        Msg::Quit => {
            state.running = false;
            None
        }
        Msg::Tick => {
            state.tick_spinner();
            state.clear_expired_status();
            state.needs_redraw = state.spinner.is_some() || state.status_message.is_some();
            None
        }
        Msg::Resize(width, height) => {
            state.update_terminal_size(width, height);
            None
        }

        // Input mode
        Msg::SetMode(mode) => {
            state.input_mode = mode;
            None
        }
        Msg::EnterInsertMode => {
            state.input_mode = InputMode::Insert;
            None
        }
        Msg::EnterNormalMode => {
            state.input_mode = InputMode::Normal;
            state.selection_start = None;
            if state.command_palette_open {
                state.close_command_palette();
            }
            None
        }
        Msg::EnterCommandMode => {
            state.input_mode = InputMode::Command;
            state.clear_input();
            state.insert_char('/');
            None
        }
        Msg::EnterVisualMode => {
            state.input_mode = InputMode::Visual;
            state.selection_start = Some(state.input_cursor);
            None
        }

        // Input buffer
        Msg::InsertChar(c) => {
            state.insert_char(c);
            None
        }
        Msg::DeleteCharBefore => {
            state.delete_char_before();
            None
        }
        Msg::DeleteCharAt => {
            state.delete_char_at();
            None
        }
        Msg::DeleteWordBefore => {
            state.delete_word_before();
            None
        }
        Msg::ClearInput => {
            state.clear_input();
            None
        }
        Msg::MoveCursorLeft => {
            state.move_cursor_left();
            None
        }
        Msg::MoveCursorRight => {
            state.move_cursor_right();
            None
        }
        Msg::MoveCursorStart => {
            state.move_cursor_start();
            None
        }
        Msg::MoveCursorEnd => {
            state.move_cursor_end();
            None
        }
        Msg::Submit => {
            if let Some(input) = state.submit_input() {
                if input.starts_with('/') {
                    return Some(Msg::ExecuteCommand(input));
                } else {
                    return Some(Msg::AddUserMessage(input));
                }
            }
            None
        }

        // History
        Msg::HistoryPrev => {
            state.history_prev();
            None
        }
        Msg::HistoryNext => {
            state.history_next();
            None
        }

        // Scrolling
        Msg::ScrollUp(amount) => {
            state.scroll_up(amount);
            None
        }
        Msg::ScrollDown(amount) => {
            state.scroll_down(amount);
            None
        }
        Msg::ScrollToTop => {
            state.scroll_to_top();
            None
        }
        Msg::ScrollToBottom => {
            state.scroll_to_bottom();
            None
        }
        Msg::PageUp => {
            let amount = (state.terminal_height / 2) as usize;
            state.scroll_up(amount);
            None
        }
        Msg::PageDown => {
            let amount = (state.terminal_height / 2) as usize;
            state.scroll_down(amount);
            None
        }

        // Panel navigation
        Msg::NextPanel => {
            state.next_panel();
            None
        }
        Msg::PrevPanel => {
            state.prev_panel();
            None
        }
        Msg::FocusPanel(panel) => {
            state.active_panel = panel;
            None
        }
        Msg::ToggleSidePanel => {
            state.toggle_side_panel();
            None
        }

        // Command palette
        Msg::OpenCommandPalette => {
            state.open_command_palette();
            None
        }
        Msg::CloseCommandPalette => {
            state.close_command_palette();
            None
        }
        Msg::CommandPaletteInput(c) => {
            state.command_palette_input.push(c);
            state.filter_command_palette();
            None
        }
        Msg::CommandPaletteBackspace => {
            state.command_palette_input.pop();
            state.filter_command_palette();
            None
        }
        Msg::CommandPaletteUp => {
            if state.command_palette_selected > 0 {
                state.command_palette_selected -= 1;
            }
            None
        }
        Msg::CommandPaletteDown => {
            let max = state.command_palette_filtered.len().saturating_sub(1);
            state.command_palette_selected = (state.command_palette_selected + 1).min(max);
            None
        }
        Msg::CommandPaletteSelect => {
            if let Some(&entry_idx) = state.command_palette_filtered.get(state.command_palette_selected) {
                if let Some(entry) = state.command_palette_entries.get(entry_idx) {
                    let cmd = entry.name.clone();
                    state.close_command_palette();
                    return Some(Msg::ExecuteCommand(cmd));
                }
            }
            state.close_command_palette();
            None
        }

        // Help
        Msg::ShowHelp => {
            state.active_panel = Panel::Help;
            None
        }
        Msg::HideHelp => {
            if state.active_panel == Panel::Help {
                state.active_panel = Panel::Conversation;
            }
            None
        }
        Msg::ToggleHelp => {
            if state.active_panel == Panel::Help {
                state.active_panel = Panel::Conversation;
            } else {
                state.active_panel = Panel::Help;
            }
            None
        }

        // Conversation
        Msg::AddUserMessage(content) => {
            state.add_message(ChatMessage::user(content));
            Some(Msg::SendMessage)
        }
        Msg::AddAssistantMessage(content) => {
            state.add_message(ChatMessage::assistant(content));
            state.stop_thinking();
            None
        }
        Msg::AddSystemMessage(content) => {
            state.add_message(ChatMessage::system(content));
            None
        }
        Msg::AddToolOutput { tool_name, output } => {
            let content = format!("[{}]\n{}", tool_name, output);
            state.add_message(ChatMessage::tool(content, tool_name));
            None
        }
        Msg::UpdateStreamingMessage(content) => {
            state.update_last_assistant_message(&content);
            None
        }
        Msg::ClearConversation => {
            state.messages.clear();
            state.add_message(ChatMessage::system("Conversation cleared."));
            state.conversation_scroll = 0;
            None
        }

        // AI operations
        Msg::StartThinking(message) => {
            state.start_thinking(message);
            None
        }
        Msg::StopThinking => {
            state.stop_thinking();
            None
        }
        Msg::SendMessage => {
            state.start_thinking("Thinking...");
            // TODO: Integrate with actual AI provider
            // For now, simulate a response
            None
        }

        // File tree
        Msg::RefreshFileTree => {
            // TODO: Implement file tree refresh
            None
        }
        Msg::ExpandDirectory(_path) => {
            // TODO: Implement directory expansion
            None
        }
        Msg::CollapseDirectory(_path) => {
            // TODO: Implement directory collapse
            None
        }
        Msg::SelectFile(idx) => {
            state.selected_file = Some(idx);
            None
        }
        Msg::FileTreeUp => {
            if let Some(idx) = state.selected_file {
                state.selected_file = Some(idx.saturating_sub(1));
            } else if !state.file_entries.is_empty() {
                state.selected_file = Some(0);
            }
            None
        }
        Msg::FileTreeDown => {
            if let Some(idx) = state.selected_file {
                let max = state.file_entries.len().saturating_sub(1);
                state.selected_file = Some((idx + 1).min(max));
            } else if !state.file_entries.is_empty() {
                state.selected_file = Some(0);
            }
            None
        }

        // Diff
        Msg::RefreshDiff => {
            // TODO: Implement diff refresh
            None
        }
        Msg::SelectDiff(idx) => {
            state.selected_diff = Some(idx);
            None
        }
        Msg::DiffUp => {
            if let Some(idx) = state.selected_diff {
                state.selected_diff = Some(idx.saturating_sub(1));
            } else if !state.diff_entries.is_empty() {
                state.selected_diff = Some(0);
            }
            None
        }
        Msg::DiffDown => {
            if let Some(idx) = state.selected_diff {
                let max = state.diff_entries.len().saturating_sub(1);
                state.selected_diff = Some((idx + 1).min(max));
            } else if !state.diff_entries.is_empty() {
                state.selected_diff = Some(0);
            }
            None
        }
        Msg::ToggleDiffExpand => {
            // TODO: Implement diff expansion toggle
            None
        }

        // Model/Settings
        Msg::SetModel(model) => {
            state.model_name = model;
            state.set_status(format!("Model changed to {}", state.model_name));
            None
        }
        Msg::SetRiskLevel(level) => {
            state.risk_level = level;
            state.set_status(format!("Risk level set to {}", state.risk_level));
            None
        }
        Msg::ToggleTheme => {
            state.theme = state.theme.toggle();
            state.set_status(format!("Theme: {:?}", state.theme));
            None
        }

        // Commands
        Msg::ExecuteCommand(cmd) => {
            execute_command(state, &cmd)
        }

        // Errors
        Msg::ShowError(error) => {
            state.last_error = Some(error.clone());
            state.add_message(ChatMessage::system(format!("Error: {}", error)));
            None
        }
        Msg::ClearError => {
            state.last_error = None;
            None
        }

        // Status
        Msg::SetStatus(message) => {
            state.set_status(message);
            None
        }
        Msg::ClearStatus => {
            state.status_message = None;
            None
        }

        Msg::None => {
            state.needs_redraw = false;
            None
        }
    }
}

/// Execute a slash command
fn execute_command(state: &mut AppState, cmd: &str) -> Option<Msg> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let command = parts.first().map(|s| *s).unwrap_or("");
    let args = &parts[1..];

    match command {
        "/help" | "/h" | "/?" => Some(Msg::ToggleHelp),
        "/quit" | "/q" | "/exit" => Some(Msg::Quit),
        "/clear" | "/c" => Some(Msg::ClearConversation),
        "/theme" | "/t" => Some(Msg::ToggleTheme),
        "/files" | "/f" => Some(Msg::ToggleSidePanel),
        "/model" | "/m" => {
            if let Some(model) = args.first() {
                Some(Msg::SetModel(model.to_string()))
            } else {
                Some(Msg::AddSystemMessage(format!(
                    "Current model: {}. Use /model <name> to switch.",
                    state.model_name
                )))
            }
        }
        "/risk" | "/r" => {
            if let Some(level_str) = args.first() {
                match level_str.parse::<RiskLevel>() {
                    Ok(level) => Some(Msg::SetRiskLevel(level)),
                    Err(_) => Some(Msg::ShowError(format!(
                        "Unknown risk level: {}. Options: safe, normal, trusted, yolo",
                        level_str
                    ))),
                }
            } else {
                Some(Msg::AddSystemMessage(format!(
                    "Current risk level: {} - {}. Use /risk <level> to change.",
                    state.risk_level,
                    state.risk_level.description()
                )))
            }
        }
        "/diff" | "/d" => {
            state.active_panel = Panel::Diff;
            Some(Msg::RefreshDiff)
        }
        "/save" => {
            Some(Msg::SetStatus("Conversation saved.".to_string()))
        }
        "/load" => {
            Some(Msg::SetStatus("Loading conversation...".to_string()))
        }
        "/commit" => {
            Some(Msg::AddSystemMessage("Use git commit for now. Full integration coming soon!".to_string()))
        }
        "/rollback" => {
            Some(Msg::AddSystemMessage("Rollback feature coming soon!".to_string()))
        }
        _ => {
            Some(Msg::ShowError(format!(
                "Unknown command: {}. Type /help for available commands.",
                command
            )))
        }
    }
}

/// Convert crossterm event to message
pub fn handle_event(state: &AppState, event: Event) -> Msg {
    match event {
        Event::Key(key) => handle_key_event(state, key),
        Event::Mouse(mouse) => handle_mouse_event(state, mouse),
        Event::Resize(width, height) => Msg::Resize(width, height),
        Event::FocusGained | Event::FocusLost | Event::Paste(_) => Msg::None,
    }
}

/// Handle keyboard events
fn handle_key_event(state: &AppState, key: KeyEvent) -> Msg {
    // Global keybindings (work in any mode)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => return Msg::Quit,
            KeyCode::Char('q') => return Msg::Quit,
            KeyCode::Char('p') => return Msg::OpenCommandPalette,
            KeyCode::Char('b') => return Msg::ToggleSidePanel,
            KeyCode::Char('t') => return Msg::ToggleTheme,
            KeyCode::Char('l') => return Msg::ClearConversation,
            KeyCode::Char('w') => return Msg::DeleteWordBefore,
            KeyCode::Char('u') => return Msg::ClearInput,
            KeyCode::Char('a') => return Msg::MoveCursorStart,
            KeyCode::Char('e') => return Msg::MoveCursorEnd,
            _ => {}
        }
    }

    // Command palette mode
    if state.command_palette_open {
        return handle_command_palette_key(key);
    }

    // Mode-specific handling
    match state.input_mode {
        InputMode::Normal => handle_normal_mode_key(state, key),
        InputMode::Insert => handle_insert_mode_key(key),
        InputMode::Command => handle_command_mode_key(key),
        InputMode::Visual => handle_visual_mode_key(state, key),
    }
}

/// Handle keys in normal mode
fn handle_normal_mode_key(state: &AppState, key: KeyEvent) -> Msg {
    match key.code {
        // Mode switching
        KeyCode::Char('i') => Msg::EnterInsertMode,
        KeyCode::Char('a') => {
            // Append mode - move cursor right and enter insert
            Msg::EnterInsertMode
        }
        KeyCode::Char('A') => {
            // Append at end of line
            Msg::EnterInsertMode
        }
        KeyCode::Char('I') => {
            // Insert at beginning of line
            Msg::EnterInsertMode
        }
        KeyCode::Char('/') | KeyCode::Char(':') => Msg::EnterCommandMode,
        KeyCode::Char('v') => Msg::EnterVisualMode,

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => Msg::ScrollDown(1),
        KeyCode::Char('k') | KeyCode::Up => Msg::ScrollUp(1),
        KeyCode::Char('g') => Msg::ScrollToTop,
        KeyCode::Char('G') => Msg::ScrollToBottom,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Msg::PageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Msg::PageUp,
        KeyCode::PageUp => Msg::PageUp,
        KeyCode::PageDown => Msg::PageDown,
        KeyCode::Home => Msg::ScrollToTop,
        KeyCode::End => Msg::ScrollToBottom,

        // Panel navigation
        KeyCode::Tab => Msg::NextPanel,
        KeyCode::BackTab => Msg::PrevPanel,
        KeyCode::Char('1') => Msg::FocusPanel(Panel::Conversation),
        KeyCode::Char('2') => Msg::FocusPanel(Panel::FileTree),
        KeyCode::Char('3') => Msg::FocusPanel(Panel::Diff),
        KeyCode::Char('4') => Msg::FocusPanel(Panel::ToolOutput),

        // Help
        KeyCode::Char('?') => Msg::ToggleHelp,
        KeyCode::F(1) => Msg::ShowHelp,

        // Enter insert mode and send
        KeyCode::Enter => {
            if state.input_buffer.is_empty() {
                Msg::EnterInsertMode
            } else {
                Msg::Submit
            }
        }

        // Escape does nothing in normal mode
        KeyCode::Esc => Msg::None,

        // Other
        KeyCode::Char('q') => Msg::Quit,
        KeyCode::Char('r') => Msg::RefreshFileTree,

        _ => Msg::None,
    }
}

/// Handle keys in insert mode
fn handle_insert_mode_key(key: KeyEvent) -> Msg {
    match key.code {
        KeyCode::Esc => Msg::EnterNormalMode,
        KeyCode::Enter => Msg::Submit,
        KeyCode::Backspace => Msg::DeleteCharBefore,
        KeyCode::Delete => Msg::DeleteCharAt,
        KeyCode::Left => Msg::MoveCursorLeft,
        KeyCode::Right => Msg::MoveCursorRight,
        KeyCode::Home => Msg::MoveCursorStart,
        KeyCode::End => Msg::MoveCursorEnd,
        KeyCode::Up => Msg::HistoryPrev,
        KeyCode::Down => Msg::HistoryNext,
        KeyCode::Char(c) => {
            // Check if starting a command
            if c == '/' && key.modifiers.is_empty() {
                // Let it through as a character, user might be typing a path
                Msg::InsertChar(c)
            } else {
                Msg::InsertChar(c)
            }
        }
        KeyCode::Tab => Msg::InsertChar('\t'),
        _ => Msg::None,
    }
}

/// Handle keys in command mode
fn handle_command_mode_key(key: KeyEvent) -> Msg {
    match key.code {
        KeyCode::Esc => Msg::EnterNormalMode,
        KeyCode::Enter => {
            Msg::Submit
        }
        KeyCode::Backspace => {
            Msg::DeleteCharBefore
        }
        KeyCode::Delete => Msg::DeleteCharAt,
        KeyCode::Left => Msg::MoveCursorLeft,
        KeyCode::Right => Msg::MoveCursorRight,
        KeyCode::Home => Msg::MoveCursorStart,
        KeyCode::End => Msg::MoveCursorEnd,
        KeyCode::Up => Msg::HistoryPrev,
        KeyCode::Down => Msg::HistoryNext,
        KeyCode::Char(c) => Msg::InsertChar(c),
        KeyCode::Tab => {
            // TODO: Tab completion for commands
            Msg::None
        }
        _ => Msg::None,
    }
}

/// Handle keys in visual mode
fn handle_visual_mode_key(_state: &AppState, key: KeyEvent) -> Msg {
    match key.code {
        KeyCode::Esc => Msg::EnterNormalMode,
        KeyCode::Char('y') => {
            // TODO: Yank selection
            Msg::EnterNormalMode
        }
        KeyCode::Char('d') | KeyCode::Char('x') => {
            // TODO: Delete selection
            Msg::EnterNormalMode
        }
        // Movement extends selection
        KeyCode::Char('h') | KeyCode::Left => Msg::MoveCursorLeft,
        KeyCode::Char('l') | KeyCode::Right => Msg::MoveCursorRight,
        KeyCode::Char('0') | KeyCode::Home => Msg::MoveCursorStart,
        KeyCode::Char('$') | KeyCode::End => Msg::MoveCursorEnd,
        _ => Msg::None,
    }
}

/// Handle keys in command palette
fn handle_command_palette_key(key: KeyEvent) -> Msg {
    match key.code {
        KeyCode::Esc => Msg::CloseCommandPalette,
        KeyCode::Enter => Msg::CommandPaletteSelect,
        KeyCode::Up => Msg::CommandPaletteUp,
        KeyCode::Down => Msg::CommandPaletteDown,
        KeyCode::Backspace => Msg::CommandPaletteBackspace,
        KeyCode::Char(c) => Msg::CommandPaletteInput(c),
        _ => Msg::None,
    }
}

/// Handle mouse events
fn handle_mouse_event(_state: &AppState, mouse: MouseEvent) -> Msg {
    match mouse.kind {
        MouseEventKind::ScrollUp => Msg::ScrollUp(3),
        MouseEventKind::ScrollDown => Msg::ScrollDown(3),
        MouseEventKind::Down(_) => {
            // TODO: Handle click to focus panels, select items, etc.
            Msg::None
        }
        _ => Msg::None,
    }
}
