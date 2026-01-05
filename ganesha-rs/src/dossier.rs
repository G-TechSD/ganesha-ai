//! System Dossier - Complete situational awareness for AI control
//!
//! Like Task Manager but bespoke for AI needs:
//! - Running apps with window z-order
//! - Processes and resource usage
//! - Installed apps that can be launched
//! - System stats (RAM, CPU, network)
//! - OS details, screen resolution, time

use std::collections::HashMap;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Complete system state snapshot
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemDossier {
    /// Timestamp of this snapshot
    pub timestamp: u64,

    /// OS information
    pub os: OsInfo,

    /// Display/screen info
    pub display: DisplayInfo,

    /// Running windows with z-order (front to back)
    pub windows: Vec<WindowInfo>,

    /// Top processes by resource usage
    pub processes: Vec<ProcessInfo>,

    /// Installed launchable applications
    pub installed_apps: Vec<InstalledApp>,

    /// System resource stats
    pub resources: ResourceStats,

    /// Network interfaces and stats
    pub network: Vec<NetworkInterface>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OsInfo {
    pub name: String,           // "Ubuntu", "Arch Linux", "Windows 11"
    pub version: String,        // "24.04", "rolling", "23H2"
    pub kernel: String,         // "6.14.0-37-generic"
    pub hostname: String,
    pub username: String,
    pub desktop_env: String,    // "GNOME", "KDE", "Windows Explorer"
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    pub scale: f32,             // HiDPI scaling
    pub monitors: Vec<MonitorInfo>,
    pub active_monitor: usize,  // Which monitor has focus
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,                 // Position in virtual screen
    pub y: i32,
    pub primary: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    pub id: String,             // Window ID (X11 window id, HWND, etc)
    pub title: String,
    pub app_name: String,       // "Firefox", "Code", "Terminal"
    pub class: String,          // WM_CLASS or similar
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub z_order: u32,           // 0 = topmost
    pub is_focused: bool,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub workspace: u32,         // Virtual desktop number
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub cpu_percent: f32,
    pub mem_mb: f32,
    pub state: String,          // "running", "sleeping", "zombie"
    pub user: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstalledApp {
    pub name: String,
    pub exec: String,           // Command to launch
    pub icon: String,
    pub categories: Vec<String>, // "Browser", "Development", "Graphics"
    pub desktop_file: String,   // Path to .desktop file
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceStats {
    pub cpu_percent: f32,
    pub cpu_cores: u32,
    pub mem_total_mb: u64,
    pub mem_used_mb: u64,
    pub mem_available_mb: u64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub disk_read_mb_s: f32,
    pub disk_write_mb_s: f32,
    pub uptime_secs: u64,
    pub load_avg: [f32; 3],     // 1min, 5min, 15min
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_speed_kbps: f32,
    pub tx_speed_kbps: f32,
    pub is_up: bool,
}

impl SystemDossier {
    /// Collect complete system dossier (Linux implementation)
    #[cfg(target_os = "linux")]
    pub fn collect() -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Self {
            timestamp,
            os: collect_os_info()?,
            display: collect_display_info()?,
            windows: collect_windows()?,
            processes: collect_processes()?,
            installed_apps: collect_installed_apps()?,
            resources: collect_resources()?,
            network: collect_network()?,
        })
    }

    /// Get focused window
    pub fn focused_window(&self) -> Option<&WindowInfo> {
        self.windows.iter().find(|w| w.is_focused)
    }

    /// Get windows for a specific app
    pub fn windows_for_app(&self, app_name: &str) -> Vec<&WindowInfo> {
        self.windows
            .iter()
            .filter(|w| w.app_name.to_lowercase().contains(&app_name.to_lowercase()))
            .collect()
    }

    /// Check if an app is running
    pub fn is_app_running(&self, app_name: &str) -> bool {
        self.windows.iter().any(|w|
            w.app_name.to_lowercase().contains(&app_name.to_lowercase())
        )
    }

    /// Find app to launch by name
    pub fn find_app(&self, query: &str) -> Option<&InstalledApp> {
        let query_lower = query.to_lowercase();
        self.installed_apps.iter().find(|app|
            app.name.to_lowercase().contains(&query_lower)
        )
    }

    /// Summarize for LLM context (concise)
    pub fn summarize(&self) -> String {
        let focused = self.focused_window()
            .map(|w| format!("{} - {}", w.app_name, w.title))
            .unwrap_or_else(|| "None".into());

        let top_windows: Vec<String> = self.windows
            .iter()
            .filter(|w| !w.is_minimized)
            .take(5)
            .map(|w| format!("{}:{}", w.app_name, w.title.chars().take(30).collect::<String>()))
            .collect();

        format!(
            r#"SYSTEM DOSSIER:
OS: {} {} ({})
Display: {}x{} @ {:.1}x scale
Focused: {}
Windows: {:?}
CPU: {:.1}% | RAM: {}MB / {}MB ({:.0}%)
Uptime: {}h {}m"#,
            self.os.name, self.os.version, self.os.desktop_env,
            self.display.width, self.display.height, self.display.scale,
            focused,
            top_windows,
            self.resources.cpu_percent,
            self.resources.mem_used_mb, self.resources.mem_total_mb,
            (self.resources.mem_used_mb as f64 / self.resources.mem_total_mb as f64) * 100.0,
            self.resources.uptime_secs / 3600,
            (self.resources.uptime_secs % 3600) / 60
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LINUX COLLECTORS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "linux")]
fn collect_os_info() -> Result<OsInfo, String> {
    // Read /etc/os-release
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut name = "Linux".to_string();
    let mut version = "unknown".to_string();

    for line in os_release.lines() {
        if line.starts_with("NAME=") {
            name = line[5..].trim_matches('"').to_string();
        } else if line.starts_with("VERSION_ID=") {
            version = line[11..].trim_matches('"').to_string();
        }
    }

    // Kernel version
    let kernel = Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());

    // Hostname
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();

    // Username
    let username = std::env::var("USER").unwrap_or_else(|_| "unknown".into());

    // Desktop environment
    let desktop_env = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "unknown".into());

    Ok(OsInfo {
        name,
        version,
        kernel,
        hostname,
        username,
        desktop_env,
    })
}

