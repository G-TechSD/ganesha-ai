//! Flux Capacitor - Time-boxed autonomous execution mode
//!
//! "When this baby hits 88 miles per hour, you're gonna see some serious shit."
//!
//! The Flux Capacitor allows Ganesha to run autonomously for a specified period,
//! continuously iterating on tasks until time runs out or the mission is complete.
//!
//! Features:
//! - Duration-based: `--flux "1 hour"` or `--flux "30m"`
//! - Target time: `--until "11:11"` or `--until "23:30"`
//! - Auto-extend: `--flux auto` (runs until manually stopped)
//! - Extend mid-run: Press 'e' to add more time
//! - FluxCanvas: Persistent workspace for accumulating work across iterations

use chrono::{Duration, Local, NaiveTime, Timelike};
use console::style;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Parse a duration string like "1h", "30m", "1 hour", "2 hours 30 minutes"
pub fn parse_duration(input: &str) -> Option<Duration> {
    let input = input.trim().to_lowercase();

    // Handle "auto" for infinite mode
    if input == "auto" || input == "forever" || input == "infinite" {
        return Some(Duration::days(365)); // Effectively forever
    }

    // Try simple patterns first: "1h", "30m", "2h30m"
    let mut total_minutes = 0i64;
    let mut current_num = String::new();

    for c in input.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else if c == 'h' || c == 'H' {
            if let Ok(hours) = current_num.parse::<i64>() {
                total_minutes += hours * 60;
            }
            current_num.clear();
        } else if c == 'm' || c == 'M' {
            if let Ok(mins) = current_num.parse::<i64>() {
                total_minutes += mins;
            }
            current_num.clear();
        } else if c == 's' || c == 'S' {
            if let Ok(secs) = current_num.parse::<i64>() {
                total_minutes += secs / 60;
                // If less than a minute, round up to 1 minute minimum
                if total_minutes == 0 && secs > 0 {
                    total_minutes = 1;
                }
            }
            current_num.clear();
        }
    }

    if total_minutes > 0 {
        return Some(Duration::minutes(total_minutes));
    }

    // Try "X hours", "X minutes" patterns
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        if let Ok(num) = words[i].parse::<i64>() {
            if i + 1 < words.len() {
                let unit = words[i + 1];
                if unit.starts_with("hour") {
                    total_minutes += num * 60;
                } else if unit.starts_with("min") {
                    total_minutes += num;
                } else if unit.starts_with("sec") {
                    // Ignore
                }
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    if total_minutes > 0 {
        Some(Duration::minutes(total_minutes))
    } else {
        None
    }
}

/// Parse a target time like "11:11", "23:30", "11:11 PM"
pub fn parse_target_time(input: &str) -> Option<NaiveTime> {
    let input = input.trim().to_uppercase();

    // Handle 12-hour format with AM/PM
    let is_pm = input.contains("PM");
    let is_am = input.contains("AM");
    let time_part = input
        .replace("PM", "")
        .replace("AM", "")
        .replace(" ", "");

    // Parse HH:MM or H:MM
    let parts: Vec<&str> = time_part.split(':').collect();
    if parts.len() >= 2 {
        if let (Ok(mut hours), Ok(mins)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            // Convert 12-hour to 24-hour
            if is_pm && hours < 12 {
                hours += 12;
            } else if is_am && hours == 12 {
                hours = 0;
            }

            return NaiveTime::from_hms_opt(hours, mins, 0);
        }
    }

    None
}

/// Calculate duration until a target time (handles next-day wrap)
pub fn duration_until(target: NaiveTime) -> Duration {
    let now = Local::now().time();

    if target > now {
        // Target is later today
        let hours_diff = target.hour() as i64 - now.hour() as i64;
        let mins_diff = target.minute() as i64 - now.minute() as i64;
        Duration::minutes(hours_diff * 60 + mins_diff)
    } else {
        // Target is tomorrow
        let until_midnight = Duration::hours(24) - Duration::hours(now.hour() as i64)
            - Duration::minutes(now.minute() as i64);
        let after_midnight = Duration::hours(target.hour() as i64)
            + Duration::minutes(target.minute() as i64);
        until_midnight + after_midnight
    }
}

/// Flux Capacitor configuration
pub struct FluxConfig {
    pub duration: Duration,
    pub task: String,
    pub auto_extend: bool,
    pub provider_url: String,
    pub model: String,
    pub auto_approve: bool,
    pub verbose: bool,
    pub temperature: f32,
    pub seed: Option<i64>,
    pub resume: Option<String>,
}

/// Flux Capacitor status
pub struct FluxStatus {
    pub start_time: Instant,
    pub end_time: chrono::DateTime<Local>,
    pub iterations: usize,
    pub successes: usize,
    pub failures: usize,
    pub extended_count: usize,
}

impl FluxStatus {
    pub fn new(duration: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            end_time: Local::now() + duration,
            iterations: 0,
            successes: 0,
            failures: 0,
            extended_count: 0,
        }
    }

    pub fn remaining(&self) -> Duration {
        let now = Local::now();
        if now >= self.end_time {
            Duration::zero()
        } else {
            self.end_time - now
        }
    }

    pub fn is_time_up(&self) -> bool {
        Local::now() >= self.end_time
    }

    pub fn extend(&mut self, additional: Duration) {
        self.end_time = self.end_time + additional;
        self.extended_count += 1;
    }

    pub fn format_remaining(&self) -> String {
        let remaining = self.remaining();
        let hours = remaining.num_hours();
        let mins = remaining.num_minutes() % 60;
        let secs = remaining.num_seconds() % 60;

        if hours > 0 {
            format!("{}h {:02}m {:02}s", hours, mins, secs)
        } else if mins > 0 {
            format!("{}m {:02}s", mins, secs)
        } else {
            format!("{}s", secs)
        }
    }
}

