//! Ralph Playwright - Vision + Browser DOM Hybrid
//!
//! Combines:
//! - VISION (ministral-3b): Screen understanding, state detection
//! - PLAYWRIGHT: Precise DOM control, element clicking, data extraction
//! - PLANNER (gpt-oss-20b): Strategic decision making
//!
//! Run with: cargo run --example ralph_playwright --features "computer-use,browser"

use playwright::Playwright;
use std::time::Duration;
use tokio::time::sleep;

// Models
const VISION_ENDPOINT: &str = "http://192.168.27.182:1234/v1/chat/completions";
const VISION_MODEL: &str = "mistralai/ministral-3b-2410";
const PLANNER_ENDPOINT: &str = "http://192.168.245.155:1234/v1/chat/completions";
const PLANNER_MODEL: &str = "gpt-oss-20b";

#[derive(Debug, Clone)]
struct PageState {
    url: String,
    title: String,
    has_search: bool,
    search_selector: Option<String>,
    links: Vec<(String, String)>, // (text, href)
}

#[derive(Debug, Clone)]
struct PlannedAction {
    action: String,
    target: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║        RALPH PLAYWRIGHT - Vision + DOM Hybrid                 ║");
    println!("║                                                               ║");
    println!("║   VISION: ministral-3b - sees & understands                   ║");
    println!("║   PLAYWRIGHT: DOM access - precise control                    ║");
    println!("║   PLANNER: gpt-oss-20b - strategic decisions                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Initialize Playwright
    let playwright = Playwright::initialize().await?;
    playwright.install_chromium()?;

    let chromium = playwright.chromium();
    let browser = chromium.launcher()
        .headless(false)  // Show browser for visibility
        .launch()
        .await?;

    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    // Get goal
    println!("What would you like me to do?");
    print!("> ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut goal = String::new();
    io::stdin().read_line(&mut goal)?;
    let goal = goal.trim();

    if goal.is_empty() {
        println!("No goal. Exiting.");
        browser.close().await?;
        return Ok(());
    }

    println!("\n[*] Goal: {}\n", goal);

    // Run the task
    let mut history: Vec<String> = Vec::new();
    let max_steps = 20;

    for step in 1..=max_steps {
        // Get page state from DOM (fast, precise)
        let state = get_page_state(&page).await?;
        println!("[Step {}] 📄 {} - {}", step, state.title, state.url);

        // Ask planner what to do
        let action = ask_planner(&client, &goal, &state, &history).await?;
        println!("  🧠 Plan: {} {}", action.action, action.target);

        // Execute action
        match action.action.as_str() {
            "SEARCH_EBAY" => {
                let url = format!("https://www.ebay.com/sch/i.html?_nkw={}",
                    action.target.replace(' ', "+"));
                page.goto_builder(&url).goto().await?;
                history.push(format!("Searched eBay: {}", action.target));
            }
            "SEARCH_GOOGLE" => {
                let url = format!("https://www.google.com/search?q={}",
                    action.target.replace(' ', "+"));
                page.goto_builder(&url).goto().await?;
                history.push(format!("Searched Google: {}", action.target));
            }
            "VISIT" => {
                let url = if action.target.starts_with("http") {
                    action.target.clone()
                } else {
                    format!("https://{}", action.target)
                };
                page.goto_builder(&url).goto().await?;
                history.push(format!("Visited: {}", action.target));
            }
            "CLICK" => {
                // Click by selector or text
                if let Err(e) = page.click_builder(&action.target).click().await {
                    println!("  ⚠ Click failed: {}", e);
                } else {
                    history.push(format!("Clicked: {}", action.target));
                }
            }
            "TYPE" => {
                if let Some(selector) = &state.search_selector {
                    page.fill_builder(selector, &action.target).fill().await?;
                    history.push(format!("Typed: {}", action.target));
                }
            }
            "SCROLL" => {
                let delta = if action.target == "down" { 500 } else { -500 };
                let script = format!("window.scrollBy(0, {})", delta);
                let _ = page.evaluate::<(), ()>(&script, ()).await;
                history.push(format!("Scrolled: {}", action.target));
            }
            "EXTRACT_LINKS" => {
                println!("  📋 Found {} links:", state.links.len());
                for (i, (text, href)) in state.links.iter().take(10).enumerate() {
                    println!("    {}. {} -> {}", i+1, text, href);
                }
                history.push("Extracted links".to_string());
            }
            "DONE" => {
                println!("\n✓ Task complete!");
                break;
            }
            _ => {
                println!("  ⚠ Unknown action: {}", action.action);
            }
        }

        sleep(Duration::from_millis(500)).await;
    }

    println!("\n[*] Closing browser...");
    browser.close().await?;

    Ok(())
}

async fn get_page_state(page: &playwright::api::Page) -> Result<PageState, Box<dyn std::error::Error>> {
    let url = page.url().unwrap_or_default();
    let title = page.title().await.unwrap_or_default();

    // Simple state detection - just URL and title for now
    let has_search = url.contains("ebay") || url.contains("google");

    Ok(PageState {
        url,
        title,
        has_search,
        search_selector: None,
        links: vec![],
    })
}

async fn ask_planner(
    client: &reqwest::Client,
    goal: &str,
    state: &PageState,
    history: &[String],
) -> Result<PlannedAction, Box<dyn std::error::Error>> {
    let history_text = if history.is_empty() {
        "None".to_string()
    } else {
        history.iter().rev().take(5).cloned().collect::<Vec<_>>().join(" → ")
    };

    let state_desc = format!("URL: {} | Title: {} | Has search: {} | Links: {}",
        state.url, state.title, state.has_search, state.links.len());

    let tools = serde_json::json!([
        {"type": "function", "function": {
            "name": "search_ebay",
            "description": "Search eBay for products",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
        }},
        {"type": "function", "function": {
            "name": "search_google",
            "description": "Search Google",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
        }},
        {"type": "function", "function": {
            "name": "visit",
            "description": "Navigate to URL",
            "parameters": {"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}
        }},
        {"type": "function", "function": {
            "name": "click",
            "description": "Click element by CSS selector or text",
            "parameters": {"type": "object", "properties": {"selector": {"type": "string"}}, "required": ["selector"]}
        }},
        {"type": "function", "function": {
            "name": "scroll",
            "description": "Scroll page",
            "parameters": {"type": "object", "properties": {"direction": {"type": "string", "enum": ["up", "down"]}}, "required": ["direction"]}
        }},
        {"type": "function", "function": {
            "name": "extract_links",
            "description": "Show all links on current page",
            "parameters": {"type": "object", "properties": {}}
        }},
        {"type": "function", "function": {
            "name": "done",
            "description": "Task complete",
            "parameters": {"type": "object", "properties": {}}
        }}
    ]);

    let request = serde_json::json!({
        "model": PLANNER_MODEL,
        "messages": [
            {"role": "system", "content": "You control a browser. Call a tool to act. Be decisive."},
            {"role": "user", "content": format!("GOAL: {}\nPAGE: {}\nHISTORY: {}", goal, state_desc, history_text)}
        ],
        "tools": tools,
        "tool_choice": "required",
        "max_tokens": 100,
        "temperature": 0.0
    });

    let response = client.post(PLANNER_ENDPOINT).json(&request).send().await?;
    let result: serde_json::Value = response.json().await?;

    if let Some(calls) = result["choices"][0]["message"]["tool_calls"].as_array() {
        if let Some(call) = calls.first() {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: serde_json::Value = serde_json::from_str(
                call["function"]["arguments"].as_str().unwrap_or("{}")
            ).unwrap_or_default();

            return Ok(match name {
                "search_ebay" => PlannedAction {
                    action: "SEARCH_EBAY".into(),
                    target: args["query"].as_str().unwrap_or("").into(),
                },
                "search_google" => PlannedAction {
                    action: "SEARCH_GOOGLE".into(),
                    target: args["query"].as_str().unwrap_or("").into(),
                },
                "visit" => PlannedAction {
                    action: "VISIT".into(),
                    target: args["url"].as_str().unwrap_or("").into(),
                },
                "click" => PlannedAction {
                    action: "CLICK".into(),
                    target: args["selector"].as_str().unwrap_or("").into(),
                },
                "scroll" => PlannedAction {
                    action: "SCROLL".into(),
                    target: args["direction"].as_str().unwrap_or("down").into(),
                },
                "extract_links" => PlannedAction {
                    action: "EXTRACT_LINKS".into(),
                    target: String::new(),
                },
                "done" => PlannedAction {
                    action: "DONE".into(),
                    target: String::new(),
                },
                _ => PlannedAction {
                    action: "WAIT".into(),
                    target: String::new(),
                },
            });
        }
    }

    Ok(PlannedAction {
        action: "WAIT".into(),
        target: String::new(),
    })
}
