//! Temporal Memory - SpacetimeDB-backed screen activity log
//!
//! Instead of relying on LLM context (which gets summarized/lost),
//! we persist all screen activity to a temporal database:
//!
//! - What was on screen at any point
//! - Every action taken and result
//! - Zone states over time
//! - Goal progress history
//!
//! Query patterns:
//! - "What was on screen 30 seconds ago?"
//! - "What actions have we tried for this goal?"
//! - "When did this element last change?"
//! - "What's the pattern of failures?"

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ═══════════════════════════════════════════════════════════════════════════════
// TEMPORAL RECORDS (SpacetimeDB table schemas)
// ═══════════════════════════════════════════════════════════════════════════════

/// Screen state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSnapshot {
    /// Unique ID
    pub id: u64,
    /// Unix timestamp (ms)
    pub timestamp_ms: u64,
    /// URL or app identifier
    pub context: String,
    /// Page/window title
    pub title: String,
    /// Screenshot hash (for motion detection)
    pub screen_hash: u64,
    /// Active zones that had content
    pub active_zones: Vec<String>,
    /// Vision model's description
    pub vision_description: String,
    /// Markdown content (truncated)
    pub markdown_summary: String,
    /// Detected anomalies (popups, errors, etc)
    pub anomalies: Vec<String>,
}

/// Action taken by the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    pub id: u64,
    pub timestamp_ms: u64,
    /// Related screen snapshot ID
    pub snapshot_id: u64,
    /// Action type: "SEARCH_EBAY", "CLICK", "SCROLL", etc
    pub action_type: String,
    /// Action target/parameter
    pub target: String,
    /// Did it succeed?
    pub success: bool,
    /// What the ant reported
    pub ant_result: String,
    /// Did eagle verify it?
    pub eagle_verified: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Time to execute (ms)
    pub duration_ms: u64,
}

/// Goal progress tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProgress {
    pub id: u64,
    pub timestamp_ms: u64,
    /// The goal text
    pub goal: String,
    /// Extracted keywords
    pub keywords: Vec<String>,
    /// Progress 0.0-1.0
    pub progress: f32,
    /// Current step number
    pub step: u32,
    /// Related snapshot ID
    pub snapshot_id: u64,
    /// Status: "in_progress", "achieved", "failed", "stuck"
    pub status: String,
}

/// Zone state history (NVR-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneState {
    pub id: u64,
    pub timestamp_ms: u64,
    /// Zone identifier
    pub zone_id: String,
    /// Content hash
    pub content_hash: u64,
    /// Brief description of zone content
    pub content_desc: String,
    /// Has this zone changed since last check?
    pub changed: bool,
    /// How long has this zone been stable? (ms)
    pub stable_duration_ms: u64,
}

/// Obstacle encountered and how it was handled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObstacleRecord {
    pub id: u64,
    pub timestamp_ms: u64,
    /// Type: "cookie_consent", "modal", "captcha", "error"
    pub obstacle_type: String,
    /// Selector or description
    pub identifier: String,
    /// How was it handled?
    pub resolution: String,
    /// Did resolution work?
    pub resolved: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// IN-MEMORY TEMPORAL STORE (before SpacetimeDB integration)
// ═══════════════════════════════════════════════════════════════════════════════

/// Temporal memory store - can be backed by SpacetimeDB
pub struct TemporalMemory {
    /// Screen snapshots (ring buffer, keeps last N)
    snapshots: RwLock<VecDeque<ScreenSnapshot>>,
    /// Action history
    actions: RwLock<VecDeque<ActionRecord>>,
    /// Goal progress timeline
    goals: RwLock<VecDeque<GoalProgress>>,
    /// Zone states
    zones: RwLock<VecDeque<ZoneState>>,
    /// Obstacles encountered
    obstacles: RwLock<VecDeque<ObstacleRecord>>,
    /// Auto-increment ID
    next_id: RwLock<u64>,
    /// Max entries to keep in memory
    max_entries: usize,
    /// When memory was started
    start_time: Instant,
}

