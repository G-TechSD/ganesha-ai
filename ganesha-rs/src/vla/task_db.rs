//! VLA Task Database - SQLite-backed action tracking and long-horizon context
//!
//! Every VLA task gets a persistent record of:
//! - The goal and sub-steps
//! - Every action attempted with before/after screen state
//! - Whether each action achieved its expected result
//! - Failed approaches (so the planner can avoid repeating mistakes)
//! - Screen state hashes for change detection
//!
//! This gives the planner memory across iterations:
//! "Last time I hit Super key from Firefox, I ended up in GNOME Activities. Don't do that."

use rusqlite::{Connection, Result as SqliteResult, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// VLA task tracker backed by SQLite
pub struct VlaTaskDb {
    conn: Connection,
}

impl VlaTaskDb {
    /// Open or create the VLA task database
    pub fn open() -> SqliteResult<Self> {
        let base_dir = Self::get_base_dir();
        let db_path = base_dir.join("vla_tasks.db");
        let conn = Connection::open(&db_path)?;
        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing)
    pub fn open_memory() -> SqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn get_base_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let base = home.join(".ganesha").join("vla");
        std::fs::create_dir_all(&base).ok();
        base
    }

    fn init_schema(&mut self) -> SqliteResult<()> {
        self.conn.execute_batch(r#"
            -- Top-level tasks (one per ganesha vla invocation)
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                criteria TEXT NOT NULL,       -- JSON array of success criteria
                status TEXT NOT NULL DEFAULT 'running',  -- running, success, failed, timeout, stopped
                started_at TEXT NOT NULL,
                ended_at TEXT,
                total_actions INTEGER DEFAULT 0,
                error TEXT,
                final_screen_state TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_started ON tasks(started_at DESC);

            -- Every action attempted within a task
            CREATE TABLE IF NOT EXISTS actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                step_num INTEGER NOT NULL,
                intent TEXT NOT NULL,
                action_type TEXT NOT NULL,     -- click, type, key_press, etc.
                target_desc TEXT,              -- element description
                target_x INTEGER,
                target_y INTEGER,
                text_input TEXT,               -- for type actions
                keys_input TEXT,               -- for key_press actions
                confidence REAL,
                expected_result TEXT,
                -- Outcome
                executed INTEGER DEFAULT 0,    -- was the action actually executed
                exec_success INTEGER DEFAULT 0,
                exec_error TEXT,
                exec_duration_ms INTEGER,
                -- Before/after screen state
                screen_before TEXT,            -- app/title/state before action
                screen_after TEXT,             -- app/title/state after action
                screen_changed INTEGER DEFAULT 0,  -- did the screen actually change
                expected_achieved INTEGER DEFAULT 0,
                -- Timestamps
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (task_id) REFERENCES tasks(id)
            );
            CREATE INDEX IF NOT EXISTS idx_actions_task ON actions(task_id, step_num);

            -- Failed approaches - things that didn't work, to avoid repeating
            CREATE TABLE IF NOT EXISTS failures (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT,                  -- NULL = global failure pattern
                context TEXT NOT NULL,         -- what screen/app state we were in
                action_tried TEXT NOT NULL,    -- what we tried
                what_happened TEXT NOT NULL,   -- what actually happened
                lesson TEXT NOT NULL,          -- what to do instead
                times_seen INTEGER DEFAULT 1,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (task_id) REFERENCES tasks(id)
            );
            CREATE INDEX IF NOT EXISTS idx_failures_context ON failures(context);

            -- Sub-steps for complex tasks (planner can decompose goals)
            CREATE TABLE IF NOT EXISTS substeps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                step_order INTEGER NOT NULL,
                description TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',  -- pending, active, done, failed, skipped
                completed_at TEXT,
                FOREIGN KEY (task_id) REFERENCES tasks(id)
            );
            CREATE INDEX IF NOT EXISTS idx_substeps_task ON substeps(task_id, step_order);

            -- Full-text search on failures for quick pattern matching
            CREATE VIRTUAL TABLE IF NOT EXISTS failures_fts USING fts5(
                context, action_tried, what_happened, lesson,
                content='failures',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS failures_ai AFTER INSERT ON failures BEGIN
                INSERT INTO failures_fts(rowid, context, action_tried, what_happened, lesson)
                VALUES (NEW.id, NEW.context, NEW.action_tried, NEW.what_happened, NEW.lesson);
            END;
        "#)?;
        Ok(())
    }

    // ========== Task Lifecycle ==========

    /// Start a new task, returns task ID
    pub fn start_task(&self, goal: &str, criteria: &[String]) -> SqliteResult<String> {
        let id = Uuid::new_v4().to_string();
        let criteria_json = serde_json::to_string(criteria).unwrap_or_else(|_| "[]".into());
        self.conn.execute(
            "INSERT INTO tasks (id, goal, criteria, status, started_at) VALUES (?1, ?2, ?3, 'running', ?4)",
            params![id, goal, criteria_json, Utc::now().to_rfc3339()],
        )?;
        Ok(id)
    }

    /// End a task
    pub fn end_task(&self, task_id: &str, status: &str, error: Option<&str>, screen_state: Option<&str>) -> SqliteResult<()> {
        let action_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM actions WHERE task_id = ?1",
            [task_id], |r| r.get(0),
        ).unwrap_or(0);

        self.conn.execute(
            "UPDATE tasks SET status = ?1, ended_at = ?2, error = ?3, total_actions = ?4, final_screen_state = ?5 WHERE id = ?6",
            params![status, Utc::now().to_rfc3339(), error, action_count, screen_state, task_id],
        )?;
        Ok(())
    }

    // ========== Action Recording ==========

    /// Record an action before execution (returns action row id)
    pub fn record_action_start(
        &self,
        task_id: &str,
        step_num: usize,
        intent: &str,
        action_type: &str,
        target_desc: Option<&str>,
        target_x: Option<i32>,
        target_y: Option<i32>,
        text_input: Option<&str>,
        keys_input: Option<&str>,
        confidence: f32,
        expected_result: &str,
        screen_before: &str,
    ) -> SqliteResult<i64> {
        self.conn.execute(
            "INSERT INTO actions (task_id, step_num, intent, action_type, target_desc, target_x, target_y,
             text_input, keys_input, confidence, expected_result, screen_before)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                task_id, step_num as i64, intent, action_type,
                target_desc, target_x, target_y,
                text_input, keys_input, confidence, expected_result,
                screen_before
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Update action after execution
    pub fn record_action_result(
        &self,
        action_id: i64,
        exec_success: bool,
        exec_error: Option<&str>,
        exec_duration_ms: u64,
        screen_after: &str,
        screen_changed: bool,
        expected_achieved: bool,
    ) -> SqliteResult<()> {
        self.conn.execute(
            "UPDATE actions SET executed = 1, exec_success = ?1, exec_error = ?2, exec_duration_ms = ?3,
             screen_after = ?4, screen_changed = ?5, expected_achieved = ?6
             WHERE id = ?7",
            params![
                exec_success as i32, exec_error, exec_duration_ms as i64,
                screen_after, screen_changed as i32, expected_achieved as i32,
                action_id
            ],
        )?;
        Ok(())
    }

    // ========== Failure Tracking ==========

    /// Record a failed approach
    pub fn record_failure(
        &self,
        task_id: Option<&str>,
        context: &str,
        action_tried: &str,
        what_happened: &str,
        lesson: &str,
    ) -> SqliteResult<()> {
        // Check if we've seen this exact pattern before
        let existing: Option<i64> = self.conn.query_row(
            "SELECT id FROM failures WHERE context = ?1 AND action_tried = ?2",
            params![context, action_tried],
            |r| r.get(0),
        ).ok();

        if let Some(id) = existing {
            // Reinforce - bump times_seen
            self.conn.execute(
                "UPDATE failures SET times_seen = times_seen + 1, what_happened = ?1, lesson = ?2 WHERE id = ?3",
                params![what_happened, lesson, id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO failures (task_id, context, action_tried, what_happened, lesson)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![task_id, context, action_tried, what_happened, lesson],
            )?;
        }
        Ok(())
    }

    /// Get relevant failures for current context (to inject into planner prompt)
    pub fn get_relevant_failures(&self, context: &str, limit: usize) -> SqliteResult<Vec<FailureRecord>> {
        // First try FTS match
        let mut failures = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT f.context, f.action_tried, f.what_happened, f.lesson, f.times_seen
             FROM failures f
             JOIN failures_fts fts ON f.id = fts.rowid
             WHERE failures_fts MATCH ?1
             ORDER BY f.times_seen DESC
             LIMIT ?2"
        ) {
            if let Ok(rows) = stmt.query_map(params![context, limit as i64], |row| {
                Ok(FailureRecord {
                    context: row.get(0)?,
                    action_tried: row.get(1)?,
                    what_happened: row.get(2)?,
                    lesson: row.get(3)?,
                    times_seen: row.get(4)?,
                })
            }) {
                for r in rows.flatten() {
                    failures.push(r);
                }
            }
        }

        // If FTS didn't find enough, do a simple LIKE search
        if failures.len() < limit {
            let remaining = limit - failures.len();
            if let Ok(mut stmt) = self.conn.prepare(
                "SELECT context, action_tried, what_happened, lesson, times_seen
                 FROM failures
                 WHERE context LIKE ?1
                 ORDER BY times_seen DESC
                 LIMIT ?2"
            ) {
                let pattern = format!("%{}%", context.split_whitespace().next().unwrap_or(context));
                if let Ok(rows) = stmt.query_map(params![pattern, remaining as i64], |row| {
                    Ok(FailureRecord {
                        context: row.get(0)?,
                        action_tried: row.get(1)?,
                        what_happened: row.get(2)?,
                        lesson: row.get(3)?,
                        times_seen: row.get(4)?,
                    })
                }) {
                    for r in rows.flatten() {
                        failures.push(r);
                    }
                }
            }
        }

        Ok(failures)
    }

    // ========== Sub-steps ==========

    /// Set sub-steps for a task (planner decomposes goal into steps)
    pub fn set_substeps(&self, task_id: &str, steps: &[&str]) -> SqliteResult<()> {
        for (i, step) in steps.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO substeps (task_id, step_order, description) VALUES (?1, ?2, ?3)",
                params![task_id, i as i64, step],
            )?;
        }
        Ok(())
    }

    /// Get current sub-steps for a task
    pub fn get_substeps(&self, task_id: &str) -> SqliteResult<Vec<SubStep>> {
        let mut stmt = self.conn.prepare(
            "SELECT step_order, description, status FROM substeps WHERE task_id = ?1 ORDER BY step_order"
        )?;
        let mut steps = Vec::new();
        let rows = stmt.query_map([task_id], |row| {
            Ok(SubStep {
                order: row.get(0)?,
                description: row.get(1)?,
                status: row.get(2)?,
            })
        })?;
        for s in rows.flatten() {
            steps.push(s);
        }
        Ok(steps)
    }

    /// Mark a sub-step as done/failed
    pub fn update_substep(&self, task_id: &str, step_order: usize, status: &str) -> SqliteResult<()> {
        let completed_at = if status == "done" { Some(Utc::now().to_rfc3339()) } else { None };
        self.conn.execute(
            "UPDATE substeps SET status = ?1, completed_at = ?2 WHERE task_id = ?3 AND step_order = ?4",
            params![status, completed_at, task_id, step_order as i64],
        )?;
        Ok(())
    }

    // ========== Context for Planner ==========

    /// Build context string for the planner prompt - recent actions + failures
    pub fn get_planner_context(&self, task_id: &str, current_app: &str) -> SqliteResult<String> {
        let mut ctx = String::new();

        // Recent actions for this task
        let mut stmt = self.conn.prepare(
            "SELECT step_num, intent, action_type, exec_success, screen_changed, expected_achieved,
                    screen_before, screen_after, exec_error
             FROM actions WHERE task_id = ?1 ORDER BY step_num DESC LIMIT 5"
        )?;
        let actions: Vec<String> = stmt.query_map([task_id], |row| {
            let step: i64 = row.get(0)?;
            let intent: String = row.get(1)?;
            let action_type: String = row.get(2)?;
            let success: bool = row.get::<_, i32>(3)? != 0;
            let changed: bool = row.get::<_, i32>(4)? != 0;
            let achieved: bool = row.get::<_, i32>(5)? != 0;
            let before: String = row.get::<_, Option<String>>(6)?.unwrap_or_default();
            let after: String = row.get::<_, Option<String>>(7)?.unwrap_or_default();
            let error: Option<String> = row.get(8)?;

            let status_str = if !success {
                format!("FAILED: {}", error.unwrap_or_default())
            } else if !changed {
                "executed but SCREEN DID NOT CHANGE".into()
            } else if !achieved {
                format!("screen changed but expected result NOT seen (was: {})", after)
            } else {
                "SUCCESS".into()
            };

            Ok(format!("  Step {}: [{}] {} → {}", step, action_type, intent, status_str))
        })?.flatten().collect();

        if !actions.is_empty() {
            ctx.push_str("RECENT ACTIONS (newest first):\n");
            for a in &actions {
                ctx.push_str(a);
                ctx.push('\n');
            }
            ctx.push('\n');
        }

        // Relevant failures
        if let Ok(failures) = self.get_relevant_failures(current_app, 3) {
            if !failures.is_empty() {
                ctx.push_str("KNOWN PITFALLS (avoid these):\n");
                for f in &failures {
                    ctx.push_str(&format!("  - In {}: tried '{}' → {}. Instead: {}\n",
                        f.context, f.action_tried, f.what_happened, f.lesson));
                }
                ctx.push('\n');
            }
        }

        // Sub-steps if any
        if let Ok(steps) = self.get_substeps(task_id) {
            if !steps.is_empty() {
                ctx.push_str("TASK PLAN:\n");
                for s in &steps {
                    let marker = match s.status.as_str() {
                        "done" => "[x]",
                        "active" => "[>]",
                        "failed" => "[!]",
                        "skipped" => "[-]",
                        _ => "[ ]",
                    };
                    ctx.push_str(&format!("  {} {}\n", marker, s.description));
                }
                ctx.push('\n');
            }
        }

        Ok(ctx)
    }

    /// Get stats for display
    pub fn task_stats(&self) -> SqliteResult<TaskStats> {
        let total: i64 = self.conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
        let successes: i64 = self.conn.query_row("SELECT COUNT(*) FROM tasks WHERE status = 'success'", [], |r| r.get(0))?;
        let total_actions: i64 = self.conn.query_row("SELECT COUNT(*) FROM actions", [], |r| r.get(0))?;
        let failures: i64 = self.conn.query_row("SELECT COUNT(*) FROM failures", [], |r| r.get(0))?;

        Ok(TaskStats {
            total_tasks: total as usize,
            successful_tasks: successes as usize,
            total_actions: total_actions as usize,
            known_failures: failures as usize,
        })
    }
}

