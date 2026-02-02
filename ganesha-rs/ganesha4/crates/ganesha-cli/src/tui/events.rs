//! # Event Handling
//!
//! Simple, intuitive keyboard handling like Claude Code.
//! No vim modes - just type, Enter to send, Escape to cancel.

use super::app::{AppState, InputMode, Panel};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

/// Messages that can be sent to update state
#[derive(Debug, Clone)]
pub enum Msg {
    // Lifecycle
    Quit,
    Tick,
    Resize(u16, u16),

    // Input
    InsertChar(char),
    DeleteCharBefore,
    DeleteCharAt,
    DeleteWordBefore,
    ClearInput,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorStart,
    MoveCursorEnd,

    // Actions
    Submit,
    SendMessage,
    Cancel,

    // History
    HistoryPrev,
    HistoryNext,

    // Scrolling
    ScrollUp(usize),
    ScrollDown(usize),
    ScrollToTop,
    ScrollToBottom,
    PageUp,
    PageDown,

    // Panels
    NextPanel,
    PrevPanel,
    FocusPanel(Panel),
    ToggleSidePanel,

    // Mode (simplified)
    SetMode(InputMode),
    EnterInsertMode,
    EnterNormalMode,
    EnterCommandMode,
    EnterVisualMode,

    // Messages
    AddSystemMessage(String),
    AddUserMessage(String),
    AddAssistantMessage(String),
    ClearConversation,

    // File tree
    RefreshFileTree,
    ToggleFileExpand,
    SelectFile,

    // Help
    ToggleHelp,
    ShowHelp,

    // Command palette
    OpenCommandPalette,
    CloseCommandPalette,
    CommandPaletteUp,
    CommandPaletteDown,
    CommandPaletteSelect,
    CommandPaletteFilter(String),

    // Mouse
    MouseClick(u16, u16),
    MouseScroll(i16),

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

        // Actions
        Msg::Submit => {
            if !state.input_buffer.trim().is_empty() {
                Some(Msg::SendMessage)
            } else {
                None
            }
        }
        Msg::SendMessage => None, // Handled specially in main loop
        Msg::Cancel => {
            state.clear_input();
            state.input_mode = InputMode::Insert;
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
        Msg::ScrollUp(n) => {
            state.scroll_up(n);
            None
        }
        Msg::ScrollDown(n) => {
            state.scroll_down(n);
            None
        }
        Msg::ScrollToTop => {
            match state.active_panel {
                Panel::Conversation => state.conversation_scroll = 0,
                Panel::FileTree => state.file_tree_scroll = 0,
                Panel::Diff => state.diff_scroll = 0,
                _ => {}
            }
            None
        }
        Msg::ScrollToBottom => {
            match state.active_panel {
                Panel::Conversation => {
                    state.conversation_scroll = state.messages.len().saturating_sub(1);
                }
                _ => {}
            }
            None
        }
        Msg::PageUp => {
            let page_size = state.terminal_height.saturating_sub(10) as usize;
            Some(Msg::ScrollUp(page_size.max(1)))
        }
        Msg::PageDown => {
            let page_size = state.terminal_height.saturating_sub(10) as usize;
            Some(Msg::ScrollDown(page_size.max(1)))
        }

        // Panels
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
            state.show_side_panel = !state.show_side_panel;
            None
        }

        // Messages
        Msg::AddSystemMessage(content) => {
            state.add_message(super::app::ChatMessage::system(content));
            None
        }
        Msg::AddUserMessage(content) => {
            state.add_message(super::app::ChatMessage::user(content));
            None
        }
        Msg::AddAssistantMessage(content) => {
            state.add_message(super::app::ChatMessage::assistant(content));
            None
        }
        Msg::ClearConversation => {
            state.messages.clear();
            state.add_message(super::app::ChatMessage::system("Conversation cleared.".to_string()));
            None
        }

        // File tree
        Msg::RefreshFileTree => {
            // TODO: Implement file tree loading
            None
        }
        Msg::ToggleFileExpand => {
            // TODO: Implement file expand
            None
        }
        Msg::SelectFile => {
            // TODO: Open file or add to context
            None
        }