/// FluxCanvas - Persistent workspace for accumulating work across iterations
///
/// The canvas provides:
/// - A list accumulator for building up items (facts, todos, test cases, etc.)
/// - A file tree for building entire codebases (path -> content)
/// - A code workspace for iterating on single files
/// - Key-value storage for tracking state
/// - Iteration history for context
///
/// Uses SQLite for efficient storage - O(1) inserts, no rewriting!
pub struct FluxCanvas {
    /// Session ID for this flux run
    pub session_id: String,
    /// Path to the SQLite database
    pub db_path: PathBuf,
    /// Database connection
    db: Connection,
    /// Target count (if building a list)
    pub target_count: Option<usize>,
    /// Target file count (if building a codebase)
    pub target_files: Option<usize>,
    /// Output directory for exporting codebase
    pub output_dir: Option<PathBuf>,
}

// Manual Clone since Connection doesn't implement Clone
impl Clone for FluxCanvas {
    fn clone(&self) -> Self {
        // Re-open the database connection
        let db = Connection::open(&self.db_path).expect("Failed to reopen database");
        Self {
            session_id: self.session_id.clone(),
            db_path: self.db_path.clone(),
            db,
            target_count: self.target_count,
            target_files: self.target_files,
            output_dir: self.output_dir.clone(),
        }
    }
}

impl FluxCanvas {
    /// Create a new canvas for this flux session
    pub fn new(task: &str) -> Self {
        let session_id = format!("flux_{}", chrono::Local::now().format("%Y%m%d_%H%M%S"));
        let db_path = std::env::temp_dir().join(format!("{}.db", session_id));

        // Try to detect target count from task (e.g., "1000 cat facts")
        let target_count = Self::detect_target_count(task);

        // Detect if this is a codebase/project task
        let target_files = Self::detect_target_files(task);
        let output_dir = Self::detect_output_dir(task);

        // Create database
        let db = Connection::open(&db_path).expect("Failed to create canvas database");

        // Initialize tables
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                summary TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_items_created ON items(created_at);
            "
        ).expect("Failed to initialize canvas tables");

