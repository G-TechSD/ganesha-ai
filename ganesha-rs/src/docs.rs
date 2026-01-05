//! Dynamic Documentation Loader - Context-aware GUI/OS docs
//!
//! Fetches relevant documentation based on current context:
//! - Detected app/website
//! - Current OS and desktop environment
//! - Task being performed
//!
//! Pluggable backend: Context7, local docs, custom knowledge base, etc.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════════════
// TRAIT: Documentation Provider (pluggable backend)
// ═══════════════════════════════════════════════════════════════════════════════

/// A documentation provider that can fetch relevant docs for a context
#[async_trait]
pub trait DocsProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Check if provider is available/connected
    async fn is_available(&self) -> bool;

    /// Fetch docs for a specific topic
    async fn fetch(&self, query: &str) -> Result<Vec<DocSnippet>, String>;

    /// Fetch docs relevant to current app context
    async fn fetch_for_app(&self, app_name: &str, task: &str) -> Result<Vec<DocSnippet>, String>;

    /// Fetch OS-specific GUI interaction patterns
    async fn fetch_os_patterns(&self, os: &str, desktop_env: &str) -> Result<Vec<DocSnippet>, String>;
}

/// A snippet of documentation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocSnippet {
    /// Source/title
    pub source: String,
    /// The documentation content (markdown)
    pub content: String,
    /// Relevance score 0.0-1.0
    pub relevance: f32,
    /// Category: "gui", "api", "keyboard", "accessibility", etc
    pub category: String,
    /// Tags for filtering
    pub tags: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// DOCUMENTATION LOADER (orchestrates providers)
// ═══════════════════════════════════════════════════════════════════════════════

/// Documentation loader with fallback chain
pub struct DocsLoader {
    providers: Vec<Arc<dyn DocsProvider>>,
    cache: tokio::sync::RwLock<HashMap<String, CachedDocs>>,
    cache_ttl_secs: u64,
}

#[derive(Clone)]
struct CachedDocs {
    snippets: Vec<DocSnippet>,
    fetched_at: std::time::Instant,
}

