//! Ganesha Privileged Daemon
//!
//! Runs with elevated privileges and handles privileged command execution.
//!
//! ```bash
//! sudo ganesha-daemon
//! sudo ganesha-daemon --level elevated
//! sudo ganesha-daemon install  # Install as system service
//! ```

use clap::{Parser, Subcommand};
use ganesha::core::access_control::{AccessLevel, AccessPolicy};
use ganesha::logging::SystemLogger;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

#[cfg(target_os = "linux")]
const SOCKET_PATH: &str = "/var/run/ganesha/privileged.sock";
#[cfg(target_os = "macos")]
const SOCKET_PATH: &str = "/var/run/ganesha/privileged.sock";
#[cfg(target_os = "windows")]
const SOCKET_PATH: &str = r"\\.\pipe\ganesha";

#[derive(Parser)]
#[command(name = "ganesha-daemon")]
#[command(about = "Ganesha Privileged Daemon")]
struct Args {
    /// Access level
    #[arg(long, default_value = "standard")]
    #[arg(value_parser = ["restricted", "standard", "elevated", "full_access"])]
    level: String,

    #[command(subcommand)]
    command: Option<DaemonCommand>,
}

#[derive(Subcommand)]
enum DaemonCommand {
    /// Install as system service
    Install,
    /// Uninstall system service
    Uninstall,
    /// Show daemon status
    Status,
}

fn print_banner() {
    println!(
        r#"
╔═══════════════════════════════════════════════════════════════╗
║           GANESHA PRIVILEGED DAEMON                           ║
║           The Remover of Obstacles                            ║
╠═══════════════════════════════════════════════════════════════╣
║  This daemon runs with elevated privileges.                   ║
║  All commands are logged to the system log.                   ║
╚═══════════════════════════════════════════════════════════════╝
"#
    );
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Handle subcommands
    if let Some(cmd) = args.command {
        match cmd {
            DaemonCommand::Install => {
                install_service();
                return;
            }
            DaemonCommand::Uninstall => {
                uninstall_service();
                return;
            }
            DaemonCommand::Status => {
                show_status();
                return;
            }
        }
    }

    // Check if running as root
    #[cfg(unix)]
    {
        if !nix::unistd::geteuid().is_root() {
            eprintln!("ERROR: Daemon must run as root");
            eprintln!("Run with: sudo ganesha-daemon");
            std::process::exit(1);
        }
    }

    print_banner();

    let level = match args.level.as_str() {
        "restricted" => AccessLevel::Restricted,
        "standard" => AccessLevel::Standard,
        "elevated" => AccessLevel::Elevated,
        "full_access" => AccessLevel::FullAccess,
        _ => AccessLevel::Standard,
    };

    let policy = AccessPolicy {
        level,
        ..Default::default()
    };

    println!("Socket: {}", SOCKET_PATH);
    println!("Access level: {:?}", policy.level);
    println!();

    let logger = SystemLogger::new();
    logger.daemon_start(&format!("{:?}", policy.level));

    // Run daemon
    if let Err(e) = run_daemon(policy, logger).await {
        eprintln!("Daemon error: {}", e);
        std::process::exit(1);
    }
}

async fn run_daemon(
    policy: AccessPolicy,
    logger: SystemLogger,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create socket directory
    let socket_path = PathBuf::from(SOCKET_PATH);
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove old socket
    let _ = std::fs::remove_file(&socket_path);

    // Bind listener
    #[cfg(unix)]
    let listener = UnixListener::bind(&socket_path)?;

    // Set permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))?;
    }

    println!("Daemon listening...");

    let controller = Arc::new(ganesha::core::access_control::AccessController::new(policy));
    let logger = Arc::new(logger);

    loop {
        let (stream, _) = listener.accept().await?;
        let controller = Arc::clone(&controller);
        let logger = Arc::clone(&logger);

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, &controller, &logger).await {
                eprintln!("Client error: {}", e);
            }
        });
    }
}

