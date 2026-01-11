"""
Ganesha Privileged Daemon

A separate process that runs with elevated privileges (root/sudo).
Communicates with unprivileged Ganesha CLI via Unix socket.

This architecture provides:
1. Privilege separation - main CLI doesn't need sudo
2. Access control - configurable whitelists/blacklists
3. Audit logging - all privileged commands are logged
4. Rate limiting - prevents abuse

Run as root:
    sudo python -m ganesha.daemon.privileged

Or install as systemd service:
    sudo ganesha-daemon install
"""

import asyncio
import json
import os
import pwd
import grp
import signal
import subprocess
import sys
import time
from dataclasses import dataclass, asdict
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, Optional

from .access_control import (
    AccessController,
    AccessLevel,
    AccessPolicy,
    load_policy,
    save_policy,
)


# ═══════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════

SOCKET_PATH = "/var/run/ganesha/privileged.sock"
PID_FILE = "/var/run/ganesha/daemon.pid"
LOG_DIR = Path("/var/log/ganesha")
CONFIG_DIR = Path("/etc/ganesha")
AUDIT_LOG = LOG_DIR / "audit.log"

# Socket permissions - only ganesha group can connect
SOCKET_MODE = 0o660
SOCKET_GROUP = "ganesha"  # Create this group and add users


@dataclass
class CommandRequest:
    """Request from unprivileged client."""
    command: str
    working_dir: str = "/tmp"
    timeout: int = 60
    request_id: str = ""
    user: str = ""
    timestamp: str = ""


@dataclass
class CommandResponse:
    """Response to unprivileged client."""
    success: bool
    output: str
    error: str
    exit_code: int
    risk_level: str
    request_id: str
    execution_time_ms: int


# ═══════════════════════════════════════════════════════════════════════════
# AUDIT LOGGING
# ═══════════════════════════════════════════════════════════════════════════

class AuditLogger:
    """Logs all privileged operations for security audit."""

    def __init__(self, log_path: Path = AUDIT_LOG):
        self.log_path = log_path
        self.log_path.parent.mkdir(parents=True, exist_ok=True)

    def log(
        self,
        action: str,
        user: str,
        command: str,
        allowed: bool,
        risk_level: str,
        reason: str,
        result: Optional[str] = None,
    ):
        """Log an audit event."""
        entry = {
            "timestamp": datetime.now().isoformat(),
            "action": action,
            "user": user,
            "command": command[:500],  # Truncate long commands
            "allowed": allowed,
            "risk_level": risk_level,
            "reason": reason,
            "result": result[:200] if result else None,
        }

        with open(self.log_path, "a") as f:
            f.write(json.dumps(entry) + "\n")


# ═══════════════════════════════════════════════════════════════════════════
# PRIVILEGED DAEMON
# ═══════════════════════════════════════════════════════════════════════════