impl DocsLoader {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            cache: tokio::sync::RwLock::new(HashMap::new()),
            cache_ttl_secs: 300, // 5 minute cache
        }
    }

    /// Add a documentation provider
    pub fn add_provider(&mut self, provider: Arc<dyn DocsProvider>) {
        self.providers.push(provider);
    }

    /// Get docs for current context (auto-detects best source)
    pub async fn get_context_docs(
        &self,
        app_name: &str,
        os: &str,
        desktop_env: &str,
        task: &str,
    ) -> Vec<DocSnippet> {
        let cache_key = format!("{}:{}:{}:{}", app_name, os, desktop_env, task);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.fetched_at.elapsed().as_secs() < self.cache_ttl_secs {
                    return cached.snippets.clone();
                }
            }
        }

        let mut all_docs = Vec::new();

        // Try each provider
        for provider in &self.providers {
            if !provider.is_available().await {
                continue;
            }

            // Fetch app-specific docs
            if let Ok(docs) = provider.fetch_for_app(app_name, task).await {
                all_docs.extend(docs);
            }

            // Fetch OS patterns
            if let Ok(docs) = provider.fetch_os_patterns(os, desktop_env).await {
                all_docs.extend(docs);
            }
        }

        // Sort by relevance
        all_docs.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));

        // Deduplicate
        all_docs.dedup_by(|a, b| a.source == b.source);

        // Take top results
        all_docs.truncate(10);

        // Cache results
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, CachedDocs {
                snippets: all_docs.clone(),
                fetched_at: std::time::Instant::now(),
            });
        }

        all_docs
    }

    /// Format docs for LLM context injection
    pub fn format_for_context(docs: &[DocSnippet], max_chars: usize) -> String {
        if docs.is_empty() {
            return String::new();
        }

        let mut output = String::from("## Relevant Documentation\n\n");
        let mut chars_used = output.len();

        for doc in docs {
            let entry = format!(
                "### {} [{}]\n{}\n\n",
                doc.source, doc.category, doc.content
            );

            if chars_used + entry.len() > max_chars {
                break;
            }

            output.push_str(&entry);
            chars_used += entry.len();
        }

        output
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONTEXT7 PROVIDER (MCP-based)
// ═══════════════════════════════════════════════════════════════════════════════

/// Context7 MCP documentation provider
pub struct Context7Provider {
    /// MCP endpoint (if running as separate process)
    endpoint: Option<String>,
    /// Library mappings: app name -> Context7 library ID
    library_map: HashMap<String, String>,
}

impl Context7Provider {
    pub fn new() -> Self {
        let mut library_map = HashMap::new();

        // Map common apps to their Context7 library IDs
        // These would be discovered/configured at runtime
        library_map.insert("firefox".into(), "mozilla/firefox-docs".into());
        library_map.insert("chromium".into(), "nicholasgriffintn/chromium-docs".into());
        library_map.insert("code".into(), "nicholasgriffintn/vscode-docs".into());
        library_map.insert("gnome".into(), "nicholasgriffintn/gnome-docs".into());
        library_map.insert("gtk".into(), "nicholasgriffintn/gtk-docs".into());
        library_map.insert("playwright".into(), "nicholasgriffintn/playwright-docs".into());
        library_map.insert("ebay".into(), "nicholasgriffintn/ebay-api-docs".into());

        Self {
            endpoint: None,
            library_map,
        }
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    /// Add custom library mapping
    pub fn add_library(&mut self, app: &str, library_id: &str) {
        self.library_map.insert(app.to_lowercase(), library_id.to_string());
    }

    /// Find best matching library for an app
    fn find_library(&self, app_name: &str) -> Option<&String> {
        let app_lower = app_name.to_lowercase();

        // Exact match
        if let Some(lib) = self.library_map.get(&app_lower) {
            return Some(lib);
        }

        // Partial match
        for (key, lib) in &self.library_map {
            if app_lower.contains(key) || key.contains(&app_lower) {
                return Some(lib);
            }
        }

        None
    }
}

#[async_trait]
impl DocsProvider for Context7Provider {
    fn name(&self) -> &str {
        "Context7"
    }

    async fn is_available(&self) -> bool {
        // TODO: Check if Context7 MCP is connected
        // For now, assume available if we have library mappings
        !self.library_map.is_empty()
    }

    async fn fetch(&self, query: &str) -> Result<Vec<DocSnippet>, String> {
        // TODO: Call Context7 MCP resolve-library-id and get-library-docs
        // For now, return placeholder
        Ok(vec![DocSnippet {
            source: format!("Context7: {}", query),
            content: format!("Documentation for '{}' would be fetched from Context7 MCP", query),
            relevance: 0.5,
            category: "general".into(),
            tags: vec![query.to_string()],
        }])
    }

    async fn fetch_for_app(&self, app_name: &str, task: &str) -> Result<Vec<DocSnippet>, String> {
        let library = self.find_library(app_name);

        if library.is_none() {
            return Ok(vec![]);
        }

        // TODO: Call Context7 MCP
        // mcp__context7__get-library-docs with library ID and topic
        Ok(vec![DocSnippet {
            source: format!("{} GUI Patterns", app_name),
            content: format!(
                "## {} Interaction Patterns\n\nFor task: {}\n\n[Would fetch from Context7 library: {:?}]",
                app_name, task, library
            ),
            relevance: 0.8,
            category: "gui".into(),
            tags: vec![app_name.to_lowercase(), "gui".into()],
        }])
    }

    async fn fetch_os_patterns(&self, os: &str, desktop_env: &str) -> Result<Vec<DocSnippet>, String> {
        let library = match desktop_env.to_lowercase().as_str() {
            "gnome" => self.library_map.get("gnome"),
            "kde" | "plasma" => self.library_map.get("kde"),
            _ => self.library_map.get("gtk"), // Default to GTK
        };

        if library.is_none() {
            return Ok(vec![]);
        }

        Ok(vec![DocSnippet {
            source: format!("{} ({}) Patterns", os, desktop_env),
            content: format!(
                "## {} Desktop Patterns\n\n- Window management\n- Keyboard shortcuts\n- Accessibility features\n\n[Would fetch from Context7]",
                desktop_env
            ),
            relevance: 0.6,
            category: "os".into(),
            tags: vec![os.to_lowercase(), desktop_env.to_lowercase()],
        }])
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LOCAL DOCS PROVIDER (fallback when offline)
// ═══════════════════════════════════════════════════════════════════════════════

/// Local documentation provider - embedded or filesystem-based
pub struct LocalDocsProvider {
    docs_dir: Option<std::path::PathBuf>,
    embedded: HashMap<String, Vec<DocSnippet>>,
}

impl LocalDocsProvider {
    pub fn new() -> Self {
        let mut embedded = HashMap::new();

        // Embed essential GUI patterns that are always available
        embedded.insert("browser".into(), vec![
            DocSnippet {
                source: "Browser Basics".into(),
                content: r#"## Browser GUI Patterns

### Navigation
- Address bar: Usually `input[type="text"]` or `input[name="q"]` at top
- Back/Forward: Look for buttons with aria-label="Back" or "Forward"
- Tabs: `.tab`, `[role="tab"]`, or browser-specific selectors

### Common Elements
- Search box: `input[type="search"]`, `#search`, `.search-input`
- Submit buttons: `button[type="submit"]`, `input[type="submit"]`
- Links: `a[href]`, filter by visible text

### Obstacles
- Cookie banners: `[class*="cookie"]`, `[id*="consent"]`
- Popups: `[role="dialog"]`, `.modal`, `[class*="popup"]`
- Overlays: `.overlay`, `[class*="overlay"]`

### Scrolling
- Infinite scroll: Watch for new elements loading
- Lazy images: May need scroll to trigger load
"#.into(),
                relevance: 1.0,
                category: "gui".into(),
                tags: vec!["browser".into(), "web".into()],
            },
        ]);

        embedded.insert("ebay".into(), vec![
            DocSnippet {
                source: "eBay Patterns".into(),
                content: r#"## eBay GUI Patterns

### Search
- Search box: `#gh-ac-box` or `input[name="_nkw"]`
- Search button: `#gh-btn`
- Category dropdown: `#gh-cat`

### Listings
- Item cards: `.s-item`
- Title: `.s-item__title`
- Price: `.s-item__price`
- Shipping: `.s-item__shipping`
- Item link: `a.s-item__link`

### Filters
- Left sidebar: `.srp-controls__control`
- Price range: `input[name="_udlo"]`, `input[name="_udhi"]`
- Condition: `.x-refine__select__svg`

### Pagination
- Next page: `.pagination__next`
- Page numbers: `.pagination__items a`
"#.into(),
                relevance: 1.0,
                category: "gui".into(),
                tags: vec!["ebay".into(), "shopping".into()],
            },
        ]);

        embedded.insert("linux".into(), vec![
            DocSnippet {
                source: "Linux Desktop Patterns".into(),
                content: r#"## Linux Desktop GUI Patterns

### Window Management (X11)
- Get window list: `wmctrl -l`
- Focus window: `wmctrl -a "Title"`
- Window geometry: `xdotool getwindowgeometry`

### GNOME Specifics
- Top bar: Activities button top-left
- App grid: Super key or Activities
- Notifications: Top-center clock area

### Keyboard Shortcuts
- Super: Open activities/app menu
- Alt+Tab: Switch windows
- Super+Arrow: Snap windows
- Ctrl+Alt+T: Terminal (usually)

### Accessibility
- Screen reader: Orca
- High contrast: Settings > Accessibility
- Large text: Settings > Accessibility
"#.into(),
                relevance: 0.9,
                category: "os".into(),
                tags: vec!["linux".into(), "gnome".into(), "x11".into()],
            },
        ]);

        Self {
            docs_dir: None,
            embedded,
        }
    }

    pub fn with_docs_dir(mut self, path: std::path::PathBuf) -> Self {
        self.docs_dir = Some(path);
        self
    }

    /// Add embedded documentation
    pub fn add_embedded(&mut self, key: &str, snippets: Vec<DocSnippet>) {
        self.embedded.insert(key.to_lowercase(), snippets);
    }
}

#[async_trait]
impl DocsProvider for LocalDocsProvider {
    fn name(&self) -> &str {
        "Local"
    }

    async fn is_available(&self) -> bool {
        true // Always available
    }

    async fn fetch(&self, query: &str) -> Result<Vec<DocSnippet>, String> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (key, snippets) in &self.embedded {
            if key.contains(&query_lower) || query_lower.contains(key) {
                results.extend(snippets.clone());
            }
        }

        Ok(results)
    }

    async fn fetch_for_app(&self, app_name: &str, _task: &str) -> Result<Vec<DocSnippet>, String> {
        self.fetch(app_name).await
    }

    async fn fetch_os_patterns(&self, os: &str, desktop_env: &str) -> Result<Vec<DocSnippet>, String> {
        let mut results = self.fetch(os).await?;
        results.extend(self.fetch(desktop_env).await?);
        Ok(results)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BUILDER
// ═══════════════════════════════════════════════════════════════════════════════

impl Default for DocsLoader {
    fn default() -> Self {
        let mut loader = Self::new();

        // Add default providers in priority order
        loader.add_provider(Arc::new(Context7Provider::new()));
        loader.add_provider(Arc::new(LocalDocsProvider::new()));

        loader
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docs_loader() {
        let loader = DocsLoader::default();

        let docs = loader.get_context_docs(
            "Firefox",
            "Linux",
            "GNOME",
            "search for vintage synth"
        ).await;

        assert!(!docs.is_empty());

        let formatted = DocsLoader::format_for_context(&docs, 2000);
        println!("{}", formatted);
        assert!(formatted.contains("Documentation"));
    }

    #[tokio::test]
    async fn test_local_provider() {
        let provider = LocalDocsProvider::new();

        let docs = provider.fetch("ebay").await.unwrap();
        assert!(!docs.is_empty());
        assert!(docs[0].content.contains("s-item"));
    }
}