#[cfg(target_os = "linux")]
fn collect_display_info() -> Result<DisplayInfo, String> {
    // Use xrandr to get display info
    let output = Command::new("xrandr")
        .arg("--query")
        .output()
        .map_err(|e| format!("xrandr failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut monitors = Vec::new();
    let mut primary_idx = 0;

    for line in stdout.lines() {
        if line.contains(" connected") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let name = parts.get(0).unwrap_or(&"unknown").to_string();
            let is_primary = line.contains("primary");

            // Parse resolution like "1920x1080+0+0"
            let mut width = 1920;
            let mut height = 1080;
            let mut x = 0;
            let mut y = 0;

            for part in &parts {
                if part.contains('x') && part.contains('+') {
                    let dims: Vec<&str> = part.split(|c| c == 'x' || c == '+').collect();
                    width = dims.get(0).and_then(|s| s.parse().ok()).unwrap_or(1920);
                    height = dims.get(1).and_then(|s| s.parse().ok()).unwrap_or(1080);
                    x = dims.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                    y = dims.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                }
            }

            if is_primary {
                primary_idx = monitors.len();
            }

            monitors.push(MonitorInfo {
                name,
                width,
                height,
                x,
                y,
                primary: is_primary,
            });
        }
    }

    // Get total virtual screen size
    let total_width = monitors.iter().map(|m| m.x as u32 + m.width).max().unwrap_or(1920);
    let total_height = monitors.iter().map(|m| m.y as u32 + m.height).max().unwrap_or(1080);

    // Check for HiDPI scale
    let scale = std::env::var("GDK_SCALE")
        .or_else(|_| std::env::var("QT_SCALE_FACTOR"))
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    Ok(DisplayInfo {
        width: total_width,
        height: total_height,
        scale,
        monitors,
        active_monitor: primary_idx,
    })
}

#[cfg(target_os = "linux")]
fn collect_windows() -> Result<Vec<WindowInfo>, String> {
    // Use wmctrl to list windows
    let output = Command::new("wmctrl")
        .args(["-l", "-p", "-G"])
        .output()
        .map_err(|e| format!("wmctrl failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut windows = Vec::new();

    // Get focused window ID
    let focused_id = Command::new("xdotool")
        .arg("getactivewindow")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    for (z_order, line) in stdout.lines().enumerate() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 {
            continue;
        }

        let id = parts[0].to_string();
        let workspace: u32 = parts[1].parse().unwrap_or(0);
        let pid: u32 = parts[2].parse().unwrap_or(0);
        let x: i32 = parts[3].parse().unwrap_or(0);
        let y: i32 = parts[4].parse().unwrap_or(0);
        let width: u32 = parts[5].parse().unwrap_or(0);
        let height: u32 = parts[6].parse().unwrap_or(0);
        let _hostname = parts[7];
        let title = parts[8..].join(" ");

        // Get WM_CLASS for app name
        let class_output = Command::new("xprop")
            .args(["-id", &id, "WM_CLASS"])
            .output()
            .ok();

        let (app_name, class) = if let Some(out) = class_output {
            let class_str = String::from_utf8_lossy(&out.stdout);
            if let Some(eq_pos) = class_str.find('=') {
                let classes: Vec<&str> = class_str[eq_pos + 1..]
                    .split(',')
                    .map(|s| s.trim().trim_matches('"'))
                    .collect();
                (
                    classes.get(1).unwrap_or(&"unknown").to_string(),
                    classes.get(0).unwrap_or(&"unknown").to_string(),
                )
            } else {
                ("unknown".into(), "unknown".into())
            }
        } else {
            ("unknown".into(), "unknown".into())
        };

        let is_focused = focused_id.as_ref().map(|f| {
            // Convert hex to decimal for comparison
            if let Ok(win_id) = u64::from_str_radix(id.trim_start_matches("0x"), 16) {
                f == &win_id.to_string()
            } else {
                false
            }
        }).unwrap_or(false);

        windows.push(WindowInfo {
            id,
            title,
            app_name,
            class,
            pid,
            x,
            y,
            width,
            height,
            z_order: z_order as u32,
            is_focused,
            is_minimized: workspace == u32::MAX, // -1 usually means minimized
            is_maximized: false, // Would need additional check
            workspace,
        });
    }

    Ok(windows)
}

#[cfg(target_os = "linux")]
fn collect_processes() -> Result<Vec<ProcessInfo>, String> {
    // Use ps to get top processes by CPU
    let output = Command::new("ps")
        .args(["aux", "--sort=-%cpu"])
        .output()
        .map_err(|e| format!("ps failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();

    for line in stdout.lines().skip(1).take(20) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 11 {
            continue;
        }

        processes.push(ProcessInfo {
            pid: parts[1].parse().unwrap_or(0),
            name: parts[10].to_string(),
            cmdline: parts[10..].join(" "),
            cpu_percent: parts[2].parse().unwrap_or(0.0),
            mem_mb: parts[3].parse::<f32>().unwrap_or(0.0) * 100.0, // RSS approximation
            state: parts[7].to_string(),
            user: parts[0].to_string(),
        });
    }

    Ok(processes)
}

#[cfg(target_os = "linux")]
fn collect_installed_apps() -> Result<Vec<InstalledApp>, String> {
    let mut apps = Vec::new();
    let app_dirs = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        &format!("{}/.local/share/applications", std::env::var("HOME").unwrap_or_default()),
    ];

    for dir in &app_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let mut name = String::new();
                        let mut exec = String::new();
                        let mut icon = String::new();
                        let mut categories = Vec::new();
                        let mut no_display = false;

                        for line in content.lines() {
                            if line.starts_with("Name=") && name.is_empty() {
                                name = line[5..].to_string();
                            } else if line.starts_with("Exec=") {
                                exec = line[5..].split_whitespace().next().unwrap_or("").to_string();
                            } else if line.starts_with("Icon=") {
                                icon = line[5..].to_string();
                            } else if line.starts_with("Categories=") {
                                categories = line[11..].split(';')
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string())
                                    .collect();
                            } else if line == "NoDisplay=true" {
                                no_display = true;
                            }
                        }

                        if !no_display && !name.is_empty() && !exec.is_empty() {
                            apps.push(InstalledApp {
                                name,
                                exec,
                                icon,
                                categories,
                                desktop_file: path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by name
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);

    Ok(apps)
}

#[cfg(target_os = "linux")]
fn collect_resources() -> Result<ResourceStats, String> {
    // Memory from /proc/meminfo
    let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut mem_total = 0u64;
    let mut mem_available = 0u64;
    let mut swap_total = 0u64;
    let mut swap_free = 0u64;

    for line in meminfo.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let value: u64 = parts[1].parse().unwrap_or(0);
            match parts[0] {
                "MemTotal:" => mem_total = value,
                "MemAvailable:" => mem_available = value,
                "SwapTotal:" => swap_total = value,
                "SwapFree:" => swap_free = value,
                _ => {}
            }
        }
    }

    // CPU from /proc/stat (simplified - just shows current load)
    let loadavg = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let load_parts: Vec<f32> = loadavg.split_whitespace()
        .take(3)
        .filter_map(|s| s.parse().ok())
        .collect();

    let cpu_cores = num_cpus();

    // Uptime
    let uptime_str = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    let uptime_secs: u64 = uptime_str.split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0);

    Ok(ResourceStats {
        cpu_percent: load_parts.get(0).copied().unwrap_or(0.0) / cpu_cores as f32 * 100.0,
        cpu_cores,
        mem_total_mb: mem_total / 1024,
        mem_used_mb: (mem_total - mem_available) / 1024,
        mem_available_mb: mem_available / 1024,
        swap_total_mb: swap_total / 1024,
        swap_used_mb: (swap_total - swap_free) / 1024,
        disk_read_mb_s: 0.0, // Would need iostat
        disk_write_mb_s: 0.0,
        uptime_secs,
        load_avg: [
            load_parts.get(0).copied().unwrap_or(0.0),
            load_parts.get(1).copied().unwrap_or(0.0),
            load_parts.get(2).copied().unwrap_or(0.0),
        ],
    })
}

