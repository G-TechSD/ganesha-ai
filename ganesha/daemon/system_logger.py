"""
Ganesha System Logger

Writes audit events to OS-level logs for security and accountability.
These logs are harder to tamper with than application logs.

Linux:
    - syslog (/var/log/syslog, /var/log/messages)
    - journald (systemd journal)
    - Filter with: journalctl -t ganesha
                   grep GANESHA /var/log/syslog

Windows:
    - Windows Event Log (Event Viewer)
    - Custom event source: "Ganesha"
    - Filter in Event Viewer: Applications and Services Logs → Ganesha

Event IDs:
    1000-1099: Informational (command executed, daemon start/stop)
    1100-1199: Warnings (high-risk command approved, config change)
    1200-1299: Errors (command denied, access violation)
    1300-1399: Critical (manipulation detected, security breach attempt)
"""

import json
import os
import platform
import socket
import sys
from dataclasses import dataclass
from datetime import datetime
from enum import IntEnum
from pathlib import Path
from typing import Any, Dict, Optional


# ═══════════════════════════════════════════════════════════════════════════
# EVENT IDS (for filtering in logs)
# ═══════════════════════════════════════════════════════════════════════════

class GaneshaEventID(IntEnum):
    """Event IDs for Ganesha audit events."""
    # Informational (1000-1099)
    DAEMON_START = 1000
    DAEMON_STOP = 1001
    COMMAND_EXECUTED = 1010
    COMMAND_PLANNED = 1011
    SESSION_START = 1020
    SESSION_END = 1021
    CONFIG_LOADED = 1030

    # Warnings (1100-1199)
    HIGH_RISK_APPROVED = 1100
    CONFIG_CHANGED = 1110
    ACCESS_LEVEL_CHANGED = 1111
    WHITELIST_MODIFIED = 1120
    BLACKLIST_MODIFIED = 1121
    ELEVATED_ACCESS_USED = 1130

    # Errors (1200-1299)
    COMMAND_DENIED = 1200
    ACCESS_VIOLATION = 1201
    AUTHENTICATION_FAILED = 1210
    INVALID_REQUEST = 1220
    EXECUTION_FAILED = 1230
    TIMEOUT = 1240

    # Critical (1300-1399)
    MANIPULATION_DETECTED = 1300
    SELF_INVOCATION_BLOCKED = 1301
    SECURITY_BREACH_ATTEMPT = 1310
    CRITICAL_COMMAND_BLOCKED = 1320
    LOG_TAMPERING_ATTEMPT = 1330


class LogLevel(IntEnum):
    """Log severity levels."""
    DEBUG = 0
    INFO = 1
    WARNING = 2
    ERROR = 3
    CRITICAL = 4


# ═══════════════════════════════════════════════════════════════════════════
# LOG MESSAGE FORMAT
# ═══════════════════════════════════════════════════════════════════════════

@dataclass
class GaneshaLogEvent:
    """Structured log event for Ganesha."""
    event_id: GaneshaEventID
    level: LogLevel
    message: str
    user: str = ""
    command: str = ""
    risk_level: str = ""
    allowed: Optional[bool] = None
    reason: str = ""
    session_id: str = ""
    extra: Optional[Dict[str, Any]] = None

    def to_syslog_message(self) -> str:
        """Format for syslog (human-readable, parseable)."""
        parts = [
            f"GANESHA[{self.event_id}]",
            f"level={self.level.name}",
        ]

        if self.user:
            parts.append(f"user={self.user}")
        if self.command:
            # Truncate and escape command for log safety
            cmd = self.command[:200].replace('"', '\\"').replace('\n', ' ')
            parts.append(f'cmd="{cmd}"')
        if self.risk_level:
            parts.append(f"risk={self.risk_level}")
        if self.allowed is not None:
            parts.append(f"allowed={'yes' if self.allowed else 'no'}")
        if self.reason:
            parts.append(f'reason="{self.reason[:100]}"')
        if self.session_id:
            parts.append(f"session={self.session_id[:8]}")

        parts.append(f"msg={self.message}")

        return " ".join(parts)

    def to_json(self) -> str:
        """Format as JSON for structured logging."""
        data = {
            "timestamp": datetime.now().isoformat(),
            "source": "ganesha",
            "event_id": int(self.event_id),
            "event_name": self.event_id.name,
            "level": self.level.name,
            "message": self.message,
            "hostname": socket.gethostname(),
        }

        if self.user:
            data["user"] = self.user
        if self.command:
            data["command"] = self.command[:500]
        if self.risk_level:
            data["risk_level"] = self.risk_level
        if self.allowed is not None:
            data["allowed"] = self.allowed
        if self.reason:
            data["reason"] = self.reason
        if self.session_id:
            data["session_id"] = self.session_id
        if self.extra:
            data["extra"] = self.extra

        return json.dumps(data)