class PrivilegedDaemon:
    """
    The privileged daemon that executes commands as root.

    Security model:
    1. Runs as root (or via sudo)
    2. Listens on Unix socket with restricted permissions
    3. Only users in 'ganesha' group can connect
    4. All commands validated against access policy
    5. All operations logged to audit log
    """

    def __init__(self, policy: Optional[AccessPolicy] = None):
        self.policy = policy or load_policy()
        self.controller = AccessController(self.policy)
        self.audit = AuditLogger()
        self.server: Optional[asyncio.Server] = None
        self._running = False

    async def start(self):
        """Start the daemon."""
        if os.geteuid() != 0:
            print("ERROR: Privileged daemon must run as root", file=sys.stderr)
            print("Run with: sudo python -m ganesha.daemon.privileged", file=sys.stderr)
            sys.exit(1)

        # Prepare socket directory
        socket_dir = Path(SOCKET_PATH).parent
        socket_dir.mkdir(parents=True, exist_ok=True)

        # Remove stale socket
        if Path(SOCKET_PATH).exists():
            Path(SOCKET_PATH).unlink()

        # Create and configure socket
        self.server = await asyncio.start_unix_server(
            self._handle_client,
            path=SOCKET_PATH,
        )

        # Set socket permissions
        self._set_socket_permissions()

        # Write PID file
        Path(PID_FILE).parent.mkdir(parents=True, exist_ok=True)
        Path(PID_FILE).write_text(str(os.getpid()))

        self._running = True
        self.audit.log(
            action="daemon_start",
            user="root",
            command="",
            allowed=True,
            risk_level="low",
            reason=f"Daemon started with policy: {self.policy.level.value}",
        )

        print(f"Ganesha Privileged Daemon started")
        print(f"  Socket: {SOCKET_PATH}")
        print(f"  Policy: {self.policy.level.value}")
        print(f"  Audit log: {AUDIT_LOG}")

        # Handle signals
        loop = asyncio.get_event_loop()
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.add_signal_handler(sig, lambda: asyncio.create_task(self.stop()))

        async with self.server:
            await self.server.serve_forever()

    async def stop(self):
        """Stop the daemon."""
        self._running = False
        if self.server:
            self.server.close()
            await self.server.wait_closed()

        # Cleanup
        if Path(SOCKET_PATH).exists():
            Path(SOCKET_PATH).unlink()
        if Path(PID_FILE).exists():
            Path(PID_FILE).unlink()

        self.audit.log(
            action="daemon_stop",
            user="root",
            command="",
            allowed=True,
            risk_level="low",
            reason="Daemon stopped gracefully",
        )
        print("Daemon stopped")

    def _set_socket_permissions(self):
        """Set socket permissions to restrict access."""
        try:
            # Try to set group ownership to 'ganesha' group
            try:
                gid = grp.getgrnam(SOCKET_GROUP).gr_gid
                os.chown(SOCKET_PATH, 0, gid)
            except KeyError:
                # Group doesn't exist, use root group
                print(f"Warning: Group '{SOCKET_GROUP}' not found. Using root group.")
                print(f"Create group with: sudo groupadd {SOCKET_GROUP}")
                print(f"Add user with: sudo usermod -aG {SOCKET_GROUP} $USER")

            os.chmod(SOCKET_PATH, SOCKET_MODE)
        except Exception as e:
            print(f"Warning: Could not set socket permissions: {e}")

    async def _handle_client(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ):
        """Handle a client connection."""
        try:
            # Read request
            data = await asyncio.wait_for(reader.read(65536), timeout=30)
            if not data:
                return

            request_data = json.loads(data.decode())
            request = CommandRequest(**request_data)

            # Get peer credentials (user making request)
            sock = writer.get_extra_info("socket")
            try:
                creds = sock.getsockopt(
                    1,  # SOL_SOCKET
                    17,  # SO_PEERCRED
                    12,  # sizeof(struct ucred)
                )
                import struct
                pid, uid, gid = struct.unpack("iii", creds)
                username = pwd.getpwuid(uid).pw_name
            except Exception:
                username = "unknown"

            request.user = username

            # Process request
            response = await self._process_request(request)

            # Send response
            writer.write(json.dumps(asdict(response)).encode())
            await writer.drain()

        except asyncio.TimeoutError:
            error_response = CommandResponse(
                success=False,
                output="",
                error="Request timeout",
                exit_code=-1,
                risk_level="low",
                request_id=getattr(request, "request_id", ""),
                execution_time_ms=0,
            )
            writer.write(json.dumps(asdict(error_response)).encode())
            await writer.drain()
        except Exception as e:
            error_response = CommandResponse(
                success=False,
                output="",
                error=str(e),
                exit_code=-1,
                risk_level="low",
                request_id="",
                execution_time_ms=0,
            )
            try:
                writer.write(json.dumps(asdict(error_response)).encode())
                await writer.drain()
            except Exception:
                pass
        finally:
            writer.close()
            try:
                await writer.wait_closed()
            except Exception:
                pass

    async def _process_request(self, request: CommandRequest) -> CommandResponse:
        """Process a command request."""
        start_time = time.time()

        # Check access
        allowed, risk_level, reason = self.controller.check_command(request.command)

        if not allowed:
            self.audit.log(
                action="command_denied",
                user=request.user,
                command=request.command,
                allowed=False,
                risk_level=risk_level,
                reason=reason,
            )
            return CommandResponse(
                success=False,
                output="",
                error=f"Access denied: {reason}",
                exit_code=-1,
                risk_level=risk_level,
                request_id=request.request_id,
                execution_time_ms=0,
            )

        # Execute command
        try:
            process = await asyncio.create_subprocess_shell(
                request.command,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=request.working_dir,
            )

            try:
                stdout, stderr = await asyncio.wait_for(
                    process.communicate(),
                    timeout=min(request.timeout, self.policy.max_execution_time),
                )
            except asyncio.TimeoutError:
                process.kill()
                await process.wait()
                return CommandResponse(
                    success=False,
                    output="",
                    error=f"Command timed out after {request.timeout}s",
                    exit_code=-1,
                    risk_level=risk_level,
                    request_id=request.request_id,
                    execution_time_ms=int((time.time() - start_time) * 1000),
                )

            execution_time_ms = int((time.time() - start_time) * 1000)
            success = process.returncode == 0

            self.audit.log(
                action="command_executed",
                user=request.user,
                command=request.command,
                allowed=True,
                risk_level=risk_level,
                reason=reason,
                result=f"exit_code={process.returncode}",
            )

            return CommandResponse(
                success=success,
                output=stdout.decode(errors="replace"),
                error=stderr.decode(errors="replace"),
                exit_code=process.returncode,
                risk_level=risk_level,
                request_id=request.request_id,
                execution_time_ms=execution_time_ms,
            )

        except Exception as e:
            self.audit.log(
                action="command_error",
                user=request.user,
                command=request.command,
                allowed=True,
                risk_level=risk_level,
                reason=str(e),
            )
            return CommandResponse(
                success=False,
                output="",
                error=str(e),
                exit_code=-1,
                risk_level=risk_level,
                request_id=request.request_id,
                execution_time_ms=int((time.time() - start_time) * 1000),
            )