        // Help
        Msg::ToggleHelp => {
            if state.active_panel == Panel::Help {
                state.active_panel = Panel::Conversation;
            } else {
                state.active_panel = Panel::Help;
            }
            None
        }
        Msg::ShowHelp => {
            state.active_panel = Panel::Help;
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
        Msg::CommandPaletteUp => {
            state.command_palette_up();
            None
        }
        Msg::CommandPaletteDown => {
            state.command_palette_down();
            None
        }
        Msg::CommandPaletteSelect => {
            if let Some(cmd) = state.get_selected_command() {
                let cmd_name = cmd.name.clone();
                state.close_command_palette();
                state.set_status(&format!("Executed: {}", cmd_name));
            }
            None
        }
        Msg::CommandPaletteFilter(filter) => {
            state.command_palette_input = filter;
            state.filter_command_palette();
            None
        }

        // Mouse
        Msg::MouseClick(x, y) => {
            // TODO: Handle click positioning
            None
        }
        Msg::MouseScroll(delta) => {
            if delta > 0 {
                Some(Msg::ScrollUp(delta as usize))
            } else {
                Some(Msg::ScrollDown((-delta) as usize))
            }
        }

        Msg::None => None,
    }
}

/// Convert terminal events to messages
pub fn handle_event(state: &AppState, event: Event) -> Msg {
    match event {
        Event::Key(key) => handle_key(state, key),
        Event::Mouse(mouse) => handle_mouse(mouse),
        Event::Resize(width, height) => Msg::Resize(width, height),
        _ => Msg::None,
    }
}

/// Handle mouse events
fn handle_mouse(mouse: MouseEvent) -> Msg {
    match mouse.kind {
        MouseEventKind::ScrollUp => Msg::ScrollUp(3),
        MouseEventKind::ScrollDown => Msg::ScrollDown(3),
        MouseEventKind::Down(_) => Msg::MouseClick(mouse.column, mouse.row),
        _ => Msg::None,
    }
}

/// Handle keyboard events - SIMPLE like Claude Code
fn handle_key(state: &AppState, key: KeyEvent) -> Msg {
    // Ctrl+C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Msg::Quit;
    }

    // Ctrl+L clears conversation
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
        return Msg::ClearConversation;
    }

    // Command palette handling
    if state.command_palette_open {
        return handle_command_palette_key(key);
    }

    // Ctrl+P opens command palette
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('p') {
        return Msg::OpenCommandPalette;
    }

    // Simple keyboard handling - no vim modes
    match key.code {
        // Send message
        KeyCode::Enter => {
            if state.input_buffer.trim().is_empty() {
                Msg::None
            } else {
                Msg::Submit
            }
        }

        // Cancel / clear
        KeyCode::Esc => {
            if !state.input_buffer.is_empty() {
                Msg::ClearInput
            } else {
                Msg::None
            }
        }

        // Typing
        KeyCode::Char(c) => Msg::InsertChar(c),
        KeyCode::Backspace => Msg::DeleteCharBefore,
        KeyCode::Delete => Msg::DeleteCharAt,

        // Cursor movement
        KeyCode::Left => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Msg::MoveCursorStart // Ctrl+Left = start of line
            } else {
                Msg::MoveCursorLeft
            }
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Msg::MoveCursorEnd // Ctrl+Right = end of line
            } else {
                Msg::MoveCursorRight
            }
        }
        KeyCode::Home => Msg::MoveCursorStart,
        KeyCode::End => Msg::MoveCursorEnd,

        // History navigation
        KeyCode::Up => {
            if state.input_buffer.is_empty() {
                Msg::HistoryPrev
            } else {
                Msg::ScrollUp(1) // Scroll conversation if typing
            }
        }
        KeyCode::Down => {
            if state.input_buffer.is_empty() {
                Msg::HistoryNext
            } else {
                Msg::ScrollDown(1)
            }
        }

        // Page scrolling
        KeyCode::PageUp => Msg::PageUp,
        KeyCode::PageDown => Msg::PageDown,

        // Tab for panel switching
        KeyCode::Tab => Msg::NextPanel,
        KeyCode::BackTab => Msg::PrevPanel,

        // F1 for help
        KeyCode::F(1) => Msg::ToggleHelp,

        _ => Msg::None,
    }
}

/// Handle command palette keys
fn handle_command_palette_key(key: KeyEvent) -> Msg {
    match key.code {
        KeyCode::Esc => Msg::CloseCommandPalette,
        KeyCode::Enter => Msg::CommandPaletteSelect,
        KeyCode::Up => Msg::CommandPaletteUp,
        KeyCode::Down => Msg::CommandPaletteDown,
        KeyCode::Char(c) => {
            // Filter as you type
            Msg::InsertChar(c)
        }
        KeyCode::Backspace => Msg::DeleteCharBefore,
        _ => Msg::None,
    }
}

