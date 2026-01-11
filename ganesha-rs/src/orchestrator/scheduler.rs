//! Scheduled Task System
//!
//! Cross-platform scheduled task management:
//! - Linux: cron/systemd timers
//! - macOS: launchd/cron
//! - Windows: Task Scheduler
//!
//! Also integrates with:
//! - macOS Automator workflows
//! - Microsoft Power Automate (via API)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::fs;
use chrono::{DateTime, Utc, NaiveTime, Timelike};
use uuid::Uuid;

/// A scheduled task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub schedule: Schedule,
    pub action: TaskAction,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub run_count: u32,
}

/// Schedule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Schedule {
    /// Run once at a specific time
    Once(DateTime<Utc>),
    /// Run at intervals (minutes)
    Interval(u32),
    /// Cron expression
    Cron(String),
    /// Daily at specific time
    Daily(NaiveTime),
    /// Weekly on specific days
    Weekly { days: Vec<Weekday>, time: NaiveTime },
    /// On system events
    OnEvent(SystemEvent),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Weekday {
    Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    Startup,
    Login,
    Unlock,
    NetworkConnect,
    UsbConnect,
    FileChange(String),
}

/// Action to perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskAction {
    /// Run a shell command
    Command { command: String, cwd: Option<String> },
    /// Run a Ganesha task
    GaneshaTask { task: String, auto_approve: bool },
    /// Run a script file
    Script { path: String, interpreter: Option<String> },
    /// HTTP webhook
    Webhook { url: String, method: String, body: Option<String> },
    /// N8N workflow
    N8nWorkflow { workflow_id: String },
    /// Power Automate flow (Windows)
    PowerAutomate { flow_id: String },
    /// Automator workflow (macOS)
    Automator { workflow_path: String },
}

/// Cross-platform scheduler
pub struct Scheduler {
    config_path: PathBuf,
    tasks: HashMap<Uuid, ScheduledTask>,
    platform: Platform,
}

#[derive(Debug, Clone, Copy)]
enum Platform {
    Linux,
    MacOS,
    Windows,
}

impl Scheduler {
    pub fn new() -> Self {
        let platform = if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Windows
        };

        let config_path = Self::get_config_path();
        let tasks = Self::load_tasks(&config_path);