# ═══════════════════════════════════════════════════════════════════════════
# SYSTEMD SERVICE INSTALLATION
# ═══════════════════════════════════════════════════════════════════════════

SYSTEMD_SERVICE = """[Unit]
Description=Ganesha Privileged Daemon
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/python3 -m ganesha.daemon.privileged
Restart=always
RestartSec=5
User=root

# Security hardening
NoNewPrivileges=false
ProtectSystem=false
ProtectHome=false
PrivateTmp=false

[Install]
WantedBy=multi-user.target
"""


def install_service():
    """Install the daemon as a systemd service."""
    if os.geteuid() != 0:
        print("ERROR: Must run as root to install service")
        sys.exit(1)

    service_path = Path("/etc/systemd/system/ganesha-daemon.service")
    service_path.write_text(SYSTEMD_SERVICE)

    # Create ganesha group if it doesn't exist
    try:
        grp.getgrnam(SOCKET_GROUP)
    except KeyError:
        subprocess.run(["groupadd", SOCKET_GROUP], check=True)
        print(f"Created group: {SOCKET_GROUP}")

    # Reload systemd
    subprocess.run(["systemctl", "daemon-reload"], check=True)

    print(f"Service installed: {service_path}")
    print("To enable: sudo systemctl enable ganesha-daemon")
    print("To start: sudo systemctl start ganesha-daemon")
    print(f"Add users to daemon access: sudo usermod -aG {SOCKET_GROUP} USERNAME")


def uninstall_service():
    """Uninstall the systemd service."""
    if os.geteuid() != 0:
        print("ERROR: Must run as root to uninstall service")
        sys.exit(1)

    subprocess.run(["systemctl", "stop", "ganesha-daemon"], check=False)
    subprocess.run(["systemctl", "disable", "ganesha-daemon"], check=False)

    service_path = Path("/etc/systemd/system/ganesha-daemon.service")
    if service_path.exists():
        service_path.unlink()

    subprocess.run(["systemctl", "daemon-reload"], check=True)
    print("Service uninstalled")


# ═══════════════════════════════════════════════════════════════════════════
# CLI
# ═══════════════════════════════════════════════════════════════════════════

def print_banner():
    """Print daemon banner."""
    print("""
╔═══════════════════════════════════════════════════════════════╗
║           GANESHA PRIVILEGED DAEMON                           ║
║           The Remover of Obstacles                            ║
╠═══════════════════════════════════════════════════════════════╣
║  This daemon runs with root privileges.                       ║
║  All commands are logged to: /var/log/ganesha/audit.log       ║
╚═══════════════════════════════════════════════════════════════╝
""")


def main():
    """Entry point."""
    import argparse

    parser = argparse.ArgumentParser(description="Ganesha Privileged Daemon")
    parser.add_argument("command", nargs="?", default="run",
                        choices=["run", "install", "uninstall", "status"])
    parser.add_argument("--level", choices=["restricted", "standard", "elevated", "full_access"],
                        default="standard", help="Access level preset")
    parser.add_argument("--config", type=Path, help="Config file path")

    args = parser.parse_args()

    if args.command == "install":
        install_service()
    elif args.command == "uninstall":
        uninstall_service()
    elif args.command == "status":
        if Path(PID_FILE).exists():
            pid = Path(PID_FILE).read_text().strip()
            print(f"Daemon running (PID: {pid})")
        else:
            print("Daemon not running")
    else:
        print_banner()
        policy = load_policy(args.config)
        policy.level = AccessLevel(args.level)
        daemon = PrivilegedDaemon(policy)
        asyncio.run(daemon.start())


if __name__ == "__main__":
    main()