# ═══════════════════════════════════════════════════════════════════════════
# LINUX SYSLOG LOGGER
# ═══════════════════════════════════════════════════════════════════════════

class LinuxSyslogLogger:
    """
    Logs to Linux syslog/journald.

    Uses LOG_LOCAL0 facility for easy filtering.
    Identifier: "ganesha"

    Filter commands:
        journalctl -t ganesha                    # All ganesha events
        journalctl -t ganesha -p warning         # Warnings and above
        journalctl -t ganesha --since "1 hour ago"
        grep "GANESHA\[1200\]" /var/log/syslog   # Command denied events
        grep "GANESHA\[1300\]" /var/log/syslog   # Manipulation attempts
    """

    # Syslog priority levels
    LOG_EMERG = 0
    LOG_ALERT = 1
    LOG_CRIT = 2
    LOG_ERR = 3
    LOG_WARNING = 4
    LOG_NOTICE = 5
    LOG_INFO = 6
    LOG_DEBUG = 7

    # Facility: LOG_LOCAL0 = 16 (128 in priority calculation)
    LOG_LOCAL0 = 16

    LEVEL_MAP = {
        LogLevel.DEBUG: LOG_DEBUG,
        LogLevel.INFO: LOG_INFO,
        LogLevel.WARNING: LOG_WARNING,
        LogLevel.ERROR: LOG_ERR,
        LogLevel.CRITICAL: LOG_CRIT,
    }

    def __init__(self, ident: str = "ganesha"):
        self.ident = ident
        self._syslog = None

        try:
            import syslog
            self._syslog = syslog
            syslog.openlog(ident, syslog.LOG_PID, syslog.LOG_LOCAL0)
        except ImportError:
            pass

    def log(self, event: GaneshaLogEvent):
        """Log event to syslog."""
        if self._syslog is None:
            return

        priority = self.LEVEL_MAP.get(event.level, self.LOG_INFO)
        message = event.to_syslog_message()

        self._syslog.syslog(priority, message)

    def close(self):
        """Close syslog connection."""
        if self._syslog:
            self._syslog.closelog()


# ═══════════════════════════════════════════════════════════════════════════
# LINUX JOURNALD LOGGER
# ═══════════════════════════════════════════════════════════════════════════

class JournaldLogger:
    """
    Logs to systemd journald with structured fields.

    Filter commands:
        journalctl -t ganesha
        journalctl SYSLOG_IDENTIFIER=ganesha
        journalctl GANESHA_EVENT_ID=1200        # Command denied
        journalctl GANESHA_RISK_LEVEL=critical  # Critical risk
        journalctl GANESHA_USER=bill            # Specific user
    """

    LEVEL_MAP = {
        LogLevel.DEBUG: 7,    # LOG_DEBUG
        LogLevel.INFO: 6,     # LOG_INFO
        LogLevel.WARNING: 4,  # LOG_WARNING
        LogLevel.ERROR: 3,    # LOG_ERR
        LogLevel.CRITICAL: 2, # LOG_CRIT
    }

    def __init__(self):
        self._journal = None
        try:
            from systemd import journal
            self._journal = journal
        except ImportError:
            pass

    def log(self, event: GaneshaLogEvent):
        """Log event to journald with structured fields."""
        if self._journal is None:
            return

        # Build structured fields for filtering
        fields = {
            "SYSLOG_IDENTIFIER": "ganesha",
            "PRIORITY": str(self.LEVEL_MAP.get(event.level, 6)),
            "GANESHA_EVENT_ID": str(int(event.event_id)),
            "GANESHA_EVENT_NAME": event.event_id.name,
            "GANESHA_LEVEL": event.level.name,
        }

        if event.user:
            fields["GANESHA_USER"] = event.user
        if event.command:
            fields["GANESHA_COMMAND"] = event.command[:500]
        if event.risk_level:
            fields["GANESHA_RISK_LEVEL"] = event.risk_level
        if event.allowed is not None:
            fields["GANESHA_ALLOWED"] = "yes" if event.allowed else "no"
        if event.session_id:
            fields["GANESHA_SESSION_ID"] = event.session_id

        self._journal.send(event.to_syslog_message(), **fields)