async fn handle_client(
    mut stream: UnixStream,
    controller: &ganesha::core::access_control::AccessController,
    logger: &SystemLogger,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    reader.read_line(&mut line).await?;

    // Parse request (simple JSON for now)
    #[derive(serde::Deserialize)]
    struct Request {
        command: String,
        working_dir: Option<String>,
        timeout: Option<u64>,
    }

    #[derive(serde::Serialize)]
    struct Response {
        success: bool,
        output: String,
        error: Option<String>,
        risk_level: String,
    }

    let request: Request = serde_json::from_str(&line)?;

    // Check access
    let check = controller.check_command(&request.command);

    if !check.allowed {
        logger.command_denied("daemon_client", &request.command, &check.reason);

        let response = Response {
            success: false,
            output: String::new(),
            error: Some(format!("Access denied: {}", check.reason)),
            risk_level: check.risk_level.to_string(),
        };

        let json = serde_json::to_string(&response)?;
        writer.write_all(json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        return Ok(());
    }

    // Execute command
    let output = tokio::process::Command::new("sh")
        .args(["-c", &request.command])
        .current_dir(request.working_dir.unwrap_or_else(|| "/tmp".into()))
        .output()
        .await?;

    let response = Response {
        success: output.status.success(),
        output: String::from_utf8_lossy(&output.stdout).to_string(),
        error: if output.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        },
        risk_level: check.risk_level.to_string(),
    };

    logger.command_executed(
        "daemon_client",
        &request.command,
        &check.risk_level.to_string(),
        "",
    );

    let json = serde_json::to_string(&response)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    Ok(())
}

fn install_service() {
    #[cfg(target_os = "linux")]
    {
        let service = r#"[Unit]
Description=Ganesha Privileged Daemon
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/ganesha-daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#;

        let path = "/etc/systemd/system/ganesha-daemon.service";
        if let Err(e) = std::fs::write(path, service) {
            eprintln!("Failed to write service file: {}", e);
            eprintln!("Run as root");
            return;
        }

        println!("Service installed: {}", path);
        println!("Run: sudo systemctl enable ganesha-daemon");
        println!("Run: sudo systemctl start ganesha-daemon");
    }

    #[cfg(target_os = "macos")]
    {
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.gtechsd.ganesha-daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/ganesha-daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
"#;

        let path = "/Library/LaunchDaemons/com.gtechsd.ganesha-daemon.plist";
        if let Err(e) = std::fs::write(path, plist) {
            eprintln!("Failed to write plist: {}", e);
            eprintln!("Run as root");
            return;
        }

        println!("Service installed: {}", path);
        println!("Run: sudo launchctl load {}", path);
    }

    #[cfg(target_os = "windows")]
    {
        println!("Windows service installation requires sc.exe");
        println!("Run: sc create GaneshaDaemon binPath= \"C:\\Program Files\\Ganesha\\ganesha-daemon.exe\"");
    }
}

fn uninstall_service() {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .args(["stop", "ganesha-daemon"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["disable", "ganesha-daemon"])
            .status();
        let _ = std::fs::remove_file("/etc/systemd/system/ganesha-daemon.service");
        println!("Service uninstalled");
    }

    #[cfg(target_os = "macos")]
    {
        let path = "/Library/LaunchDaemons/com.gtechsd.ganesha-daemon.plist";
        let _ = std::process::Command::new("launchctl")
            .args(["unload", path])
            .status();
        let _ = std::fs::remove_file(path);
        println!("Service uninstalled");
    }

    #[cfg(target_os = "windows")]
    {
        println!("Run: sc delete GaneshaDaemon");
    }
}

fn show_status() {
    let socket_exists = std::path::Path::new(SOCKET_PATH).exists();

    if socket_exists {
        println!("Daemon: RUNNING");
        println!("Socket: {}", SOCKET_PATH);
    } else {
        println!("Daemon: NOT RUNNING");
    }
}
