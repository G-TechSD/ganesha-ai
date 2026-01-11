"""
Ganesha Privileged Daemon Client

Used by the unprivileged Ganesha CLI to communicate with the
privileged daemon for elevated operations.

Falls back to direct execution if daemon is not available.
"""

import asyncio
import json
import os
import socket
import time
import uuid
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Optional, Tuple

from .access_control import AccessController, AccessPolicy, load_policy


SOCKET_PATH = "/var/run/ganesha/privileged.sock"


@dataclass
class PrivilegedResult:
    """Result from privileged execution."""
    success: bool
    output: str
    error: str
    exit_code: int
    risk_level: str
    execution_time_ms: int
    used_daemon: bool  # True if ran via daemon, False if direct


class PrivilegedClient:
    """
    Client for communicating with the privileged daemon.

    Usage:
        client = PrivilegedClient()

        if await client.is_daemon_available():
            result = await client.execute("apt update")
        else:
            # Fall back to direct execution (may fail without sudo)
            result = await client.execute_direct("apt update")
    """

    def __init__(self, socket_path: str = SOCKET_PATH):
        self.socket_path = socket_path
        self._policy: Optional[AccessPolicy] = None
        self._controller: Optional[AccessController] = None

    async def is_daemon_available(self) -> bool:
        """Check if the privileged daemon is running and accessible."""
        if not Path(self.socket_path).exists():
            return False

        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(self.socket_path),
                timeout=2,
            )
            writer.close()
            await writer.wait_closed()
            return True
        except Exception:
            return False

    def is_daemon_available_sync(self) -> bool:
        """Synchronous check for daemon availability."""
        if not Path(self.socket_path).exists():
            return False

        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.settimeout(2)
            sock.connect(self.socket_path)
            sock.close()
            return True
        except Exception:
            return False

    async def execute(
        self,
        command: str,
        working_dir: str = "/tmp",
        timeout: int = 60,
    ) -> PrivilegedResult:
        """
        Execute a command via the privileged daemon.

        If daemon is not available, attempts direct execution.
        """
        if await self.is_daemon_available():
            return await self._execute_via_daemon(command, working_dir, timeout)
        else:
            return await self._execute_direct(command, working_dir, timeout)

    async def _execute_via_daemon(
        self,
        command: str,
        working_dir: str,
        timeout: int,
    ) -> PrivilegedResult:
        """Execute via the privileged daemon."""
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(self.socket_path),
                timeout=5,
            )

            # Send request
            request = {
                "command": command,
                "working_dir": working_dir,
                "timeout": timeout,
                "request_id": str(uuid.uuid4())[:8],
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
            }

            writer.write(json.dumps(request).encode())
            await writer.drain()

            # Receive response
            data = await asyncio.wait_for(reader.read(1024 * 1024), timeout=timeout + 5)
            response = json.loads(data.decode())

            writer.close()
            await writer.wait_closed()

            return PrivilegedResult(
                success=response["success"],
                output=response["output"],
                error=response["error"],
                exit_code=response["exit_code"],
                risk_level=response["risk_level"],
                execution_time_ms=response["execution_time_ms"],
                used_daemon=True,
            )

        except asyncio.TimeoutError:
            return PrivilegedResult(
                success=False,
                output="",
                error="Timeout connecting to privileged daemon",
                exit_code=-1,
                risk_level="unknown",
                execution_time_ms=0,
                used_daemon=True,
            )
        except Exception as e:
            return PrivilegedResult(
                success=False,
                output="",
                error=f"Daemon communication error: {e}",
                exit_code=-1,
                risk_level="unknown",
                execution_time_ms=0,
                used_daemon=True,
            )

    async def _execute_direct(
        self,
        command: str,
        working_dir: str,
        timeout: int,
    ) -> PrivilegedResult:
        """Execute directly without daemon (may fail if needs sudo)."""
        start_time = time.time()

        # Check local policy if available
        if self._controller is None:
            try:
                self._policy = load_policy()
                self._controller = AccessController(self._policy)
            except Exception:
                pass

        risk_level = "unknown"
        if self._controller:
            _, risk_level, _ = self._controller.check_command(command)

        try:
            process = await asyncio.create_subprocess_shell(
                command,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=working_dir,
            )

            try:
                stdout, stderr = await asyncio.wait_for(
                    process.communicate(),
                    timeout=timeout,
                )
            except asyncio.TimeoutError:
                process.kill()
                await process.wait()
                return PrivilegedResult(
                    success=False,
                    output="",
                    error=f"Command timed out after {timeout}s",
                    exit_code=-1,
                    risk_level=risk_level,
                    execution_time_ms=int((time.time() - start_time) * 1000),
                    used_daemon=False,
                )

            return PrivilegedResult(
                success=process.returncode == 0,
                output=stdout.decode(errors="replace"),
                error=stderr.decode(errors="replace"),
                exit_code=process.returncode,
                risk_level=risk_level,
                execution_time_ms=int((time.time() - start_time) * 1000),
                used_daemon=False,
            )

        except Exception as e:
            return PrivilegedResult(
                success=False,
                output="",
                error=str(e),
                exit_code=-1,
                risk_level=risk_level,
                execution_time_ms=int((time.time() - start_time) * 1000),
                used_daemon=False,
            )

    def get_daemon_status(self) -> dict:
        """Get daemon status information."""
        available = self.is_daemon_available_sync()

        return {
            "available": available,
            "socket_path": self.socket_path,
            "socket_exists": Path(self.socket_path).exists(),
        }


# ═══════════════════════════════════════════════════════════════════════════
# CONVENIENCE FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════

_client: Optional[PrivilegedClient] = None


def get_client() -> PrivilegedClient:
    """Get singleton client instance."""
    global _client
    if _client is None:
        _client = PrivilegedClient()
    return _client


async def privileged_execute(
    command: str,
    working_dir: str = "/tmp",
    timeout: int = 60,
) -> PrivilegedResult:
    """Execute a privileged command."""
    return await get_client().execute(command, working_dir, timeout)


def daemon_available() -> bool:
    """Check if daemon is available."""
    return get_client().is_daemon_available_sync()
