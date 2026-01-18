//! # Chat Command
//!
//! Send a single message and exit.

use crate::cli::Cli;
use crate::render;

/// Run the chat command
pub async fn run(message: String, model: Option<String>, cli: &Cli) -> anyhow::Result<()> {
    let spinner = render::Spinner::new("Thinking...");

    // TODO: Initialize provider and send message
    // For now, just print a placeholder

    spinner.finish();

    let response = format!(
        "Received message: \"{}\"\n\n\
        Model: {}\n\
        Provider: {}\n\
        Risk Level: {:?}\n\n\
        This is a placeholder response. The actual implementation will \
        send the message to the configured LLM provider.",
        message,
        model.as_deref().unwrap_or("default"),
        cli.provider.as_deref().unwrap_or("auto"),
        cli.risk
    );

    render::print_assistant_message(&response);

    Ok(())
}