        // Store metadata
        db.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('session_id', ?1)",
            params![&session_id],
        ).ok();
        db.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('task', ?1)",
            params![task],
        ).ok();
        if let Some(tc) = target_count {
            db.execute(
                "INSERT OR REPLACE INTO metadata (key, value) VALUES ('target_count', ?1)",
                params![tc.to_string()],
            ).ok();
        }

        Self {
            session_id,
            db_path,
            db,
            target_count,
            target_files,
            output_dir,
        }
    }

    /// Detect if task mentions a target file count
    fn detect_target_files(task: &str) -> Option<usize> {
        let task_lower = task.to_lowercase();
        let re = regex::Regex::new(r"(\d+)\s*(files?|modules?|components?|pages?|screens?)").ok()?;
        if let Some(caps) = re.captures(&task_lower) {
            if let Some(num_match) = caps.get(1) {
                return num_match.as_str().parse().ok();
            }
        }
        None
    }

    /// Detect output directory from task
    fn detect_output_dir(task: &str) -> Option<PathBuf> {
        let re = regex::Regex::new(r"(?:in|to|at)\s+([/~][\w/.-]+)").ok()?;
        if let Some(caps) = re.captures(task) {
            if let Some(path_match) = caps.get(1) {
                let path_str = path_match.as_str();
                let path = if path_str.starts_with('~') {
                    dirs::home_dir()?.join(&path_str[2..])
                } else {
                    PathBuf::from(path_str)
                };
                return Some(path);
            }
        }
        None
    }

    /// Detect if the task mentions a target count (e.g., "1000 facts", "100 test cases")
    fn detect_target_count(task: &str) -> Option<usize> {
        let task_lower = task.to_lowercase();
        let re = regex::Regex::new(r"(\d+)\s*(facts?|items?|things?|cases?|examples?|entries?|rows?|lines?|elements?)").ok()?;
        if let Some(caps) = re.captures(&task_lower) {
            if let Some(num_match) = caps.get(1) {
                return num_match.as_str().parse().ok();
            }
        }
        None
    }

    /// Add items to the accumulator - O(n) for n new items, NOT O(total)
    pub fn add_items(&mut self, new_items: Vec<String>) {
        let tx = self.db.transaction().expect("Failed to start transaction");
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO items (content) VALUES (?1)"
            ).expect("Failed to prepare insert");

            for item in &new_items {
                stmt.execute(params![item]).ok();
            }
        }
        tx.commit().expect("Failed to commit items");
    }

    /// Add a single item - O(1)
    pub fn add_item(&mut self, item: String) {
        self.db.execute(
            "INSERT INTO items (content) VALUES (?1)",
            params![item],
        ).ok();
    }

    /// Get item count - O(1) with index
    pub fn item_count(&self) -> usize {
        self.db.query_row(
            "SELECT COUNT(*) FROM items",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) as usize
    }

    /// Get all items (for export) - only call when needed
    pub fn get_all_items(&self) -> Vec<String> {
        let mut stmt = self.db.prepare("SELECT content FROM items ORDER BY id").unwrap();
        let items: Vec<String> = stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        items
    }

    /// Get last N items for context - O(1)
    pub fn get_last_items(&self, n: usize) -> Vec<String> {
        let mut stmt = self.db.prepare(
            "SELECT content FROM items ORDER BY id DESC LIMIT ?1"
        ).unwrap();
        let items: Vec<String> = stmt.query_map(params![n as i64], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        items.into_iter().rev().collect()
    }

    /// Add or update a file in the codebase - O(1)
    pub fn set_file(&mut self, path: &str, content: String) {
        self.db.execute(
            "INSERT OR REPLACE INTO files (path, content, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            params![path, content],
        ).ok();
    }

    /// Add multiple files at once
    pub fn add_files(&mut self, new_files: HashMap<String, String>) {
        let tx = self.db.transaction().expect("Failed to start transaction");
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO files (path, content, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)"
            ).expect("Failed to prepare insert");

            for (path, content) in &new_files {
                stmt.execute(params![path, content]).ok();
            }
        }
        tx.commit().expect("Failed to commit files");
    }

    /// Get a file's content
    pub fn get_file(&self, path: &str) -> Option<String> {
        self.db.query_row(
            "SELECT content FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        ).ok()
    }

    /// Get all files (for export)
    pub fn get_all_files(&self) -> HashMap<String, String> {
        let mut stmt = self.db.prepare("SELECT path, content FROM files").unwrap();
        let files: HashMap<String, String> = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
        files
    }

    /// Get file count - O(1)
    pub fn file_count(&self) -> usize {
        self.db.query_row(
            "SELECT COUNT(*) FROM files",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) as usize
    }

    /// Check if file target is reached
    pub fn files_target_reached(&self) -> bool {
        if let Some(target) = self.target_files {
            self.file_count() >= target
        } else {
            false
        }
    }

    /// Set a state value - O(1)
    pub fn set_state(&mut self, key: &str, value: &str) {
        self.db.execute(
            "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
            params![key, value],
        ).ok();
    }

    /// Get a state value - O(1)
    pub fn get_state(&self, key: &str) -> Option<String> {
        self.db.query_row(
            "SELECT value FROM state WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).ok()
    }

    /// Record what was done this iteration - O(1)
    pub fn record_iteration(&mut self, summary: &str) {
        self.db.execute(
            "INSERT INTO history (summary) VALUES (?1)",
            params![summary],
        ).ok();
    }

    /// Get recent history - O(1)
    pub fn get_recent_history(&self, n: usize) -> Vec<String> {
        let mut stmt = self.db.prepare(
            "SELECT created_at || ' ' || summary FROM history ORDER BY id DESC LIMIT ?1"
        ).unwrap();
        let history: Vec<String> = stmt.query_map(params![n as i64], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        history.into_iter().rev().collect()
    }

    /// Get file list with sizes (limited) - for context display
    pub fn get_file_list(&self, limit: usize) -> Vec<(String, usize)> {
        let mut stmt = self.db.prepare(
            "SELECT path, LENGTH(content) FROM files ORDER BY path LIMIT ?1"
        ).unwrap();
        stmt.query_map(params![limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Get all state key-value pairs
    pub fn get_all_state(&self) -> HashMap<String, String> {
        let mut stmt = self.db.prepare("SELECT key, value FROM state").unwrap();
        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Get progress toward target (if applicable)
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.target_count.map(|target| (self.item_count(), target))
    }

    /// Check if target is reached
    pub fn target_reached(&self) -> bool {
        if let Some(target) = self.target_count {
            self.item_count() >= target
        } else {
            false
        }
    }

    /// Get a context summary for the LLM
    pub fn get_context(&self) -> String {
        let mut context = String::new();

        context.push_str("=== FLUX CANVAS (Persistent Workspace) ===\n\n");

        // Items progress
        let item_count = self.item_count();
        if let Some((current, target)) = self.progress() {
            context.push_str(&format!("📊 ITEMS PROGRESS: {}/{} ({:.1}%)\n",
                current, target, (current as f64 / target as f64) * 100.0));
            context.push_str(&format!("🎯 REMAINING: {} items needed\n\n", target - current));
        }

        // Files progress
        let file_count = self.file_count();
        if let Some(target) = self.target_files {
            let pct = (file_count as f64 / target as f64) * 100.0;
            context.push_str(&format!("📁 FILES PROGRESS: {}/{} ({:.1}%)\n",
                file_count, target, pct));
            context.push_str(&format!("🎯 REMAINING: {} files needed\n\n", target - file_count));
        }

        // Current items count
        if item_count > 0 {
            context.push_str(&format!("📝 ACCUMULATED ITEMS: {} total\n", item_count));
            // Show last 5 items as reference
            context.push_str("   Last 5 items:\n");
            for item in self.get_last_items(5) {
                let truncated = truncate_str(&item, 60);
                context.push_str(&format!("   - {}\n", truncated));
            }
            context.push_str("\n");
        }

        // Current files
        if file_count > 0 {
            context.push_str(&format!("📁 FILES IN CODEBASE: {} total\n", file_count));
            // Show file list (limited)
            let files = self.get_file_list(20);
            for (path, size) in &files {
                context.push_str(&format!("   {} ({} bytes)\n", path, size));
            }
            if file_count > 20 {
                context.push_str(&format!("   ... and {} more files\n", file_count - 20));
            }
            context.push_str("\n");
        }

        // Recent history
        let history = self.get_recent_history(3);
        if !history.is_empty() {
            context.push_str("📜 RECENT ACTIONS:\n");
            for entry in &history {
                context.push_str(&format!("   {}\n", entry));
            }
            context.push_str("\n");
        }

        // State
        let state = self.get_all_state();
        if !state.is_empty() {
            context.push_str("🔧 STATE:\n");
            for (k, v) in &state {
                context.push_str(&format!("   {}: {}\n", k, v));
            }
            context.push_str("\n");
        }

        context.push_str("=== TO ADD FILES, use format: ===\n");
        context.push_str("FILE: path/to/file.ext\n");
        context.push_str("```\nfile content here\n```\n\n");
        context.push_str("===========================================\n\n");
        context
    }

    /// Save canvas to disk - no-op for SQLite (data is auto-persisted)
    pub fn save(&self) {
        // SQLite persists automatically - nothing to do
    }

    /// Load canvas from disk (for resume) - opens existing SQLite database
    pub fn load(path: &PathBuf) -> Option<Self> {
        if !path.exists() {
            return None;
        }

        let db = Connection::open(path).ok()?;

        // Read metadata
        let session_id: String = db.query_row(
            "SELECT value FROM metadata WHERE key = 'session_id'",
            [],
            |row| row.get(0),
        ).ok()?;

        let target_count: Option<usize> = db.query_row(
            "SELECT value FROM metadata WHERE key = 'target_count'",
            [],
            |row| row.get::<_, String>(0),
        ).ok().and_then(|s| s.parse().ok());

        let target_files: Option<usize> = db.query_row(
            "SELECT value FROM metadata WHERE key = 'target_files'",
            [],
            |row| row.get::<_, String>(0),
        ).ok().and_then(|s| s.parse().ok());

        let output_dir: Option<PathBuf> = db.query_row(
            "SELECT value FROM metadata WHERE key = 'output_dir'",
            [],
            |row| row.get::<_, String>(0),
        ).ok().map(PathBuf::from);

        Some(Self {
            session_id,
            db_path: path.clone(),
            db,
            target_count,
            target_files,
            output_dir,
        })
    }

    /// Export entire codebase to disk
    pub fn export_codebase(&self, base_dir: &PathBuf) -> std::io::Result<usize> {
        // Create base directory
        fs::create_dir_all(base_dir)?;

        let files = self.get_all_files();
        let mut exported = 0;
        for (path, content) in &files {
            let full_path = base_dir.join(path);

            // Create parent directories
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Write file
            fs::write(&full_path, content)?;
            exported += 1;
        }

        Ok(exported)
    }

    /// Get total size of all files
    pub fn total_size(&self) -> usize {
        self.db.query_row(
            "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM files",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) as usize
    }

    /// Find and load a canvas by session ID or path
    pub fn find_and_load(session_or_path: &str) -> Option<Self> {
        // First try as a direct path
        let path = PathBuf::from(session_or_path);
        if path.exists() {
            return Self::load(&path);
        }

        // Try adding .db extension
        let with_ext = PathBuf::from(format!("{}.db", session_or_path));
        if with_ext.exists() {
            return Self::load(&with_ext);
        }

        // Search in temp directory for matching session
        let temp_dir = std::env::temp_dir();
        if let Ok(entries) = fs::read_dir(&temp_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(session_or_path) && name.ends_with(".db") && name.starts_with("flux_") {
                    return Self::load(&entry.path());
                }
            }
        }

        // Try with flux_ prefix
        let flux_path = temp_dir.join(format!("flux_{}.db", session_or_path));
        if flux_path.exists() {
            return Self::load(&flux_path);
        }

        None
    }

    /// List all available canvas sessions
    pub fn list_sessions() -> Vec<(String, PathBuf, usize)> {
        let mut sessions = Vec::new();
        let temp_dir = std::env::temp_dir();

        if let Ok(entries) = fs::read_dir(&temp_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if name.starts_with("flux_") && name.ends_with(".db") {
                    if let Some(canvas) = Self::load(&path) {
                        let count = canvas.item_count();
                        sessions.push((canvas.session_id, path, count));
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.2.cmp(&a.2)); // Sort by item count descending
        sessions
    }

    /// Export items to a file
    pub fn export_items(&self, path: &str) -> std::io::Result<()> {
        let items = self.get_all_items();
        fs::write(path, items.join("\n"))
    }

    /// Generate HTML output for list-based tasks
    pub fn export_html(&self, title: &str, path: &str) -> std::io::Result<()> {
        let items = self.get_all_items();
        let items_html: String = items.iter()
            .map(|item| format!("<li>{}</li>", item))
            .collect::<Vec<_>>()
            .join("\n");

        let item_count = items.len();
        let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{}</title>
    <style>
        body {{ font-family: system-ui, sans-serif; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; margin: 0; padding: 20px; }}
        .container {{ max-width: 900px; margin: auto; background: white; padding: 40px; border-radius: 16px; box-shadow: 0 10px 40px rgba(0,0,0,.2); }}
        h1 {{ text-align: center; font-size: 2.5em; margin-bottom: 10px; }}
        .counter {{ text-align: center; font-size: 1.3em; color: #764ba2; margin-bottom: 30px; }}
        ul {{ list-style: none; padding: 0; display: grid; gap: 10px; }}
        li {{ background: linear-gradient(135deg, #f5f7fa 0%, #e4e8ed 100%); padding: 12px 18px; border-radius: 8px; border-left: 4px solid #667eea; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>{}</h1>
        <div class="counter">{} items</div>
        <ul>
{}
        </ul>
    </div>
</body>
</html>"#, title, title, item_count, items_html);

        fs::write(path, html)
    }
}

/// Print the Flux Capacitor banner
pub fn print_flux_banner(end_time: &chrono::DateTime<Local>, task: &str) {
    println!();
    println!("{}", style("╔══════════════════════════════════════════════════════════════╗").cyan());
    println!("{}", style("║            ⚡ FLUX CAPACITOR ENGAGED ⚡                       ║").cyan().bold());
    println!("{}", style("╠══════════════════════════════════════════════════════════════╣").cyan());
    println!("{}  Target time: {}",
        style("║").cyan(),
        style(end_time.format("%H:%M:%S").to_string()).yellow().bold()
    );
    println!("{}  Task: {}",
        style("║").cyan(),
        style(truncate_str(task, 50)).white()
    );
    println!("{}", style("║                                                              ║").cyan());
    println!("{}  Press {} to extend time, {} to stop",
        style("║").cyan(),
        style("'e'").green().bold(),
        style("Ctrl+C").red()
    );
    println!("{}", style("╚══════════════════════════════════════════════════════════════╝").cyan());
    println!();
}

/// Print iteration status
pub fn print_iteration_status(status: &FluxStatus, iteration_result: Option<bool>) {
    let remaining = status.format_remaining();
    let speed = if status.iterations > 0 {
        let elapsed_secs = status.start_time.elapsed().as_secs_f64();
        let mph = (status.iterations as f64 / elapsed_secs) * 3600.0;
        format!("{:.1} iterations/hr", mph)
    } else {
        "warming up...".to_string()
    };

    let result_indicator = match iteration_result {
        Some(true) => style("✓").green().bold().to_string(),
        Some(false) => style("✗").red().bold().to_string(),
        None => style("→").dim().to_string(),
    };

    println!(
        "{} {} Iteration {} | {} remaining | {} | {}/{} success",
        style("⚡").cyan(),
        result_indicator,
        style(status.iterations).bold(),
        style(remaining).yellow(),
        style(speed).dim(),
        style(status.successes).green(),
        style(status.iterations).dim()
    );
}

/// Print iteration status with canvas progress
pub fn print_iteration_status_with_canvas(
    status: &FluxStatus,
    canvas: &FluxCanvas,
    iteration_result: Option<bool>,
    items_added: usize,
) {
    let remaining = status.format_remaining();

    let result_indicator = match iteration_result {
        Some(true) => style("✓").green().bold().to_string(),
        Some(false) => style("✗").red().bold().to_string(),
        None => style("→").dim().to_string(),
    };

    // Build progress string if we have a target
    let item_count = canvas.item_count();
    let progress_str = if let Some((current, target)) = canvas.progress() {
        let pct = (current as f64 / target as f64) * 100.0;
        format!(" | 📊 {}/{} ({:.1}%)", current, target, pct)
    } else if item_count > 0 {
        format!(" | 📝 {} items", item_count)
    } else {
        String::new()
    };

    let items_str = if items_added > 0 {
        format!(" +{}", style(items_added).green())
    } else {
        String::new()
    };

    println!(
        "{} {} Iter {} | {} left{}{}",
        style("⚡").cyan(),
        result_indicator,
        style(status.iterations).bold(),
        style(remaining).yellow(),
        progress_str,
        items_str
    );
}

/// Truncate a string to max length
fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count: usize = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        // Find the byte index for the (max_chars - 3)th character
        let truncate_at = s.char_indices()
            .nth(max_chars - 3)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        format!("{}...", &s[..truncate_at])
    }
}

/// Parse ITEM: lines from write tool JSON content
/// Looks for {"name":"write","args":{"content":"..."}} patterns and extracts ITEM: lines
fn parse_items_from_write_tool(response: &str) -> Vec<String> {
    let mut items = Vec::new();

    // Look for write tool JSON with content containing ITEM: lines
    // Pattern: "content":"...ITEM: fact...\nITEM: fact..."
    let re = regex::Regex::new(r#""content"\s*:\s*"([^"]*(?:\\.[^"]*)*)""#).unwrap();

    for caps in re.captures_iter(response) {
        if let Some(content_match) = caps.get(1) {
            // Unescape the JSON string
            let content = content_match.as_str()
                .replace("\\n", "\n")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");

            // Extract ITEM: lines from the content
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("ITEM:") {
                    if let Some(item) = trimmed.strip_prefix("ITEM:") {
                        let item = item.trim().to_string();
                        if !item.is_empty() {
                            items.push(item);
                        }
                    }
                }
            }
        }
    }

    items
}

/// Parse FILE: blocks from LLM response
/// Format: FILE: path/to/file.ext
///         ```
///         content here
///         ```
fn parse_file_blocks(response: &str) -> HashMap<String, String> {
    let mut files = HashMap::new();

    // Regex to match FILE: path followed by code block
    let re = regex::Regex::new(
        r"(?m)^FILE:\s*(.+?)\s*$\s*```[^\n]*\n([\s\S]*?)```"
    ).unwrap();

    for caps in re.captures_iter(response) {
        if let (Some(path_match), Some(content_match)) = (caps.get(1), caps.get(2)) {
            let path = path_match.as_str().trim().to_string();
            let content = content_match.as_str().to_string();
            if !path.is_empty() && !content.is_empty() {
                files.insert(path, content);
            }
        }
    }

    // Also try alternative format: ### path/to/file.ext
    let alt_re = regex::Regex::new(
        r"(?m)^###\s*(.+?\.\w+)\s*$\s*```[^\n]*\n([\s\S]*?)```"
    ).unwrap();

    for caps in alt_re.captures_iter(response) {
        if let (Some(path_match), Some(content_match)) = (caps.get(1), caps.get(2)) {
            let path = path_match.as_str().trim().to_string();
            let content = content_match.as_str().to_string();
            if !path.is_empty() && !content.is_empty() && !files.contains_key(&path) {
                files.insert(path, content);
            }
        }
    }

    files
}

/// Run the Flux Capacitor
pub async fn run_flux_capacitor(config: FluxConfig) -> Result<FluxStatus, String> {
    use crate::agent_wiggum::{AgentConfig, WiggumAgent};

    let mut status = FluxStatus::new(config.duration);

    // Load existing canvas if resuming, otherwise create new
    let mut canvas = if let Some(ref session) = config.resume {
        match FluxCanvas::find_and_load(session) {
            Some(c) => {
                println!("{} Resuming session: {} ({} items, {} files)",
                    style("♻️").green(),
                    style(&c.session_id).cyan(),
                    c.item_count(),
                    c.file_count()
                );
                c
            }
            None => {
                println!("{} Session '{}' not found, starting fresh",
                    style("⚠").yellow(),
                    session
                );
                FluxCanvas::new(&config.task)
            }
        }
    } else {
        FluxCanvas::new(&config.task)
    };

    print_flux_banner(&status.end_time, &config.task);

    // Show canvas info if we detected a target
    if let Some(target) = canvas.target_count {
        println!("{}  Canvas: Accumulating {} items",
            style("║").cyan(),
            style(target).yellow().bold()
        );
        println!("{}", style("╚══════════════════════════════════════════════════════════════╝").cyan());
    }
    println!();

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl+C handler");

    // Set up agent with configurable temperature
    let agent_config = AgentConfig {
        provider_url: config.provider_url.clone(),
        model: config.model.clone(),
        auto_approve: config.auto_approve,
        verify_actions: true,
        verbose: config.verbose,
        temperature: config.temperature,
        seed: config.seed,
        ..Default::default()
    };

    let mut agent = WiggumAgent::new(agent_config);

    // Main flux loop
    while running.load(Ordering::SeqCst) && !status.is_time_up() && !canvas.target_reached() {
        status.iterations += 1;

        // Check for user input (non-blocking)
        if let Some(extension) = check_for_extend_request() {
            status.extend(extension);
            println!(
                "{} Extended! New end time: {}",
                style("⏰").green(),
                style(status.end_time.format("%H:%M:%S")).yellow().bold()
            );
        }

        // Build contextual task with canvas context
        let canvas_context = canvas.get_context();
        let contextual_task = if status.iterations == 1 {
            format!(
                "{}\n\n{}\n\nIMPORTANT: Generate NEW unique items. Output each item on its own line prefixed with 'ITEM:' so they can be collected. Example:\nITEM: First fact here\nITEM: Second fact here",
                config.task,
                canvas_context
            )
        } else {
            format!(
                "{}\n\n{}\n\n[Flux Iteration {} - {} remaining]\n\nIMPORTANT: Generate MORE unique items that are DIFFERENT from previous ones. Output each NEW item on its own line prefixed with 'ITEM:' so they can be collected.",
                config.task,
                canvas_context,
                status.iterations,
                status.format_remaining()
            )
        };

        // Run the task
        match agent.run_task(&contextual_task).await {
            Ok(result) => {
                status.successes += 1;

                // Parse response for new items (lines starting with "ITEM:")
                let mut new_items: Vec<String> = result.final_response
                    .lines()
                    .filter(|line| line.trim().starts_with("ITEM:"))
                    .map(|line| line.trim().strip_prefix("ITEM:").unwrap_or(line).trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect();

                // Also parse ITEM: lines from write tool JSON content
                let write_tool_items = parse_items_from_write_tool(&result.final_response);
                new_items.extend(write_tool_items);

                // Deduplicate items
                new_items.sort();
                new_items.dedup();

                if !new_items.is_empty() {
                    canvas.add_items(new_items.clone());
                    canvas.record_iteration(&format!("Added {} items (total: {})", new_items.len(), canvas.item_count()));
                }

                // Parse response for new files (FILE: path followed by code block)
                let new_files = parse_file_blocks(&result.final_response);
                if !new_files.is_empty() {
                    let file_count = new_files.len();
                    canvas.add_files(new_files);
                    canvas.record_iteration(&format!("Added {} files (total: {})", file_count, canvas.file_count()));
                }

                // Print status with canvas progress
                let items_added = new_items.len();
                print_iteration_status_with_canvas(&status, &canvas, Some(true), items_added);

                if config.verbose && new_items.is_empty() {
                    println!("{}", style(&result.final_response).dim());
                }
            }
            Err(e) => {
                status.failures += 1;
                print_iteration_status_with_canvas(&status, &canvas, Some(false), 0);

                if config.verbose {
                    println!("{} {}", style("Error:").red(), e);
                }
            }
        }

        // Small delay between iterations
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Export canvas contents
    println!();

    // Export items if we accumulated any
    let final_item_count = canvas.item_count();
    if final_item_count > 0 {
        // Determine output filename from task or use default
        let output_file = if config.task.to_lowercase().contains("cat") {
            "/tmp/catfacts.html"
        } else {
            "/tmp/flux_output.html"
        };

        // Extract title from task
        let title = if let Some(target) = canvas.target_count {
            format!("{} Items", target)
        } else {
            "Flux Capacitor Output".to_string()
        };

        if let Err(e) = canvas.export_html(&title, output_file) {
            println!("{} Failed to export items: {}", style("⚠").yellow(), e);
        } else {
            println!("{} Exported {} items to {}",
                style("📄").green(),
                final_item_count,
                style(output_file).cyan()
            );
        }

        // Also save raw items
        let raw_file = output_file.replace(".html", ".txt");
        let _ = canvas.export_items(&raw_file);
    }

    // Export codebase if we accumulated files
    let final_file_count = canvas.file_count();
    if final_file_count > 0 {
        let output_dir = canvas.output_dir.clone()
            .unwrap_or_else(|| std::env::temp_dir().join(format!("flux_codebase_{}", canvas.session_id)));

        match canvas.export_codebase(&output_dir) {
            Ok(count) => {
                println!("{} Exported {} files ({} bytes) to {}",
                    style("📁").green(),
                    count,
                    canvas.total_size(),
                    style(output_dir.display()).cyan()
                );
            }
            Err(e) => {
                println!("{} Failed to export codebase: {}", style("⚠").yellow(), e);
            }
        }
    }

    // Final summary with canvas info
    print_flux_summary_with_canvas(&status, &canvas);

    Ok(status)
}

/// Check for extension request (non-blocking stdin check)
fn check_for_extend_request() -> Option<Duration> {
    // This is a simplified check - in practice we'd use terminal raw mode
    // For now, we'll check if there's input available

    // Set stdin to non-blocking temporarily
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        let stdin = io::stdin();
        let fd = stdin.as_raw_fd();

        // Check if there's data available using select with 0 timeout
        let mut readfds: libc::fd_set = unsafe { std::mem::zeroed() };
        unsafe {
            libc::FD_ZERO(&mut readfds);
            libc::FD_SET(fd, &mut readfds);
        }

        let mut timeout = libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };

        let result = unsafe {
            libc::select(
                fd + 1,
                &mut readfds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeout,
            )
        };

        if result > 0 {
            let mut buf = [0u8; 1];
            if io::stdin().read(&mut buf).is_ok() {
                if buf[0] == b'e' || buf[0] == b'E' {
                    // Extend by 15 minutes by default
                    return Some(Duration::minutes(15));
                }
            }
        }
    }

    None
}

/// Print final summary with canvas info
fn print_flux_summary_with_canvas(status: &FluxStatus, canvas: &FluxCanvas) {
    let elapsed = status.start_time.elapsed();
    let elapsed_secs = elapsed.as_secs();
    let elapsed_str = if elapsed_secs >= 3600 {
        format!(
            "{}h {:02}m {:02}s",
            elapsed_secs / 3600,
            (elapsed_secs % 3600) / 60,
            elapsed_secs % 60
        )
    } else if elapsed_secs >= 60 {
        format!("{}m {:02}s", elapsed_secs / 60, elapsed_secs % 60)
    } else {
        format!("{}s", elapsed_secs)
    };

    let success_rate = if status.iterations > 0 {
        (status.successes as f64 / status.iterations as f64) * 100.0
    } else {
        0.0
    };

    println!();
    println!("{}", style("╔══════════════════════════════════════════════════════════════╗").cyan());

    // Check if target was reached
    if canvas.target_reached() {
        println!("{}", style("║           🎯 TARGET REACHED! MISSION COMPLETE! 🎯            ║").green().bold());
    } else {
        println!("{}", style("║              ⚡ FLUX CAPACITOR COMPLETE ⚡                    ║").cyan().bold());
    }

    println!("{}", style("╠══════════════════════════════════════════════════════════════╣").cyan());
    println!("{}  Total time:    {}", style("║").cyan(), style(&elapsed_str).yellow());
    println!("{}  Iterations:    {}", style("║").cyan(), style(status.iterations).white().bold());
    println!("{}  Successes:     {}", style("║").cyan(), style(status.successes).green());
    println!("{}  Failures:      {}", style("║").cyan(), style(status.failures).red());
    println!("{}  Success rate:  {}%", style("║").cyan(), style(format!("{:.1}", success_rate)).white());

    // Canvas stats
    let item_count = canvas.item_count();
    let file_count = canvas.file_count();
    if item_count > 0 || file_count > 0 {
        println!("{}", style("╠══════════════════════════════════════════════════════════════╣").cyan());

        if item_count > 0 {
            println!("{}  📝 Items collected: {}", style("║").cyan(), style(item_count).green().bold());
            if let Some((current, target)) = canvas.progress() {
                let pct = (current as f64 / target as f64) * 100.0;
                println!("{}  📊 Progress: {}/{} ({:.1}%)", style("║").cyan(), current, target, pct);
            }
        }

        if file_count > 0 {
            println!("{}  📁 Files generated: {}", style("║").cyan(), style(file_count).green().bold());
            println!("{}  💾 Total size: {} bytes", style("║").cyan(), canvas.total_size());
            if let Some(target) = canvas.target_files {
                let pct = (file_count as f64 / target as f64) * 100.0;
                println!("{}  📊 Progress: {}/{} ({:.1}%)", style("║").cyan(), file_count, target, pct);
            }
        }
    }

    if status.extended_count > 0 {
        println!("{}  ⏰ Extensions:    {}", style("║").cyan(), style(status.extended_count).cyan());
    }
    println!("{}", style("╚══════════════════════════════════════════════════════════════╝").cyan());
    println!();
}

/// Print final summary
fn print_flux_summary(status: &FluxStatus) {
    let elapsed = status.start_time.elapsed();
    let elapsed_secs = elapsed.as_secs();
    let elapsed_str = if elapsed_secs >= 3600 {
        format!(
            "{}h {:02}m {:02}s",
            elapsed_secs / 3600,
            (elapsed_secs % 3600) / 60,
            elapsed_secs % 60
        )
    } else if elapsed_secs >= 60 {
        format!("{}m {:02}s", elapsed_secs / 60, elapsed_secs % 60)
    } else {
        format!("{}s", elapsed_secs)
    };

    let success_rate = if status.iterations > 0 {
        (status.successes as f64 / status.iterations as f64) * 100.0
    } else {
        0.0
    };

    println!();
    println!("{}", style("╔══════════════════════════════════════════════════════════════╗").cyan());
    println!("{}", style("║              ⚡ FLUX CAPACITOR COMPLETE ⚡                    ║").cyan().bold());
    println!("{}", style("╠══════════════════════════════════════════════════════════════╣").cyan());
    println!("{}  Total time:    {}", style("║").cyan(), style(&elapsed_str).yellow());
    println!("{}  Iterations:    {}", style("║").cyan(), style(status.iterations).white().bold());
    println!("{}  Successes:     {}", style("║").cyan(), style(status.successes).green());
    println!("{}  Failures:      {}", style("║").cyan(), style(status.failures).red());
    println!("{}  Success rate:  {}%", style("║").cyan(), style(format!("{:.1}", success_rate)).white());
    if status.extended_count > 0 {
        println!("{}  Extensions:    {}", style("║").cyan(), style(status.extended_count).cyan());
    }
    println!("{}", style("╚══════════════════════════════════════════════════════════════╝").cyan());
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("1h"), Some(Duration::minutes(60)));
        assert_eq!(parse_duration("30m"), Some(Duration::minutes(30)));
        assert_eq!(parse_duration("1h30m"), Some(Duration::minutes(90)));
        assert_eq!(parse_duration("2 hours"), Some(Duration::minutes(120)));
        assert_eq!(parse_duration("1 hour 30 minutes"), Some(Duration::minutes(90)));
        assert!(parse_duration("auto").unwrap() > Duration::hours(1000));
    }

    #[test]
    fn test_parse_target_time() {
        assert_eq!(parse_target_time("11:11"), NaiveTime::from_hms_opt(11, 11, 0));
        assert_eq!(parse_target_time("23:30"), NaiveTime::from_hms_opt(23, 30, 0));
        assert_eq!(parse_target_time("11:11 PM"), NaiveTime::from_hms_opt(23, 11, 0));
        assert_eq!(parse_target_time("11:11 AM"), NaiveTime::from_hms_opt(11, 11, 0));
    }
}