# ═══════════════════════════════════════════════════════════════════════════
# WINDOWS EVENT LOG LOGGER
# ═══════════════════════════════════════════════════════════════════════════

class WindowsEventLogger:
    """
    Logs to Windows Event Log.

    Creates custom event source "Ganesha" in Application log.

    View in Event Viewer:
        - Open Event Viewer
        - Windows Logs → Application
        - Filter by Source: "Ganesha"

    Or use PowerShell:
        Get-EventLog -LogName Application -Source Ganesha
        Get-EventLog -LogName Application -Source Ganesha -EntryType Error
        Get-EventLog -LogName Application -Source Ganesha | Where-Object {$_.EventID -eq 1200}
    """

    # Windows event types
    EVENTLOG_ERROR_TYPE = 0x0001
    EVENTLOG_WARNING_TYPE = 0x0002
    EVENTLOG_INFORMATION_TYPE = 0x0004

    LEVEL_MAP = {
        LogLevel.DEBUG: EVENTLOG_INFORMATION_TYPE,
        LogLevel.INFO: EVENTLOG_INFORMATION_TYPE,
        LogLevel.WARNING: EVENTLOG_WARNING_TYPE,
        LogLevel.ERROR: EVENTLOG_ERROR_TYPE,
        LogLevel.CRITICAL: EVENTLOG_ERROR_TYPE,
    }

    def __init__(self, source: str = "Ganesha"):
        self.source = source
        self._win32 = None
        self._handle = None

        try:
            import win32evtlogutil
            import win32evtlog
            import win32security
            import win32con

            self._win32 = {
                "util": win32evtlogutil,
                "log": win32evtlog,
                "security": win32security,
                "con": win32con,
            }

            # Try to register event source (requires admin first time)
            try:
                win32evtlogutil.AddSourceToRegistry(
                    source,
                    msgDLL=None,
                    eventLogType="Application",
                )
            except Exception:
                pass  # May already exist or need admin

        except ImportError:
            pass

    def log(self, event: GaneshaLogEvent):
        """Log event to Windows Event Log."""
        if self._win32 is None:
            return

        event_type = self.LEVEL_MAP.get(event.level, self.EVENTLOG_INFORMATION_TYPE)

        # Format message with all details
        message = event.to_syslog_message()

        try:
            self._win32["util"].ReportEvent(
                self.source,
                int(event.event_id),
                eventCategory=0,
                eventType=event_type,
                strings=[message],
                data=event.to_json().encode("utf-8"),
            )
        except Exception as e:
            # Fallback: print to stderr
            print(f"EventLog error: {e}", file=sys.stderr)


# ═══════════════════════════════════════════════════════════════════════════
# UNIFIED SYSTEM LOGGER
# ═══════════════════════════════════════════════════════════════════════════