// ========== Data Structures ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    pub context: String,
    pub action_tried: String,
    pub what_happened: String,
    pub lesson: String,
    pub times_seen: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubStep {
    pub order: i64,
    pub description: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStats {
    pub total_tasks: usize,
    pub successful_tasks: usize,
    pub total_actions: usize,
    pub known_failures: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_lifecycle() {
        let db = VlaTaskDb::open_memory().unwrap();

        // Start task
        let task_id = db.start_task("Open Firefox", &["Firefox".into(), "browser".into()]).unwrap();

        // Record action
        let action_id = db.record_action_start(
            &task_id, 0, "Click Firefox icon", "click",
            Some("Firefox dock icon"), Some(20), Some(45),
            None, None, 0.9, "Firefox opens",
            "Desktop, GNOME Shell",
        ).unwrap();

        // Record result
        db.record_action_result(
            action_id, true, None, 150,
            "Firefox, New Tab", true, true,
        ).unwrap();

        // End task
        db.end_task(&task_id, "success", None, Some("Firefox, New Tab")).unwrap();

        let stats = db.task_stats().unwrap();
        assert_eq!(stats.total_tasks, 1);
        assert_eq!(stats.successful_tasks, 1);
        assert_eq!(stats.total_actions, 1);
    }

    #[test]
    fn test_failure_tracking() {
        let db = VlaTaskDb::open_memory().unwrap();

        db.record_failure(
            None,
            "Firefox browser",
            "key_press Super",
            "Opened GNOME Activities instead of staying in Firefox",
            "Use Ctrl+L for address bar or Ctrl+F for find, not Super key",
        ).unwrap();

        // Same failure reinforces
        db.record_failure(
            None,
            "Firefox browser",
            "key_press Super",
            "Opened GNOME Activities again",
            "Use Ctrl+L for address bar or Ctrl+F for find, not Super key",
        ).unwrap();

        let failures = db.get_relevant_failures("Firefox", 5).unwrap();
        assert!(!failures.is_empty());
        assert_eq!(failures[0].times_seen, 2);
    }

    #[test]
    fn test_substeps() {
        let db = VlaTaskDb::open_memory().unwrap();
        let task_id = db.start_task("Navigate to repo", &[]).unwrap();

        db.set_substeps(&task_id, &[
            "Focus Firefox address bar with Ctrl+L",
            "Type github.com/G-TechSD/ganesha-ai",
            "Press Enter to navigate",
            "Verify page loaded",
        ]).unwrap();

        let steps = db.get_substeps(&task_id).unwrap();
        assert_eq!(steps.len(), 4);
        assert_eq!(steps[0].status, "pending");

        db.update_substep(&task_id, 0, "done").unwrap();
        let steps = db.get_substeps(&task_id).unwrap();
        assert_eq!(steps[0].status, "done");
    }

    #[test]
    fn test_planner_context() {
        let db = VlaTaskDb::open_memory().unwrap();
        let task_id = db.start_task("Test goal", &[]).unwrap();

        // Add a failed action
        let aid = db.record_action_start(
            &task_id, 0, "Click link", "click",
            Some("ganesha-ai"), Some(350), Some(250),
            None, None, 0.8, "repo page opens",
            "GitHub profile",
        ).unwrap();
        db.record_action_result(aid, true, None, 150, "GitHub profile", false, false).unwrap();

        // Add failure pattern
        db.record_failure(None, "GitHub", "click at estimated coords",
            "Click missed target", "Use Ctrl+L and type URL directly").unwrap();

        let ctx = db.get_planner_context(&task_id, "GitHub").unwrap();
        assert!(ctx.contains("SCREEN DID NOT CHANGE"));
        assert!(ctx.contains("KNOWN PITFALLS"));
    }
}
