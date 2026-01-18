//! # TUI Mode
//!
//! Terminal User Interface using ratatui with Elm-style architecture.
//!
//! ## Architecture
//!
//! The TUI follows the Elm architecture pattern:
//! - **Model**: `AppState` holds all UI state
//! - **View**: `ui::view()` renders state to the terminal
//! - **Update**: `events::update()` handles messages and state transitions
//!
//! ## Modules
//!
//! - `app`: Application state and data structures
//! - `events`: Message types and update logic
//! - `ui`: Rendering functions
//! - `widgets`: Reusable custom widgets

// TUI is scaffolded but not fully integrated yet
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod app;
pub mod events;
pub mod ui;
pub mod widgets;

use app::AppState;
use events::{handle_event, update, Msg};

use ganesha_providers::{GenerateOptions, Message as ProviderMessage, ProviderManager};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Result type for TUI operations
pub type Result<T> = std::result::Result<T, TuiError>;

/// TUI-specific errors
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Terminal error: {0}")]
    Terminal(String),
}

/// Message from async AI task back to UI
pub enum AiResponse {
    /// AI responded with content
    Response(String),
    /// AI call failed
    Error(String),
}

/// Run the TUI application
pub async fn run() -> anyhow::Result<()> {
    // Initialize provider manager
    let provider_manager = Arc::new(ProviderManager::new());
    provider_manager.auto_discover().await?;

    // Check if we have providers available
    let has_providers = provider_manager.has_available_provider().await;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut state = AppState::new();

    // Show provider status
    if has_providers {
        let providers = provider_manager.list_providers().await;
        let names: Vec<_> = providers.iter().map(|p| p.name.clone()).collect();
        state.add_message(app::ChatMessage::system(format!(
            "Connected to {} provider(s): {}",
            providers.len(),
            names.join(", ")
        )));
    } else {
        state.add_message(app::ChatMessage::system(
            "Warning: No LLM providers available. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or start a local server.".to_string()
        ));
    }

    // Get initial terminal size
    let size = terminal.size()?;
    state.update_terminal_size(size.width, size.height);

    // Load initial data
    load_git_info(&mut state);

    // Create channel for AI responses
    let (ai_tx, ai_rx) = mpsc::channel::<AiResponse>(10);

    // Main event loop
    let result = run_event_loop(&mut terminal, &mut state, provider_manager, ai_tx, ai_rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Main event loop
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    provider_manager: Arc<ProviderManager>,
    ai_tx: mpsc::Sender<AiResponse>,
    mut ai_rx: mpsc::Receiver<AiResponse>,
) -> anyhow::Result<()> {
    let tick_rate = Duration::from_millis(100);

    while state.running {
        // Draw UI
        if state.needs_redraw {
            terminal.draw(|f| ui::view(f, state))?;
            state.needs_redraw = false;
        }

        // Check for AI responses (non-blocking)
        while let Ok(response) = ai_rx.try_recv() {
            match response {
                AiResponse::Response(content) => {
                    state.stop_thinking();
                    state.add_message(app::ChatMessage::assistant(content));
                }
                AiResponse::Error(error) => {
                    state.stop_thinking();
                    state.add_message(app::ChatMessage::system(format!("Error: {}", error)));
                    state.last_error = Some(error);
                }
            }
        }

        // Poll for events with timeout (for spinner animation)
        if event::poll(tick_rate)? {
            let event = event::read()?;

            // Convert event to message
            let msg = handle_event(state, event);

            // Process message chain (some messages trigger other messages)
            let mut current_msg = Some(msg);
            while let Some(msg) = current_msg {
                // Check if this is a SendMessage that needs AI handling
                if let Msg::SendMessage = &msg {
                    // Get the input and add user message
                    if let Some(input) = state.submit_input() {
                        state.add_message(app::ChatMessage::user(&input));
                        state.start_thinking("Thinking...");

                        // Spawn async task to call AI
                        let pm = provider_manager.clone();
                        let tx = ai_tx.clone();
                        let messages: Vec<ProviderMessage> = state
                            .messages
                            .iter()
                            .map(|m| match m.role {
                                ganesha_providers::message::MessageRole::System => {
                                    ProviderMessage::system(&m.content)
                                }
                                ganesha_providers::message::MessageRole::User => {
                                    ProviderMessage::user(&m.content)
                                }
                                ganesha_providers::message::MessageRole::Assistant => {
                                    ProviderMessage::assistant(&m.content)
                                }
                                ganesha_providers::message::MessageRole::Tool => {
                                    ProviderMessage::tool(&m.content, m.tool_call_id.clone().unwrap_or_default())
                                }
                            })
                            .collect();

                        tokio::spawn(async move {
                            let system_prompt = "You are Ganesha, an AI coding assistant. Be concise and helpful.";
                            let mut all_messages = vec![ProviderMessage::system(system_prompt)];
                            all_messages.extend(messages);

                            let options = GenerateOptions {
                                temperature: Some(0.7),
                                max_tokens: Some(4096),
                                ..Default::default()
                            };

                            match pm.chat(&all_messages, &options).await {
                                Ok(response) => {
                                    let _ = tx.send(AiResponse::Response(response.content)).await;
                                }
                                Err(e) => {
                                    let _ = tx.send(AiResponse::Error(e.to_string())).await;
                                }
                            }
                        });

                        current_msg = None;
                        continue;
                    }
                }

                current_msg = update(state, msg);
            }
        } else {
            // Tick for spinner animation
            let msg = Msg::Tick;
            update(state, msg);
        }
    }

    Ok(())
}

/// Load git information into state
fn load_git_info(state: &mut AppState) {
    // Try to get git branch
    if let Ok(repo) = git2::Repository::discover(&state.working_directory) {
        if let Ok(head) = repo.head() {
            if let Some(name) = head.shorthand() {
                state.git_branch = Some(name.to_string());
            }
        }

        // Get a simple git status summary
        if let Ok(statuses) = repo.statuses(None) {
            let modified = statuses
                .iter()
                .filter(|s| {
                    s.status().contains(git2::Status::WT_MODIFIED)
                        || s.status().contains(git2::Status::INDEX_MODIFIED)
                })
                .count();
            let added = statuses
                .iter()
                .filter(|s| {
                    s.status().contains(git2::Status::WT_NEW)
                        || s.status().contains(git2::Status::INDEX_NEW)
                })
                .count();

            if modified > 0 || added > 0 {
                state.git_status = Some(format!("+{} ~{}", added, modified));
            }
        }
    }
}

/// Load file tree for the current directory
pub fn load_file_tree(state: &mut AppState) {
    use std::fs;

    state.file_entries.clear();

    fn visit_dir(
        entries: &mut Vec<app::FileEntry>,
        path: &std::path::Path,
        depth: usize,
        max_depth: usize,
    ) {
        if depth > max_depth {
            return;
        }

        let mut dir_entries: Vec<_> = match fs::read_dir(path) {
            Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };

        // Sort: directories first, then alphabetically
        dir_entries.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        for entry in dir_entries {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files and common ignore patterns
            if file_name.starts_with('.')
                || file_name == "target"
                || file_name == "node_modules"
                || file_name == "__pycache__"
            {
                continue;
            }

            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            entries.push(app::FileEntry {
                path: entry.path(),
                name: file_name,
                is_dir,
                is_expanded: false,
                depth,
                is_modified: false,
                is_staged: false,
            });

            // Don't auto-expand directories (user will expand manually)
        }
    }

    visit_dir(&mut state.file_entries, &state.working_directory, 0, 1);
}

/// Expand a directory in the file tree
pub fn expand_directory(state: &mut AppState, dir_path: &std::path::Path) {
    use std::fs;

    // Find the directory entry
    let dir_idx = state
        .file_entries
        .iter()
        .position(|e| e.path == dir_path && e.is_dir);

    if let Some(idx) = dir_idx {
        if state.file_entries[idx].is_expanded {
            // Collapse: remove children
            let depth = state.file_entries[idx].depth;
            let mut remove_count = 0;
            for entry in state.file_entries.iter().skip(idx + 1) {
                if entry.depth > depth {
                    remove_count += 1;
                } else {
                    break;
                }
            }
            state.file_entries.drain((idx + 1)..(idx + 1 + remove_count));
            state.file_entries[idx].is_expanded = false;
        } else {
            // Expand: insert children
            let depth = state.file_entries[idx].depth;
            let mut new_entries = Vec::new();

            let mut dir_entries: Vec<_> = match fs::read_dir(dir_path) {
                Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
                Err(_) => return,
            };

            dir_entries.sort_by(|a, b| {
                let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            for entry in dir_entries {
                let file_name = entry.file_name().to_string_lossy().to_string();

                if file_name.starts_with('.')
                    || file_name == "target"
                    || file_name == "node_modules"
                {
                    continue;
                }

                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

                new_entries.push(app::FileEntry {
                    path: entry.path(),
                    name: file_name,
                    is_dir,
                    is_expanded: false,
                    depth: depth + 1,
                    is_modified: false,
                    is_staged: false,
                });
            }

            // Insert after the directory
            let insert_pos = idx + 1;
            for (i, entry) in new_entries.into_iter().enumerate() {
                state.file_entries.insert(insert_pos + i, entry);
            }

            state.file_entries[idx].is_expanded = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        assert!(state.running);
        assert_eq!(state.input_mode, app::InputMode::Normal);
        assert!(!state.messages.is_empty()); // Should have welcome message
    }

    #[test]
    fn test_input_operations() {
        let mut state = AppState::new();

        // Test insert
        state.insert_char('h');
        state.insert_char('e');
        state.insert_char('l');
        state.insert_char('l');
        state.insert_char('o');
        assert_eq!(state.input_buffer, "hello");
        assert_eq!(state.input_cursor, 5);

        // Test delete
        state.delete_char_before();
        assert_eq!(state.input_buffer, "hell");
        assert_eq!(state.input_cursor, 4);

        // Test cursor movement
        state.move_cursor_left();
        assert_eq!(state.input_cursor, 3);
        state.move_cursor_start();
        assert_eq!(state.input_cursor, 0);
        state.move_cursor_end();
        assert_eq!(state.input_cursor, 4);
    }

    #[test]
    fn test_message_handling() {
        let mut state = AppState::new();

        // Initial message count
        let initial_count = state.messages.len();

        // Add a user message
        update(&mut state, Msg::AddSystemMessage("Test message".to_string()));

        assert_eq!(state.messages.len(), initial_count + 1);
    }
}
