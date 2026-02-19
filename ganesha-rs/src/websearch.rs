//! Web Search module for Ganesha
//!
//! Provides web search capabilities using:
//! - Brave Search API (preferred, requires API key)
//! - DuckDuckGo HTML scraping (fallback, no key needed)

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Search result from any provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Web search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub provider: String,
}

/// Perform a web search using best available provider
pub async fn search(query: &str, max_results: usize) -> Result<SearchResponse, String> {
    // Try Brave Search first (if API key available)
    if let Ok(api_key) = std::env::var("BRAVE_SEARCH_API_KEY") {
        match brave_search(query, max_results, &api_key).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                eprintln!("[WebSearch] Brave failed, falling back to DuckDuckGo: {}", e);
            }
        }
    }

    // Fallback to DuckDuckGo (no API key needed)
    duckduckgo_search(query, max_results).await
}

/// Search using Brave Search API
async fn brave_search(query: &str, max_results: usize, api_key: &str) -> Result<SearchResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
        urlencoding::encode(query),
        max_results.min(20)
    );

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .send()
        .await
        .map_err(|e| format!("Brave API request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Brave API error: {}", response.status()));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Brave response: {}", e))?;

    let mut results = Vec::new();

    if let Some(web) = json.get("web").and_then(|w| w.get("results")).and_then(|r| r.as_array()) {
        for item in web.iter().take(max_results) {
            results.push(SearchResult {
                title: item.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                url: item.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string(),
                snippet: item.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string(),
            });
        }
    }

    Ok(SearchResponse {
        query: query.to_string(),
        results,
        provider: "Brave".to_string(),
    })
}

/// Search using DuckDuckGo HTML scraping (no API key needed)
async fn duckduckgo_search(query: &str, max_results: usize) -> Result<SearchResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
        .build()
        .map_err(|e| e.to_string())?;

    // Use DuckDuckGo HTML version
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("DuckDuckGo request failed: {}", e))?;

    let html = response
        .text()
        .await
        .map_err(|e| format!("Failed to read DuckDuckGo response: {}", e))?;

    // Check if DuckDuckGo is showing a CAPTCHA
    if html.contains("anomaly-modal") || html.contains("bots use DuckDuckGo") {
        return Err("DuckDuckGo CAPTCHA detected. Set BRAVE_SEARCH_API_KEY for reliable web search. Get a free key at https://brave.com/search/api/".to_string());
    }

    // Parse results from HTML
    let mut results = Vec::new();

    // Simple regex-based parsing (not perfect but works)
    let title_re = regex::Regex::new(r#"class="result__a"[^>]*>([^<]+)</a>"#).unwrap();
    let url_re = regex::Regex::new(r#"class="result__url"[^>]*>([^<]+)</a>"#).unwrap();
    let snippet_re = regex::Regex::new(r#"class="result__snippet"[^>]*>([^<]+)"#).unwrap();

    let titles: Vec<_> = title_re.captures_iter(&html).collect();
    let urls: Vec<_> = url_re.captures_iter(&html).collect();
    let snippets: Vec<_> = snippet_re.captures_iter(&html).collect();

    for i in 0..titles.len().min(max_results) {
        let title = titles.get(i).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("");
        let url = urls.get(i).and_then(|c| c.get(1)).map(|m| m.as_str().trim()).unwrap_or("");
        let snippet = snippets.get(i).and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("");

        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult {
                title: html_escape::decode_html_entities(title).to_string(),
                url: if url.starts_with("http") { url.to_string() } else { format!("https://{}", url) },
                snippet: html_escape::decode_html_entities(snippet).to_string(),
            });
        }
    }

    // If regex parsing failed, try a simpler approach
    if results.is_empty() {
        // Extract any URLs that look like search results
        let link_re = regex::Regex::new(r#"href="//duckduckgo.com/l/\?uddg=([^&"]+)"#).unwrap();
        for (i, cap) in link_re.captures_iter(&html).enumerate() {
            if i >= max_results { break; }
            if let Some(encoded_url) = cap.get(1) {
                if let Ok(decoded) = urlencoding::decode(encoded_url.as_str()) {
                    results.push(SearchResult {
                        title: format!("Result {}", i + 1),
                        url: decoded.to_string(),
                        snippet: String::new(),
                    });
                }
            }
        }
    }

    Ok(SearchResponse {
        query: query.to_string(),
        results,
        provider: "DuckDuckGo".to_string(),
    })
}

/// Format search results for LLM consumption
pub fn format_results(response: &SearchResponse) -> String {
    if response.results.is_empty() {
        return format!("No results found for: {}", response.query);
    }

    let mut output = format!("Search results for \"{}\" (via {}):\n\n", response.query, response.provider);

    for (i, result) in response.results.iter().enumerate() {
        output.push_str(&format!("{}. {}\n", i + 1, result.title));
        output.push_str(&format!("   URL: {}\n", result.url));
        if !result.snippet.is_empty() {
            output.push_str(&format!("   {}\n", result.snippet));
        }
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Network test - requires internet access, run with: cargo test -- --ignored"]
    async fn test_duckduckgo_search() {
        let result = duckduckgo_search("rust programming language", 5).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.results.is_empty());
        println!("{}", format_results(&response));
    }
}
