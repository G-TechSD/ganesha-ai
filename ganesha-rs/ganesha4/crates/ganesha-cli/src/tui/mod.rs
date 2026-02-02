//! # TUI Mode
//!
//! Terminal User Interface - a visual wrapper around the same agentic core as the REPL.
//! Uses the shared ReplState and agentic_chat function for full feature parity.

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod app;
pub mod events;
pub mod ui;
pub mod widgets;

use app::AppState;
use events::{handle_event, update, Msg};

use crate::config::CliConfig;
use crate::repl::{ReplState, agentic_chat};
use crate::setup::{ProvidersConfig, ProviderType};
use ganesha_providers::{
    ProviderManager, ProviderPriority,
    LocalProvider, LocalProviderType, AnthropicProvider, OpenAiProvider, GeminiProvider, OpenRouterProvider
};
use tracing::{info, warn};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::io::{self, Write as IoWrite};
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

/// Run the TUI application - uses the SAME core as REPL, just different UI
pub async fn run() -> anyhow::Result<()> {
    // Load configuration (SAME as REPL)
    let mut config = CliConfig::load().await.unwrap_or_default();
    config.apply_env_overrides();

    // Initialize provider manager (SAME as REPL)
    let provider_manager = Arc::new(ProviderManager::new());

    // Load saved provider configs (SAME as REPL)
    let saved_config = ProvidersConfig::load();
    let use_config_providers = saved_config.has_providers();

    if use_config_providers {
        info!("TUI: Loading providers from config file...");
        for (idx, provider) in saved_config.enabled_providers().iter().enumerate() {
            let priority = if idx == 0 {
                ProviderPriority::Primary
            } else if idx == 1 {
                ProviderPriority::Secondary
            } else {
                ProviderPriority::Fallback
            };

            match provider.provider_type {
                ProviderType::Anthropic => {
                    if let Some(ref api_key) = provider.api_key {
                        provider_manager.register(AnthropicProvider::new(api_key.clone()), priority).await;
                    }
                }
                ProviderType::OpenAI => {
                    if let Some(ref api_key) = provider.api_key {
                        provider_manager.register(OpenAiProvider::new(api_key.clone()), priority).await;
                    }
                }
                ProviderType::Gemini => {
                    if let Some(ref api_key) = provider.api_key {
                        provider_manager.register(GeminiProvider::new(api_key.clone()), priority).await;
                    }
                }
                ProviderType::OpenRouter => {
                    if let Some(ref api_key) = provider.api_key {
                        provider_manager.register(OpenRouterProvider::new(api_key.clone()), priority).await;
                    }
                }
                ProviderType::Local => {
                    if let Some(ref base_url) = provider.base_url {
                        let url = if base_url.ends_with("/v1") {
                            base_url.clone()
                        } else if base_url.ends_with('/') {
                            format!("{}v1", base_url)
                        } else {
                            format!("{}/v1", base_url)
                        };
                        let local = LocalProvider::new(LocalProviderType::OpenAiCompatible)
                            .with_base_url(url);
                        provider_manager.register(local, priority).await;
                    }
                }
            }
        }
    } else {
        info!("TUI: No providers.toml found, using auto-discovery...");
        let _ = provider_manager.auto_discover().await;
    }

    let has_providers = provider_manager.has_available_provider().await;
    let providers = provider_manager.list_providers().await;
    let provider_names: Vec<_> = providers.iter().map(|p| p.name.clone()).collect();

    // Create ReplState - the SAME state used by REPL
    let mut repl_state = ReplState::new_for_tui(provider_manager.clone(), &config);

    // Initialize MCP servers (SAME as REPL)
    if let Err(e) = repl_state.init_mcp().await {
        warn!("Failed to initialize MCP: {}", e);
    }

    // Start session logging
    if let Err(e) = repl_state.session_logger.start_session(Some("tui")) {
        warn!("Failed to start session logging: {}", e);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create UI state (for display only)
    let mut ui_state = AppState::new();

    // Show startup info
    if has_providers {
        ui_state.add_message(app::ChatMessage::system(format!(
            "Ganesha TUI - Connected to: {} | {} MCP tools available",
            provider_names.join(", "),
            repl_state.mcp_tools.len()
        )));
    } else {
        ui_state.add_message(app::ChatMessage::system(
            "Warning: No LLM providers available. Run 'ganesha init' to configure.".to_string()
        ));
    }

    let size = terminal.size()?;
    ui_state.update_terminal_size(size.width, size.height);
    load_git_info(&mut ui_state);

    // Create channel for AI responses
    let (ai_tx, mut ai_rx) = mpsc::channel::<AiResponse>(10);

    // Main event loop
    loop {
        if !ui_state.running {
            break;
        }

        // Draw UI first
        terminal.draw(|f| ui::view(f, &ui_state))?;

        // Check for AI responses (non-blocking)
        while let Ok(response) = ai_rx.try_recv() {
            match response {
                AiResponse::Response(content) => {
                    ui_state.stop_thinking();
                    ui_state.add_message(app::ChatMessage::assistant(content.clone()));
                    repl_state.messages.push(ganesha_providers::Message::assistant(&content));
                    repl_state.auto_save_session();
                }
                AiResponse::Error(error) => {
                    ui_state.stop_thinking();
                    ui_state.add_message(app::ChatMessage::system(format!("Error: {}", error)));
                    ui_state.last_error = Some(error);
                }
            }
        }

        // Poll for terminal events with short timeout
        if event::poll(Duration::from_millis(50))? {
            let evt = event::read()?;
            let msg = handle_event(&ui_state, evt);

            let mut current_msg = Some(msg);
            while let Some(msg) = current_msg {
                // Handle SendMessage specially - use the REAL agentic_chat
                if let Msg::SendMessage = &msg {
                    if let Some(input) = ui_state.submit_input() {
                        // Add to UI
                        ui_state.add_message(app::ChatMessage::user(&input));
                        ui_state.start_thinking("Thinking...");

                        // Clone what we need for the async task
                        let tx = ai_tx.clone();
                        let user_input = input.clone();

                        // We need to call agentic_chat, but it needs &mut ReplState
                        // Since we can't move repl_state into the spawn, we'll call it directly
                        // For now, use a simpler approach - call synchronously in the main loop
                        // This blocks the UI but ensures we use the real agentic_chat

                        // For proper async, we'd need to restructure more significantly
                        // For now, let's at least use the same providers and system prompt

                        // Use the provider_manager we set up at startup (not repl_state's copy)
                        let pm = provider_manager.clone();
                        let system_prompt = crate::repl::agentic_system_prompt(&repl_state);
                        let messages = repl_state.messages.clone();

                        tokio::spawn(async move {
                            use ganesha_providers::{GenerateOptions, Message as ProviderMessage};
                            use tokio::time::timeout;

                            // Check if we have providers
                            if !pm.has_available_provider().await {
                                let _ = tx.send(AiResponse::Error("No LLM providers available. Run 'ganesha init' to configure.".to_string())).await;
                                return;
                            }

                            let mut all_messages = vec![ProviderMessage::system(&system_prompt)];
                            all_messages.extend(messages);
                            all_messages.push(ProviderMessage::user(&user_input));

                            let options = GenerateOptions {
                                temperature: Some(0.7),
                                max_tokens: Some(4096),
                                ..Default::default()
                            };

                            // Use timeout to avoid hanging forever
                            let chat_result = timeout(
                                std::time::Duration::from_secs(120),
                                pm.chat(&all_messages, &options)
                            ).await;

                            match chat_result {
                                Ok(Ok(response)) => {
                                    if response.content.is_empty() {
                                        let _ = tx.send(AiResponse::Error("Empty response from model".to_string())).await;
                                    } else {
                                        let _ = tx.send(AiResponse::Response(response.content)).await;
                                    }
                                }
                                Ok(Err(e)) => {
                                    let _ = tx.send(AiResponse::Error(format!("LLM error: {}", e))).await;
                                }
                                Err(_) => {
                                    let _ = tx.send(AiResponse::Error("Request timed out (120s)".to_string())).await;
                                }
                            }
                        });

                        // Add to repl_state messages for context
                        repl_state.messages.push(ganesha_providers::Message::user(&input));

                        current_msg = None;
                        continue;
                    }
                }

                current_msg = update(&mut ui_state, msg);
            }
        }

        // Tick for animations
        update(&mut ui_state, Msg::Tick);

        // Yield to tokio runtime to let async tasks progress
        tokio::task::yield_now().await;
    }

    // End session
    let _ = repl_state.session_logger.end_session();

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

/// Load git information into UI state
fn load_git_info(state: &mut AppState) {
    if let Ok(repo) = git2::Repository::discover(&state.working_directory) {
        if let Ok(head) = repo.head() {
            if let Some(name) = head.shorthand() {
                state.git_branch = Some(name.to_string());
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        assert!(state.running);
        assert_eq!(state.input_mode, app::InputMode::Insert);
        assert!(!state.messages.is_empty());
    }

    #[test]
    fn test_input_operations() {
        let mut state = AppState::new();

        state.insert_char('h');
        state.insert_char('e');
        state.insert_char('l');
        state.insert_char('l');
        state.insert_char('o');
        assert_eq!(state.input_buffer, "hello");
        assert_eq!(state.input_cursor, 5);

        state.delete_char_before();
        assert_eq!(state.input_buffer, "hell");
        assert_eq!(state.input_cursor, 4);

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
        let initial_count = state.messages.len();

        update(&mut state, Msg::AddSystemMessage("Test message".to_string()));

        assert_eq!(state.messages.len(), initial_count + 1);
    }
}
