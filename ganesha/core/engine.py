"""
Ganesha Core Execution Engine

The heart of Ganesha - handles task planning, execution, and iteration.
This is the engine that removes obstacles.

Design principles:
- Clean, minimal, no AI slop
- Async-first for performance
- Provider-agnostic (works with any LLM)
- Safe by default (user consent required)
- Observable (every action is logged)
"""

import asyncio
import json
import uuid
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import (
    Any,
    AsyncIterator,
    Callable,
    Dict,
    List,
    Optional,
    Protocol,
    TypeVar,
)


class TaskState(Enum):
    """State of a task in the execution pipeline."""
    PENDING = "pending"
    PLANNING = "planning"
    AWAITING_CONSENT = "awaiting_consent"
    EXECUTING = "executing"
    ITERATING = "iterating"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class ActionType(Enum):
    """Types of actions Ganesha can take."""
    SHELL_COMMAND = "shell_command"
    FILE_READ = "file_read"
    FILE_WRITE = "file_write"
    FILE_DELETE = "file_delete"
    CODE_GENERATE = "code_generate"
    CODE_EXECUTE = "code_execute"
    API_CALL = "api_call"
    SYSTEM_INFO = "system_info"


@dataclass
class Action:
    """A single action to be executed."""
    id: str = field(default_factory=lambda: str(uuid.uuid4())[:8])
    type: ActionType = ActionType.SHELL_COMMAND
    command: str = ""
    explanation: str = ""
    reversible: bool = True
    rollback_command: Optional[str] = None
    risk_level: str = "low"  # low, medium, high, critical
    requires_consent: bool = True

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "type": self.type.value,
            "command": self.command,
            "explanation": self.explanation,
            "reversible": self.reversible,
            "rollback_command": self.rollback_command,
            "risk_level": self.risk_level,
        }


@dataclass
class ExecutionPlan:
    """A plan consisting of multiple actions."""
    id: str = field(default_factory=lambda: str(uuid.uuid4())[:8])
    task: str = ""
    actions: List[Action] = field(default_factory=list)
    context: Dict[str, Any] = field(default_factory=dict)
    created_at: datetime = field(default_factory=datetime.now)

    @property
    def total_actions(self) -> int:
        return len(self.actions)

    @property
    def high_risk_actions(self) -> List[Action]:
        return [a for a in self.actions if a.risk_level in ("high", "critical")]


@dataclass
class ExecutionResult:
    """Result of executing an action or plan."""
    success: bool
    output: str = ""
    error: str = ""
    duration_ms: int = 0
    action_id: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "success": self.success,
            "output": self.output[:1000] if self.output else "",
            "error": self.error,
            "duration_ms": self.duration_ms,
            "action_id": self.action_id,
        }


@dataclass
class Session:
    """An execution session with full history for rollback."""
    id: str = field(default_factory=lambda: datetime.now().strftime("%Y%m%d_%H%M%S"))
    task: str = ""
    state: TaskState = TaskState.PENDING
    plan: Optional[ExecutionPlan] = None
    executed_actions: List[Action] = field(default_factory=list)
    results: List[ExecutionResult] = field(default_factory=list)
    started_at: datetime = field(default_factory=datetime.now)
    completed_at: Optional[datetime] = None

    def add_result(self, action: Action, result: ExecutionResult):
        self.executed_actions.append(action)
        self.results.append(result)

    def get_rollback_actions(self) -> List[Action]:
        """Get actions needed to rollback this session."""
        rollbacks = []
        for action in reversed(self.executed_actions):
            if action.reversible and action.rollback_command:
                rollbacks.append(Action(
                    type=action.type,
                    command=action.rollback_command,
                    explanation=f"Rollback: {action.explanation}",
                    requires_consent=True,
                ))
        return rollbacks


class LLMProvider(Protocol):
    """Protocol for LLM providers."""

    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
    ) -> str:
        """Generate a response from the LLM."""
        ...

    def is_available(self) -> bool:
        """Check if provider is available."""
        ...


class ConsentHandler(Protocol):
    """Protocol for handling user consent."""

    async def request_consent(
        self,
        plan: ExecutionPlan,
        auto_approve: bool = False,
    ) -> tuple[bool, List[str]]:
        """
        Request user consent for a plan.
        Returns (approved, list of approved action IDs).
        """
        ...


