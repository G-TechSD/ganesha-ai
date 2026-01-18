//! # Session Command
//!
//! Manage conversation sessions.

use crate::cli::SessionAction;
use crate::render;
use colored::Colorize;
use ganesha_core::session::SessionManager;
use std::io::{self, Write};

/// Get the sessions directory
fn sessions_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .map(|d| d.join("ganesha").join("sessions"))
        .unwrap_or_else(|| std::path::PathBuf::from(".ganesha/sessions"))
}

/// Run the session command
pub async fn run(action: SessionAction) -> anyhow::Result<()> {
    let sessions_dir = sessions_dir();
    let manager = SessionManager::new(&sessions_dir)?;

    match action {
        SessionAction::List => {
            let sessions = manager.list_sessions()?;

            if sessions.is_empty() {
                println!("{}", "No saved sessions found.".dimmed());
                println!();
                println!(
                    "Start a session with {} and save it with {}",
                    "ganesha".bright_green(),
                    "/session save".bright_green()
                );
                return Ok(());
            }

            println!("{}\n", "Saved Sessions".bright_cyan().bold());

            let headers = &["ID", "Name", "Project", "Date", "Messages"];
            let rows: Vec<Vec<String>> = sessions
                .iter()
                .take(20) // Limit to 20 most recent
                .map(|s| {
                    let id_short = if s.id.len() > 8 {
                        format!("{}...", &s.id[..8])
                    } else {
                        s.id.clone()
                    };
                    let name = s.name.clone().unwrap_or_else(|| "-".to_string());
                    let project = s.project_name.clone().unwrap_or_else(|| "-".to_string());
                    let date = s.last_activity.format("%Y-%m-%d %H:%M").to_string();
                    let msg_count = s.message_count.to_string();

                    vec![id_short, name, project, date, msg_count]
                })
                .collect();

            render::print_table(headers, &rows);

            println!();
            println!(
                "Use {} to resume a session",
                "ganesha session resume <id>".bright_green()
            );
            if sessions.len() > 20 {
                println!(
                    "{}",
                    format!("Showing 20 of {} sessions", sessions.len()).dimmed()
                );
            }
        }

        SessionAction::Resume { session } => {
            let spinner = render::Spinner::new(&format!("Loading session {}...", session));

            // Try to find the session by full ID or partial match
            let sessions = manager.list_sessions()?;
            let matching = sessions.iter().find(|s| {
                s.id == session || s.id.starts_with(&session) || s.name.as_deref() == Some(&session)
            });

            match matching {
                Some(summary) => {
                    spinner.finish_with_message(&format!(
                        "{} Found session: {} ({} messages)",
                        "✓".green(),
                        summary.name.as_deref().unwrap_or(&summary.id),
                        summary.message_count
                    ));

                    println!(
                        "\n{} {} {}",
                        "Project:".dimmed(),
                        summary
                            .project_name
                            .as_deref()
                            .unwrap_or("Unknown"),
                        format!("({})", summary.working_directory.display()).dimmed()
                    );
                    println!(
                        "{} {}",
                        "Last active:".dimmed(),
                        summary.last_activity.format("%Y-%m-%d %H:%M UTC")
                    );

                    println!(
                        "\n{} Session context loaded. Start the REPL to continue.",
                        "→".bright_cyan()
                    );
                    println!(
                        "  Run: {} {}",
                        "ganesha --session".bright_green(),
                        summary.id.bright_yellow()
                    );
                }
                None => {
                    spinner.finish_with_message(&format!(
                        "{} Session not found: {}",
                        "✗".red(),
                        session
                    ));
                    println!("\nUse {} to see available sessions", "ganesha session list".bright_green());
                }
            }
        }

        SessionAction::Save { name } => {
            let session_name = name.unwrap_or_else(|| {
                chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string()
            });

            println!("Saving session as: {}", session_name.bright_yellow());

            // Note: This command needs to be called from within an active REPL session
            // For now, show a message about this
            println!(
                "\n{} To save the current session, use {} in the REPL",
                "Note:".yellow(),
                "/session save".bright_green()
            );
            println!("Sessions are auto-saved when you exit the REPL.");
        }

        SessionAction::Delete { session } => {
            // Find the session first
            let sessions = manager.list_sessions()?;
            let matching = sessions.iter().find(|s| {
                s.id == session || s.id.starts_with(&session) || s.name.as_deref() == Some(&session)
            });

            match matching {
                Some(summary) => {
                    println!(
                        "{} Delete session \"{}\"?",
                        "Warning:".yellow().bold(),
                        summary.name.as_deref().unwrap_or(&summary.id).bright_yellow()
                    );
                    println!(
                        "  {} {} messages will be lost",
                        "•".dimmed(),
                        summary.message_count
                    );
                    println!(
                        "  {} Last active: {}",
                        "•".dimmed(),
                        summary.last_activity.format("%Y-%m-%d %H:%M UTC")
                    );
                    print!("\nType 'yes' to confirm: ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    if input.trim().to_lowercase() == "yes" {
                        let mut manager = SessionManager::new(&sessions_dir)?;
                        manager.delete_session(&summary.id)?;
                        println!("{} Session deleted", "✓".green());
                    } else {
                        println!("Cancelled");
                    }
                }
                None => {
                    println!("{} Session not found: {}", "Error:".red(), session);
                }
            }
        }

        SessionAction::Export { session, output } => {
            let sessions = manager.list_sessions()?;
            let matching = sessions.iter().find(|s| {
                s.id == session || s.id.starts_with(&session) || s.name.as_deref() == Some(&session)
            });

            match matching {
                Some(summary) => {
                    let output_path = output.unwrap_or_else(|| {
                        format!(
                            "{}.md",
                            summary.name.as_deref().unwrap_or(&summary.id[..8])
                        )
                    });

                    let spinner =
                        render::Spinner::new(&format!("Exporting session to {}...", output_path));

                    // Load the full session
                    let mut manager = SessionManager::new(&sessions_dir)?;
                    let full_session = manager.load_session(&summary.id)?;

                    // Generate markdown export
                    let mut content = String::new();
                    content.push_str(&format!(
                        "# Session: {}\n\n",
                        full_session.name.as_deref().unwrap_or("Untitled")
                    ));
                    content.push_str(&format!(
                        "**Project:** {}\n",
                        full_session
                            .project_name
                            .as_deref()
                            .unwrap_or("Unknown")
                    ));
                    content.push_str(&format!(
                        "**Date:** {} to {}\n\n",
                        full_session.started_at.format("%Y-%m-%d %H:%M UTC"),
                        full_session.last_activity.format("%Y-%m-%d %H:%M UTC")
                    ));
                    content.push_str("---\n\n");

                    for msg in full_session.messages() {
                        let role_str = match msg.role {
                            ganesha_core::session::MessageRole::User => "**User:**",
                            ganesha_core::session::MessageRole::Assistant => "**Assistant:**",
                            ganesha_core::session::MessageRole::System => "**System:**",
                            ganesha_core::session::MessageRole::Tool => "**Tool:**",
                        };
                        content.push_str(&format!("{}\n\n{}\n\n---\n\n", role_str, msg.content));
                    }

                    std::fs::write(&output_path, content)?;

                    spinner.finish_with_message(&format!(
                        "{} Exported {} messages to {}",
                        "✓".green(),
                        full_session.message_count(),
                        output_path.bright_cyan()
                    ));
                }
                None => {
                    println!("{} Session not found: {}", "Error:".red(), session);
                }
            }
        }
    }

    Ok(())
}