impl TemporalMemory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            snapshots: RwLock::new(VecDeque::with_capacity(max_entries)),
            actions: RwLock::new(VecDeque::with_capacity(max_entries)),
            goals: RwLock::new(VecDeque::with_capacity(max_entries)),
            zones: RwLock::new(VecDeque::with_capacity(max_entries * 10)),
            obstacles: RwLock::new(VecDeque::with_capacity(max_entries)),
            next_id: RwLock::new(1),
            max_entries,
            start_time: Instant::now(),
        }
    }

    fn next_id(&self) -> u64 {
        let mut id = self.next_id.write().unwrap();
        let current = *id;
        *id += 1;
        current
    }

    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // WRITE OPERATIONS
    // ═══════════════════════════════════════════════════════════════════════════

    /// Record a screen snapshot
    pub fn record_snapshot(
        &self,
        context: &str,
        title: &str,
        screen_hash: u64,
        active_zones: Vec<String>,
        vision_desc: &str,
        markdown: &str,
        anomalies: Vec<String>,
    ) -> u64 {
        let id = self.next_id();
        let snapshot = ScreenSnapshot {
            id,
            timestamp_ms: self.now_ms(),
            context: context.to_string(),
            title: title.to_string(),
            screen_hash,
            active_zones,
            vision_description: vision_desc.to_string(),
            markdown_summary: markdown.chars().take(1000).collect(),
            anomalies,
        };

        let mut snapshots = self.snapshots.write().unwrap();
        if snapshots.len() >= self.max_entries {
            snapshots.pop_front();
        }
        snapshots.push_back(snapshot);
        id
    }

    /// Record an action
    pub fn record_action(
        &self,
        snapshot_id: u64,
        action_type: &str,
        target: &str,
        success: bool,
        ant_result: &str,
        eagle_verified: bool,
        error: Option<&str>,
        duration_ms: u64,
    ) -> u64 {
        let id = self.next_id();
        let action = ActionRecord {
            id,
            timestamp_ms: self.now_ms(),
            snapshot_id,
            action_type: action_type.to_string(),
            target: target.to_string(),
            success,
            ant_result: ant_result.to_string(),
            eagle_verified,
            error: error.map(|s| s.to_string()),
            duration_ms,
        };

        let mut actions = self.actions.write().unwrap();
        if actions.len() >= self.max_entries {
            actions.pop_front();
        }
        actions.push_back(action);
        id
    }

    /// Record goal progress
    pub fn record_goal_progress(
        &self,
        goal: &str,
        keywords: Vec<String>,
        progress: f32,
        step: u32,
        snapshot_id: u64,
        status: &str,
    ) -> u64 {
        let id = self.next_id();
        let record = GoalProgress {
            id,
            timestamp_ms: self.now_ms(),
            goal: goal.to_string(),
            keywords,
            progress,
            step,
            snapshot_id,
            status: status.to_string(),
        };

        let mut goals = self.goals.write().unwrap();
        if goals.len() >= self.max_entries {
            goals.pop_front();
        }
        goals.push_back(record);
        id
    }

    /// Record zone state
    pub fn record_zone_state(
        &self,
        zone_id: &str,
        content_hash: u64,
        content_desc: &str,
        changed: bool,
        stable_duration_ms: u64,
    ) {
        let id = self.next_id();
        let state = ZoneState {
            id,
            timestamp_ms: self.now_ms(),
            zone_id: zone_id.to_string(),
            content_hash,
            content_desc: content_desc.to_string(),
            changed,
            stable_duration_ms,
        };

        let mut zones = self.zones.write().unwrap();
        if zones.len() >= self.max_entries * 10 {
            zones.pop_front();
        }
        zones.push_back(state);
    }

    /// Record obstacle
    pub fn record_obstacle(
        &self,
        obstacle_type: &str,
        identifier: &str,
        resolution: &str,
        resolved: bool,
    ) -> u64 {
        let id = self.next_id();
        let record = ObstacleRecord {
            id,
            timestamp_ms: self.now_ms(),
            obstacle_type: obstacle_type.to_string(),
            identifier: identifier.to_string(),
            resolution: resolution.to_string(),
            resolved,
        };

        let mut obstacles = self.obstacles.write().unwrap();
        if obstacles.len() >= self.max_entries {
            obstacles.pop_front();
        }
        obstacles.push_back(record);
        id
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // QUERY OPERATIONS
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get snapshot from N seconds ago
    pub fn snapshot_from_past(&self, seconds_ago: u64) -> Option<ScreenSnapshot> {
        let target_time = self.now_ms() - (seconds_ago * 1000);
        let snapshots = self.snapshots.read().unwrap();

        snapshots.iter()
            .rev()
            .find(|s| s.timestamp_ms <= target_time)
            .cloned()
    }

    /// Get last N snapshots
    pub fn recent_snapshots(&self, n: usize) -> Vec<ScreenSnapshot> {
        let snapshots = self.snapshots.read().unwrap();
        snapshots.iter().rev().take(n).cloned().collect()
    }

    /// Get all actions for current goal
    pub fn actions_for_goal(&self, goal: &str) -> Vec<ActionRecord> {
        let actions = self.actions.read().unwrap();
        let goals = self.goals.read().unwrap();

        // Find when this goal started
        let goal_start = goals.iter()
            .find(|g| g.goal == goal && g.step == 1)
            .map(|g| g.timestamp_ms)
            .unwrap_or(0);

        actions.iter()
            .filter(|a| a.timestamp_ms >= goal_start)
            .cloned()
            .collect()
    }

    /// Check if we've tried this action before (loop detection)
    pub fn has_tried_action(&self, action_type: &str, target: &str, within_secs: u64) -> bool {
        let cutoff = self.now_ms() - (within_secs * 1000);
        let actions = self.actions.read().unwrap();

        actions.iter().any(|a|
            a.timestamp_ms >= cutoff &&
            a.action_type == action_type &&
            a.target == target
        )
    }

    /// Get failed actions (for avoiding repeated failures)
    pub fn recent_failures(&self, n: usize) -> Vec<ActionRecord> {
        let actions = self.actions.read().unwrap();
        actions.iter()
            .rev()
            .filter(|a| !a.success)
            .take(n)
            .cloned()
            .collect()
    }

    /// When did a zone last change?
    pub fn zone_last_changed(&self, zone_id: &str) -> Option<u64> {
        let zones = self.zones.read().unwrap();
        zones.iter()
            .rev()
            .find(|z| z.zone_id == zone_id && z.changed)
            .map(|z| z.timestamp_ms)
    }

    /// Get obstacles we've encountered on this site
    pub fn obstacles_for_context(&self, context: &str) -> Vec<ObstacleRecord> {
        let obstacles = self.obstacles.read().unwrap();
        let snapshots = self.snapshots.read().unwrap();

        // Find snapshot IDs for this context
        let context_lower = context.to_lowercase();

        obstacles.iter()
            .filter(|o| {
                // Check if obstacle was encountered in this context
                snapshots.iter()
                    .any(|s| s.context.to_lowercase().contains(&context_lower) &&
                         s.timestamp_ms.abs_diff(o.timestamp_ms) < 5000)
            })
            .cloned()
            .collect()
    }

    /// Get goal progress history
    pub fn goal_history(&self, goal: &str) -> Vec<GoalProgress> {
        let goals = self.goals.read().unwrap();
        goals.iter()
            .filter(|g| g.goal == goal)
            .cloned()
            .collect()
    }

    /// Check if we're stuck (no progress in N steps)
    pub fn is_stuck(&self, goal: &str, threshold_steps: u32) -> bool {
        let history = self.goal_history(goal);
        if history.len() < threshold_steps as usize {
            return false;
        }

        let recent: Vec<_> = history.iter().rev().take(threshold_steps as usize).collect();
        let progress_range = recent.iter().map(|g| g.progress).fold(0.0f32, |a, b| a.max(b))
            - recent.iter().map(|g| g.progress).fold(1.0f32, |a, b| a.min(b));

        progress_range < 0.1 // Less than 10% progress change = stuck
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // CONTEXT GENERATION (for LLM)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Generate context summary for LLM (replaces unreliable long-horizon context)
    pub fn generate_context(&self, goal: &str, max_chars: usize) -> String {
        let mut context = String::new();

        // Recent screen state
        if let Some(snap) = self.recent_snapshots(1).first() {
            context.push_str(&format!(
                "CURRENT: {} | {}\n",
                snap.title, snap.context
            ));
        }

        // What was happening 30 seconds ago
        if let Some(past) = self.snapshot_from_past(30) {
            context.push_str(&format!(
                "30s AGO: {} | {}\n",
                past.title, past.context
            ));
        }

        // Recent actions
        let actions = self.actions_for_goal(goal);
        if !actions.is_empty() {
            context.push_str("\nACTION HISTORY:\n");
            for action in actions.iter().rev().take(5) {
                let status = if action.success { "✓" } else { "✗" };
                context.push_str(&format!(
                    "  {} {} {} ({})\n",
                    status, action.action_type, action.target,
                    if action.eagle_verified { "verified" } else { "unverified" }
                ));
            }
        }

        // Recent failures (important for avoiding loops)
        let failures = self.recent_failures(3);
        if !failures.is_empty() {
            context.push_str("\nRECENT FAILURES:\n");
            for f in &failures {
                context.push_str(&format!(
                    "  {} {} - {}\n",
                    f.action_type, f.target, f.error.as_deref().unwrap_or("unknown")
                ));
            }
        }

        // Progress
        let history = self.goal_history(goal);
        if let Some(latest) = history.last() {
            context.push_str(&format!(
                "\nPROGRESS: {:.0}% (step {})\n",
                latest.progress * 100.0, latest.step
            ));

            if self.is_stuck(goal, 3) {
                context.push_str("⚠️ STUCK: No progress in last 3 steps\n");
            }
        }

        // Truncate if too long
        if context.len() > max_chars {
            context.truncate(max_chars - 20);
            context.push_str("\n[truncated]\n");
        }

        context
    }

    /// Get memory stats
    pub fn stats(&self) -> String {
        let snapshots = self.snapshots.read().unwrap().len();
        let actions = self.actions.read().unwrap().len();
        let goals = self.goals.read().unwrap().len();
        let zones = self.zones.read().unwrap().len();
        let obstacles = self.obstacles.read().unwrap().len();

        format!(
            "Memory: {} snapshots, {} actions, {} goals, {} zones, {} obstacles | Uptime: {}s",
            snapshots, actions, goals, zones, obstacles,
            self.start_time.elapsed().as_secs()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SPACETIMEDB INTEGRATION (stub - implement when SpacetimeDB is added)
// ═══════════════════════════════════════════════════════════════════════════════

/// SpacetimeDB-backed persistent memory
/// TODO: Implement when adding spacetimedb dependency
pub struct PersistentMemory {
    // spacetimedb_client: SpacetimeDBClient,
    memory: TemporalMemory,
}

impl PersistentMemory {
    pub fn new(_db_url: &str) -> Self {
        // TODO: Connect to SpacetimeDB
        Self {
            memory: TemporalMemory::new(1000),
        }
    }

    /// Sync in-memory state to SpacetimeDB
    pub async fn sync(&self) -> Result<(), String> {
        // TODO: Push records to SpacetimeDB
        Ok(())
    }

    /// Query historical data from SpacetimeDB
    pub async fn query_history(
        &self,
        _start_time: u64,
        _end_time: u64,
    ) -> Result<Vec<ScreenSnapshot>, String> {
        // TODO: Query SpacetimeDB
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_memory() {
        let memory = TemporalMemory::new(100);

        // Record some activity
        let snap_id = memory.record_snapshot(
            "https://ebay.com",
            "eBay - Search",
            12345,
            vec!["listings".into()],
            "eBay search results page",
            "## Search Results\n- Item 1\n- Item 2",
            vec![],
        );

        memory.record_action(
            snap_id,
            "SEARCH_EBAY",
            "vintage synth",
            true,
            "Searched eBay",
            true,
            None,
            500,
        );

        memory.record_goal_progress(
            "search ebay for vintage synth",
            vec!["vintage".into(), "synth".into()],
            0.5,
            1,
            snap_id,
            "in_progress",
        );

        // Query
        assert!(!memory.is_stuck("search ebay for vintage synth", 3));
        assert!(memory.recent_snapshots(1).len() == 1);

        // Context generation
        let ctx = memory.generate_context("search ebay for vintage synth", 1000);
        assert!(ctx.contains("eBay"));
        println!("{}", ctx);
    }
}