class SystemLogger:
    """
    Unified system logger that writes to the appropriate OS log.

    Automatically detects platform and uses:
    - Linux: syslog + journald
    - Windows: Event Log
    - Fallback: stderr

    All events are tagged with "ganesha" for easy filtering.
    """

    def __init__(self):
        self.platform = platform.system().lower()
        self._loggers = []

        if self.platform == "linux":
            self._loggers.append(LinuxSyslogLogger())
            self._loggers.append(JournaldLogger())
        elif self.platform == "windows":
            self._loggers.append(WindowsEventLogger())

        # Always have a fallback
        self._file_fallback = Path("/var/log/ganesha/system.log")
        if self.platform == "windows":
            self._file_fallback = Path.home() / ".ganesha" / "system.log"

    def log(self, event: GaneshaLogEvent):
        """Log event to all available system loggers."""
        logged = False

        for logger in self._loggers:
            try:
                logger.log(event)
                logged = True
            except Exception:
                pass

        # Fallback to file if system logging failed
        if not logged:
            self._log_to_file(event)

    def _log_to_file(self, event: GaneshaLogEvent):
        """Fallback logging to file."""
        try:
            self._file_fallback.parent.mkdir(parents=True, exist_ok=True)
            with open(self._file_fallback, "a") as f:
                f.write(event.to_json() + "\n")
        except Exception:
            # Last resort: stderr
            print(event.to_syslog_message(), file=sys.stderr)

    # ═══════════════════════════════════════════════════════════════════════
    # CONVENIENCE METHODS
    # ═══════════════════════════════════════════════════════════════════════

    def command_executed(
        self,
        user: str,
        command: str,
        risk_level: str,
        session_id: str = "",
    ):
        """Log a successfully executed command."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.COMMAND_EXECUTED,
            level=LogLevel.INFO,
            message="Command executed successfully",
            user=user,
            command=command,
            risk_level=risk_level,
            allowed=True,
            session_id=session_id,
        ))

    def command_denied(
        self,
        user: str,
        command: str,
        reason: str,
        risk_level: str = "unknown",
    ):
        """Log a denied command."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.COMMAND_DENIED,
            level=LogLevel.WARNING,
            message="Command denied by access control",
            user=user,
            command=command,
            risk_level=risk_level,
            allowed=False,
            reason=reason,
        ))

    def manipulation_detected(
        self,
        user: str,
        command: str,
        indicator: str,
    ):
        """Log a manipulation attempt."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.MANIPULATION_DETECTED,
            level=LogLevel.CRITICAL,
            message="SECURITY: Manipulation attempt detected",
            user=user,
            command=command,
            risk_level="critical",
            allowed=False,
            reason=f"Matched manipulation indicator: {indicator}",
        ))

    def self_invocation_blocked(
        self,
        user: str,
        command: str,
    ):
        """Log a blocked self-invocation attempt."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.SELF_INVOCATION_BLOCKED,
            level=LogLevel.CRITICAL,
            message="SECURITY: Self-invocation with bypass flags blocked",
            user=user,
            command=command,
            risk_level="critical",
            allowed=False,
            reason="Ganesha cannot call itself with --auto or similar flags",
        ))

    def high_risk_approved(
        self,
        user: str,
        command: str,
        risk_level: str,
    ):
        """Log approval of a high-risk command."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.HIGH_RISK_APPROVED,
            level=LogLevel.WARNING,
            message="High-risk command approved by user",
            user=user,
            command=command,
            risk_level=risk_level,
            allowed=True,
        ))

    def daemon_start(self, access_level: str):
        """Log daemon startup."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.DAEMON_START,
            level=LogLevel.INFO,
            message=f"Ganesha daemon started with access level: {access_level}",
            extra={"access_level": access_level},
        ))

    def daemon_stop(self):
        """Log daemon shutdown."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.DAEMON_STOP,
            level=LogLevel.INFO,
            message="Ganesha daemon stopped",
        ))

    def config_changed(
        self,
        user: str,
        change_type: str,
        old_value: str,
        new_value: str,
    ):
        """Log configuration change."""
        self.log(GaneshaLogEvent(
            event_id=GaneshaEventID.CONFIG_CHANGED,
            level=LogLevel.WARNING,
            message=f"Configuration changed: {change_type}",
            user=user,
            extra={
                "change_type": change_type,
                "old_value": old_value,
                "new_value": new_value,
            },
        ))


# ═══════════════════════════════════════════════════════════════════════════
# SINGLETON
# ═══════════════════════════════════════════════════════════════════════════

_system_logger: Optional[SystemLogger] = None


def get_system_logger() -> SystemLogger:
    """Get the singleton system logger."""
    global _system_logger
    if _system_logger is None:
        _system_logger = SystemLogger()
    return _system_logger


# Convenience function
def log_to_system(event: GaneshaLogEvent):
    """Log an event to the system log."""
    get_system_logger().log(event)
