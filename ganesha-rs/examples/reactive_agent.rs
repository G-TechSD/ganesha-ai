//! Reactive Agent Example
//!
//! Demonstrates vision-in-the-loop computer use with continuous
//! screenshot polling and situational awareness.
//!
//! Run with: cargo run --example reactive_agent --features computer-use

use ganesha::agent::{AgentAction, AgentConfig, ReactiveAgent, WaitCondition};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA REACTIVE AGENT                              ║");
    println!("║           Vision-in-the-loop computer use                     ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Configure for dual-model setup
    // Adjust endpoints for your setup
    let config = AgentConfig {
        screenshot_interval_ms: 300,  // ~3 FPS polling
        vision_endpoint: "http://192.168.27.182:1234/v1/chat/completions".into(),
        vision_model: "ministral-3b-2501".into(),
        planner_endpoint: "http://192.168.27.42:1234/v1/chat/completions".into(),
        planner_model: "gpt-oss-20b".into(),
        max_actions: 50,
        wait_timeout_ms: 15000,
    };

    println!("[Config]");
    println!("  Screenshot interval: {}ms", config.screenshot_interval_ms);
    println!("  Vision: {} @ {}", config.vision_model, config.vision_endpoint);
    println!("  Planner: {} @ {}", config.planner_model, config.planner_endpoint);
    println!();

    let mut agent = ReactiveAgent::new(config);

    println!("[*] Starting agent (enabling vision polling)...");
    agent.start()?;
    println!("[✓] Agent running");
    println!();

    // Give polling a moment to start
    sleep(Duration::from_secs(1)).await;

    // Show initial state
    println!("═══════════════════════════════════════════════════════════════");
    println!("TASK: Close Firefox, Open Writer, Write about cats");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    if let Some(state) = agent.get_state().await {
        println!("[Initial State] {}", state.description);
        println!("[Screenshots taken] {}", agent.screenshot_count());
    }
    println!();

    // Step 1: Close Firefox (click X button)
    println!("[Step 1] Closing Firefox...");
    agent.execute(AgentAction::Click { x: 1900, y: 14 }).await?;

    // Wait until screen changes (Firefox closes)
    agent.execute(AgentAction::Wait {
        condition: WaitCondition::ScreenChanged
    }).await?;

    if let Some(state) = agent.get_state().await {
        println!("[After close] {} (screenshots: {})", state.description, agent.screenshot_count());
    }
    println!();

    // Step 2: Open Activities
    println!("[Step 2] Opening Activities...");
    agent.execute(AgentAction::Click { x: 50, y: 14 }).await?;

    // Wait for Activities to appear
    agent.execute(AgentAction::Wait {
        condition: WaitCondition::ScreenStable { duration_ms: 300 }
    }).await?;

    if let Some(state) = agent.get_state().await {
        println!("[Activities] {} (screenshots: {})", state.description, agent.screenshot_count());
    }
    println!();

    // Step 3: Search for Writer
    println!("[Step 3] Searching for 'writer'...");
    agent.execute(AgentAction::Type { text: "writer".into() }).await?;

    // Wait for search results
    agent.execute(AgentAction::Wait {
        condition: WaitCondition::ScreenStable { duration_ms: 500 }
    }).await?;

    if let Some(state) = agent.get_state().await {
        println!("[Search results] {} (screenshots: {})", state.description, agent.screenshot_count());
    }
    println!();

    // Step 4: Launch Writer
    println!("[Step 4] Launching Writer...");
    agent.execute(AgentAction::KeyPress { key: "Return".into() }).await?;

    // Wait for Writer to open (look for "Writer" or "LibreOffice" in state)
    println!("[*] Waiting for Writer to open...");
    agent.execute(AgentAction::Wait {
        condition: WaitCondition::TextVisible("writer".into())
    }).await?;

    // Extra stability wait
    agent.execute(AgentAction::Wait {
        condition: WaitCondition::ScreenStable { duration_ms: 500 }
    }).await?;

    if let Some(state) = agent.get_state().await {
        println!("[Writer opened] {} (screenshots: {})", state.description, agent.screenshot_count());
    }
    println!();

    // Step 5: Type the document
    println!("[Step 5] Writing document about cats...");

    let paragraphs = [
        "The Wonderful World of Cats\n\n",
        "Cats have been companions to humans for thousands of years. These remarkable creatures have captured our hearts with their independent spirits and mysterious personalities.\n\n",
        "The domestic cat, Felis catus, is a small carnivorous mammal that evolved from wild ancestors. With over 70 distinct breeds, cats come in an astounding variety of colors and temperaments.\n\n",
        "Ancient Egyptians held cats in the highest regard. The goddess Bastet was depicted with the head of a cat and was worshipped as a deity of home and protection.\n\n",
        "Cats communicate through vocalizations, body language, and scent marking. The familiar meow is primarily used for humans - adult cats rarely meow at each other.\n\n",
        "A cat's purr, between 25 and 150 Hertz, may promote healing. A tail held high signals confidence, while a puffed tail indicates fear.\n\n",
    ];

    for (i, para) in paragraphs.iter().enumerate() {
        println!("[*] Typing paragraph {}...", i + 1);
        agent.execute(AgentAction::Type { text: para.to_string() }).await?;

        // Check state periodically
        if let Some(state) = agent.get_state().await {
            println!("    [State] {} (screenshots: {})",
                &state.description[..state.description.len().min(50)],
                agent.screenshot_count());
        }
    }

    println!();
    println!("[Step 6] Saving document...");
    agent.execute(AgentAction::KeyCombo { combo: "ctrl+s".into() }).await?;

    // Wait for save dialog
    agent.execute(AgentAction::Wait {
        condition: WaitCondition::ScreenStable { duration_ms: 500 }
    }).await?;

    agent.execute(AgentAction::Type { text: "cats_reactive".into() }).await?;
    agent.execute(AgentAction::KeyPress { key: "Return".into() }).await?;

    // Wait for save to complete
    agent.execute(AgentAction::Wait {
        condition: WaitCondition::ScreenStable { duration_ms: 1000 }
    }).await?;

    // Final state
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("COMPLETE");
    println!("═══════════════════════════════════════════════════════════════");

    if let Some(state) = agent.get_state().await {
        println!("[Final State] {}", state.description);
    }
    println!("[Total Screenshots] {}", agent.screenshot_count());

    // Stop agent
    agent.stop();
    println!("[✓] Agent stopped");

    Ok(())
}