class GaneshaEngine:
    """
    The core Ganesha execution engine.

    Responsibilities:
    - Parse natural language tasks into execution plans
    - Request user consent before execution
    - Execute actions safely
    - Handle errors and iterate
    - Maintain session history for rollback
    """

    def __init__(
        self,
        llm_provider: LLMProvider,
        consent_handler: Optional[ConsentHandler] = None,
        session_dir: Optional[Path] = None,
        auto_approve: bool = False,
    ):
        self.llm = llm_provider
        self.consent = consent_handler
        self.session_dir = session_dir or Path.home() / ".ganesha" / "sessions"
        self.session_dir.mkdir(parents=True, exist_ok=True)
        self.auto_approve = auto_approve
        self.current_session: Optional[Session] = None
        self._event_handlers: List[Callable] = []

    def on_event(self, handler: Callable):
        """Register an event handler for observability."""
        self._event_handlers.append(handler)

    async def _emit(self, event: str, data: Dict[str, Any]):
        """Emit an event to all handlers."""
        for handler in self._event_handlers:
            try:
                if asyncio.iscoroutinefunction(handler):
                    await handler(event, data)
                else:
                    handler(event, data)
            except Exception:
                pass  # Don't let handler errors break execution

    async def plan(self, task: str, context: Optional[Dict] = None) -> ExecutionPlan:
        """
        Create an execution plan for a task.

        This is where natural language becomes actionable steps.
        """
        await self._emit("planning_started", {"task": task})

        system_prompt = self._get_planning_prompt()
        user_prompt = self._build_task_prompt(task, context)

        response = await self.llm.generate(
            system_prompt=system_prompt,
            user_prompt=user_prompt,
            temperature=0.3,
            max_tokens=2000,
        )

        plan = self._parse_plan(task, response, context)
        await self._emit("planning_completed", {"plan": plan.id, "actions": plan.total_actions})

        return plan

    async def execute(
        self,
        task: str,
        context: Optional[Dict] = None,
        max_iterations: int = 3,
    ) -> AsyncIterator[ExecutionResult]:
        """
        Execute a task with planning, consent, and iteration.

        Yields results as each action completes.
        """
        # Start session
        self.current_session = Session(task=task)
        self.current_session.state = TaskState.PLANNING
        await self._emit("session_started", {"session_id": self.current_session.id})

        try:
            # Plan
            plan = await self.plan(task, context)
            self.current_session.plan = plan

            # Get consent
            self.current_session.state = TaskState.AWAITING_CONSENT
            approved, approved_ids = await self._get_consent(plan)

            if not approved:
                self.current_session.state = TaskState.CANCELLED
                yield ExecutionResult(success=False, error="User cancelled")
                return

            # Execute with iteration
            self.current_session.state = TaskState.EXECUTING
            iteration = 0

            while iteration < max_iterations:
                iteration += 1
                await self._emit("iteration_started", {"iteration": iteration})

                all_success = True
                for action in plan.actions:
                    if action.id not in approved_ids:
                        continue

                    result = await self._execute_action(action)
                    self.current_session.add_result(action, result)
                    yield result

                    if not result.success:
                        all_success = False
                        # Try to recover
                        if iteration < max_iterations:
                            recovery_plan = await self._plan_recovery(action, result)
                            if recovery_plan:
                                plan = recovery_plan
                                break

                if all_success:
                    break

            self.current_session.state = TaskState.COMPLETED
            self.current_session.completed_at = datetime.now()

        except Exception as e:
            self.current_session.state = TaskState.FAILED
            yield ExecutionResult(success=False, error=str(e))

        finally:
            await self._save_session()
            await self._emit("session_completed", {
                "session_id": self.current_session.id,
                "state": self.current_session.state.value,
            })

    async def rollback(self, session_id: Optional[str] = None) -> AsyncIterator[ExecutionResult]:
        """Rollback a session's changes."""
        session = await self._load_session(session_id)
        if not session:
            yield ExecutionResult(success=False, error="Session not found")
            return

        rollback_actions = session.get_rollback_actions()
        if not rollback_actions:
            yield ExecutionResult(success=False, error="No reversible actions to rollback")
            return

        for action in rollback_actions:
            result = await self._execute_action(action)
            yield result

    async def _get_consent(self, plan: ExecutionPlan) -> tuple[bool, List[str]]:
        """Get user consent for plan execution."""
        if self.auto_approve:
            return True, [a.id for a in plan.actions]

        if self.consent:
            return await self.consent.request_consent(plan, self.auto_approve)

        # Default: approve all
        return True, [a.id for a in plan.actions]

    async def _execute_action(self, action: Action) -> ExecutionResult:
        """Execute a single action."""
        import subprocess
        import time

        await self._emit("action_started", {"action_id": action.id, "command": action.command})
        start = time.time()

        try:
            if action.type == ActionType.SHELL_COMMAND:
                result = subprocess.run(
                    action.command,
                    shell=True,
                    capture_output=True,
                    text=True,
                    timeout=300,
                )
                success = result.returncode == 0
                output = result.stdout
                error = result.stderr if not success else ""

            elif action.type == ActionType.FILE_READ:
                path = Path(action.command)
                if path.exists():
                    output = path.read_text()
                    success = True
                    error = ""
                else:
                    success = False
                    output = ""
                    error = f"File not found: {action.command}"

            elif action.type == ActionType.FILE_WRITE:
                # command format: "path|||content"
                parts = action.command.split("|||", 1)
                if len(parts) == 2:
                    path = Path(parts[0])
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(parts[1])
                    success = True
                    output = f"Written to {path}"
                    error = ""
                else:
                    success = False
                    output = ""
                    error = "Invalid file write format"

            else:
                success = False
                output = ""
                error = f"Unsupported action type: {action.type}"

        except subprocess.TimeoutExpired:
            success = False
            output = ""
            error = "Command timed out"
        except Exception as e:
            success = False
            output = ""
            error = str(e)

        duration = int((time.time() - start) * 1000)
        result = ExecutionResult(
            success=success,
            output=output,
            error=error,
            duration_ms=duration,
            action_id=action.id,
        )

        await self._emit("action_completed", result.to_dict())
        return result

    async def _plan_recovery(self, failed_action: Action, result: ExecutionResult) -> Optional[ExecutionPlan]:
        """Plan recovery from a failed action."""
        prompt = f"""The following action failed:
Command: {failed_action.command}
Error: {result.error}

Generate a recovery plan to fix this issue."""

        try:
            response = await self.llm.generate(
                system_prompt=self._get_planning_prompt(),
                user_prompt=prompt,
            )
            return self._parse_plan(f"Recovery: {failed_action.explanation}", response, {})
        except Exception:
            return None

    async def _save_session(self):
        """Save current session to disk."""
        if not self.current_session:
            return

        session_file = self.session_dir / f"{self.current_session.id}.json"
        data = {
            "id": self.current_session.id,
            "task": self.current_session.task,
            "state": self.current_session.state.value,
            "started_at": self.current_session.started_at.isoformat(),
            "completed_at": self.current_session.completed_at.isoformat() if self.current_session.completed_at else None,
            "executed_actions": [a.to_dict() for a in self.current_session.executed_actions],
            "results": [r.to_dict() for r in self.current_session.results],
        }
        session_file.write_text(json.dumps(data, indent=2))

    async def _load_session(self, session_id: Optional[str] = None) -> Optional[Session]:
        """Load a session from disk."""
        if session_id:
            session_file = self.session_dir / f"{session_id}.json"
        else:
            # Get most recent
            sessions = sorted(self.session_dir.glob("*.json"), reverse=True)
            if not sessions:
                return None
            session_file = sessions[0]

        if not session_file.exists():
            return None

        data = json.loads(session_file.read_text())
        session = Session(
            id=data["id"],
            task=data["task"],
            state=TaskState(data["state"]),
        )

        for action_data in data.get("executed_actions", []):
            session.executed_actions.append(Action(
                id=action_data["id"],
                type=ActionType(action_data["type"]),
                command=action_data["command"],
                explanation=action_data["explanation"],
                reversible=action_data.get("reversible", True),
                rollback_command=action_data.get("rollback_command"),
            ))

        return session

    def _get_planning_prompt(self) -> str:
        """System prompt for planning."""
        import platform
        return f"""You are Ganesha, the Remover of Obstacles.
You translate natural language tasks into executable system commands.

SYSTEM: {platform.system()} {platform.release()}
ARCH: {platform.machine()}

OUTPUT FORMAT (JSON only):
{{
  "actions": [
    {{
      "type": "shell_command",
      "command": "actual command",
      "explanation": "what this does",
      "risk_level": "low|medium|high|critical",
      "reversible": true,
      "rollback_command": "command to undo (if reversible)"
    }}
  ]
}}

RULES:
- Output ONLY valid JSON
- Use appropriate commands for {platform.system()}
- Assess risk level honestly
- Provide rollback commands when possible
- Break complex tasks into simple steps"""

    def _build_task_prompt(self, task: str, context: Optional[Dict]) -> str:
        """Build user prompt for a task."""
        prompt = f"TASK: {task}"
        if context:
            prompt += f"\n\nCONTEXT:\n{json.dumps(context, indent=2)}"
        return prompt

    def _parse_plan(self, task: str, response: str, context: Optional[Dict]) -> ExecutionPlan:
        """Parse LLM response into execution plan."""
        plan = ExecutionPlan(task=task, context=context or {})

        try:
            # Extract JSON
            start = response.find("{")
            end = response.rfind("}") + 1
            if start >= 0 and end > start:
                data = json.loads(response[start:end])

                for action_data in data.get("actions", []):
                    action = Action(
                        type=ActionType(action_data.get("type", "shell_command")),
                        command=action_data.get("command", ""),
                        explanation=action_data.get("explanation", ""),
                        risk_level=action_data.get("risk_level", "low"),
                        reversible=action_data.get("reversible", True),
                        rollback_command=action_data.get("rollback_command"),
                    )
                    plan.actions.append(action)
        except (json.JSONDecodeError, KeyError, ValueError):
            pass

        return plan