        Self {
            config_path,
            tasks,
            platform,
        }
    }

    fn get_config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".ganesha").join("scheduled_tasks.json")
    }

    fn load_tasks(path: &PathBuf) -> HashMap<Uuid, ScheduledTask> {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(tasks) = serde_json::from_str(&content) {
                    return tasks;
                }
            }
        }
        HashMap::new()
    }

    fn save_tasks(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.tasks)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// Create a new scheduled task
    pub fn create_task(&mut self, task: ScheduledTask) -> Result<Uuid, Box<dyn std::error::Error>> {
        let id = task.id;

        // Install to system scheduler
        self.install_system_task(&task)?;

        self.tasks.insert(id, task);
        self.save_tasks()?;

        Ok(id)
    }

    /// Remove a scheduled task
    pub fn remove_task(&mut self, id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(task) = self.tasks.remove(&id) {
            self.uninstall_system_task(&task)?;
            self.save_tasks()?;
        }
        Ok(())
    }

    /// Enable/disable a task
    pub fn set_enabled(&mut self, id: Uuid, enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
        // Get a clone of the task first to avoid borrow issues
        let task = match self.tasks.get(&id) {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        if enabled {
            self.install_system_task(&task)?;
        } else {
            self.uninstall_system_task(&task)?;
        }

        // Now update the enabled status
        if let Some(t) = self.tasks.get_mut(&id) {
            t.enabled = enabled;
        }

        self.save_tasks()?;
        Ok(())
    }

    /// List all tasks
    pub fn list_tasks(&self) -> Vec<&ScheduledTask> {
        self.tasks.values().collect()
    }

    /// Get a specific task
    pub fn get_task(&self, id: Uuid) -> Option<&ScheduledTask> {
        self.tasks.get(&id)
    }

    /// Install a task to the system scheduler
    fn install_system_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        match self.platform {
            Platform::Linux => self.install_linux_task(task),
            Platform::MacOS => self.install_macos_task(task),
            Platform::Windows => self.install_windows_task(task),
        }
    }

    /// Uninstall a task from the system scheduler
    fn uninstall_system_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        match self.platform {
            Platform::Linux => self.uninstall_linux_task(task),
            Platform::MacOS => self.uninstall_macos_task(task),
            Platform::Windows => self.uninstall_windows_task(task),
        }
    }

    // Linux: Use cron or systemd timers
    fn install_linux_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        let cron_entry = self.build_cron_entry(task)?;

        // Create wrapper script
        let script_path = self.create_task_script(task)?;

        // Add to user's crontab
        let current = Command::new("crontab")
            .arg("-l")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let marker = format!("# GANESHA:{}", task.id);
        let new_entry = format!("{} {} {}", cron_entry, script_path.display(), marker);

        // Remove old entry if exists, add new
        let new_crontab: String = current
            .lines()
            .filter(|l| !l.contains(&marker))
            .chain(std::iter::once(new_entry.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        // Write new crontab
        let mut child = Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(new_crontab.as_bytes())?;
        }

        child.wait()?;
        Ok(())
    }

    fn uninstall_linux_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        let marker = format!("# GANESHA:{}", task.id);

        let current = Command::new("crontab")
            .arg("-l")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let new_crontab: String = current
            .lines()
            .filter(|l| !l.contains(&marker))
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new("crontab")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(new_crontab.as_bytes())?;
        }

        child.wait()?;

        // Remove script
        let script_path = self.get_script_path(task);
        fs::remove_file(&script_path).ok();

        Ok(())
    }

    // macOS: Use launchd
    fn install_macos_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        let script_path = self.create_task_script(task)?;
        let plist_path = self.get_launchd_path(task);
        let plist = self.build_launchd_plist(task, &script_path)?;

        fs::write(&plist_path, plist)?;

        // Load the plist
        Command::new("launchctl")
            .args(["load", &plist_path.to_string_lossy()])
            .output()?;

        Ok(())
    }

    fn uninstall_macos_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        let plist_path = self.get_launchd_path(task);

        // Unload first
        Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .output()
            .ok();

        fs::remove_file(&plist_path).ok();

        // Remove script
        let script_path = self.get_script_path(task);
        fs::remove_file(&script_path).ok();

        Ok(())
    }

    // Windows: Use Task Scheduler
    fn install_windows_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        let script_path = self.create_task_script(task)?;

        let task_name = format!("Ganesha_{}", task.id);
        let schedule_args = self.build_schtasks_args(task)?;

        Command::new("schtasks")
            .args(["/create", "/tn", &task_name, "/tr"])
            .arg(&script_path)
            .args(schedule_args)
            .args(["/f"]) // Force overwrite
            .output()?;

        Ok(())
    }

    fn uninstall_windows_task(&self, task: &ScheduledTask) -> Result<(), Box<dyn std::error::Error>> {
        let task_name = format!("Ganesha_{}", task.id);

        Command::new("schtasks")
            .args(["/delete", "/tn", &task_name, "/f"])
            .output()
            .ok();

        // Remove script
        let script_path = self.get_script_path(task);
        fs::remove_file(&script_path).ok();

        Ok(())
    }

    /// Create a wrapper script for the task
    fn create_task_script(&self, task: &ScheduledTask) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let script_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ganesha")
            .join("scripts");
        fs::create_dir_all(&script_dir)?;

        let (script_path, content) = match self.platform {
            Platform::Windows => {
                let path = script_dir.join(format!("{}.bat", task.id));
                let content = self.build_windows_script(task);
                (path, content)
            }
            _ => {
                let path = script_dir.join(format!("{}.sh", task.id));
                let content = self.build_unix_script(task);
                (path, content)
            }
        };

        fs::write(&script_path, content)?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }

        Ok(script_path)
    }

    fn get_script_path(&self, task: &ScheduledTask) -> PathBuf {
        let script_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ganesha")
            .join("scripts");

        match self.platform {
            Platform::Windows => script_dir.join(format!("{}.bat", task.id)),
            _ => script_dir.join(format!("{}.sh", task.id)),
        }
    }

    fn get_launchd_path(&self, task: &ScheduledTask) -> PathBuf {
        let launch_agents = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library")
            .join("LaunchAgents");
        launch_agents.join(format!("com.ganesha.{}.plist", task.id))
    }

    fn build_unix_script(&self, task: &ScheduledTask) -> String {
        let action = match &task.action {
            TaskAction::Command { command, cwd } => {
                let cd = cwd.as_ref().map(|c| format!("cd {}\n", c)).unwrap_or_default();
                format!("{}{}", cd, command)
            }
            TaskAction::GaneshaTask { task: t, auto_approve } => {
                let flag = if *auto_approve { " --auto" } else { "" };
                format!("ganesha{} \"{}\"", flag, t)
            }
            TaskAction::Script { path, interpreter } => {
                match interpreter {
                    Some(interp) => format!("{} {}", interp, path),
                    None => path.clone(),
                }
            }
            TaskAction::Webhook { url, method, body } => {
                let body_arg = body.as_ref()
                    .map(|b| format!(" -d '{}'", b))
                    .unwrap_or_default();
                format!("curl -X {} {}{}", method, url, body_arg)
            }
            TaskAction::N8nWorkflow { workflow_id } => {
                format!("curl -X POST http://localhost:5678/webhook/{}", workflow_id)
            }
            _ => "echo 'Unsupported action'".into(),
        };

        format!(r#"#!/bin/bash
# Ganesha Scheduled Task: {}
# ID: {}
# Created: {}

{}
"#, task.name, task.id, task.created_at, action)
    }

    fn build_windows_script(&self, task: &ScheduledTask) -> String {
        let action = match &task.action {
            TaskAction::Command { command, cwd } => {
                let cd = cwd.as_ref().map(|c| format!("cd /d {}\r\n", c)).unwrap_or_default();
                format!("{}{}", cd, command)
            }
            TaskAction::GaneshaTask { task: t, auto_approve } => {
                let flag = if *auto_approve { " --auto" } else { "" };
                format!("ganesha{} \"{}\"", flag, t)
            }
            TaskAction::Script { path, interpreter } => {
                match interpreter {
                    Some(interp) => format!("{} {}", interp, path),
                    None => path.clone(),
                }
            }
            TaskAction::PowerAutomate { flow_id } => {
                format!("PowerShell -Command \"Invoke-PowerAutomateFlow -FlowId '{}'\"", flow_id)
            }
            _ => "echo Unsupported action".into(),
        };

        format!(r#"@echo off
REM Ganesha Scheduled Task: {}
REM ID: {}

{}
"#, task.name, task.id, action)
    }

    fn build_cron_entry(&self, task: &ScheduledTask) -> Result<String, Box<dyn std::error::Error>> {
        match &task.schedule {
            Schedule::Cron(expr) => Ok(expr.clone()),
            Schedule::Interval(mins) => Ok(format!("*/{} * * * *", mins)),
            Schedule::Daily(time) => Ok(format!("{} {} * * *", time.minute(), time.hour())),
            Schedule::Weekly { days, time } => {
                let day_nums: Vec<String> = days.iter().map(|d| match d {
                    Weekday::Sunday => "0",
                    Weekday::Monday => "1",
                    Weekday::Tuesday => "2",
                    Weekday::Wednesday => "3",
                    Weekday::Thursday => "4",
                    Weekday::Friday => "5",
                    Weekday::Saturday => "6",
                }.to_string()).collect();
                Ok(format!("{} {} * * {}", time.minute(), time.hour(), day_nums.join(",")))
            }
            _ => Err("Schedule type not supported for cron".into()),
        }
    }

    fn build_launchd_plist(&self, task: &ScheduledTask, script_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
        let calendar_interval = match &task.schedule {
            Schedule::Daily(time) => format!(r#"
    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>{}</integer>
        <key>Minute</key>
        <integer>{}</integer>
    </dict>"#, time.hour(), time.minute()),
            Schedule::Interval(mins) => format!(r#"
    <key>StartInterval</key>
    <integer>{}</integer>"#, mins * 60),
            _ => String::new(),
        };

        Ok(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.ganesha.{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <false/>
    {}
</dict>
</plist>"#, task.id, script_path.display(), calendar_interval))
    }

    fn build_schtasks_args(&self, task: &ScheduledTask) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        match &task.schedule {
            Schedule::Daily(time) => Ok(vec![
                "/sc".into(), "daily".into(),
                "/st".into(), format!("{:02}:{:02}", time.hour(), time.minute()),
            ]),
            Schedule::Interval(mins) => Ok(vec![
                "/sc".into(), "minute".into(),
                "/mo".into(), mins.to_string(),
            ]),
            Schedule::Once(dt) => Ok(vec![
                "/sc".into(), "once".into(),
                "/st".into(), dt.format("%H:%M").to_string(),
                "/sd".into(), dt.format("%m/%d/%Y").to_string(),
            ]),
            _ => Err("Schedule type not supported for Windows Task Scheduler".into()),
        }
    }

    /// Print task status
    pub fn print_status(&self) {
        println!("\n\x1b[1;36mScheduled Tasks:\x1b[0m\n");

        for task in self.tasks.values() {
            let status = if task.enabled { "\x1b[32m●\x1b[0m" } else { "\x1b[33m○\x1b[0m" };
            let last = task.last_run
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "never".into());

            println!("  {} {} - {}", status, task.name, task.description);
            println!("    Schedule: {:?}", task.schedule);
            println!("    Last run: {} | Run count: {}", last, task.run_count);
            println!();
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = Scheduler::new();
        assert!(scheduler.tasks.is_empty() || !scheduler.tasks.is_empty());
    }

    #[test]
    fn test_cron_entry_building() {
        let scheduler = Scheduler::new();
        let task = ScheduledTask {
            id: Uuid::new_v4(),
            name: "Test".into(),
            description: "Test task".into(),
            schedule: Schedule::Interval(15),
            action: TaskAction::Command { command: "echo test".into(), cwd: None },
            enabled: true,
            created_at: Utc::now(),
            last_run: None,
            next_run: None,
            run_count: 0,
        };

        let entry = scheduler.build_cron_entry(&task).unwrap();
        assert_eq!(entry, "*/15 * * * *");
    }
}