#[cfg(target_os = "linux")]
fn num_cpus() -> u32 {
    std::fs::read_to_string("/proc/cpuinfo")
        .map(|s| s.matches("processor").count() as u32)
        .unwrap_or(1)
}

#[cfg(target_os = "linux")]
fn collect_network() -> Result<Vec<NetworkInterface>, String> {
    let mut interfaces = Vec::new();

    // Read /proc/net/dev for stats
    let net_dev = std::fs::read_to_string("/proc/net/dev").unwrap_or_default();

    for line in net_dev.lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        let name = parts[0].trim_end_matches(':').to_string();
        if name == "lo" {
            continue; // Skip loopback
        }

        let rx_bytes: u64 = parts[1].parse().unwrap_or(0);
        let tx_bytes: u64 = parts[9].parse().unwrap_or(0);

        // Get IP address
        let ip_output = Command::new("ip")
            .args(["addr", "show", &name])
            .output()
            .ok();

        let ip = ip_output
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.lines()
                    .find(|l| l.contains("inet ") && !l.contains("inet6"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .map(|s| s.split('/').next().unwrap_or("").to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let is_up = !ip.is_empty();

        interfaces.push(NetworkInterface {
            name,
            ip,
            rx_bytes,
            tx_bytes,
            rx_speed_kbps: 0.0, // Would need delta calculation
            tx_speed_kbps: 0.0,
            is_up,
        });
    }

    Ok(interfaces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_collect_dossier() {
        let dossier = SystemDossier::collect();
        assert!(dossier.is_ok());

        let d = dossier.unwrap();
        println!("{}", d.summarize());
        assert!(!d.os.name.is_empty());
    }
}
