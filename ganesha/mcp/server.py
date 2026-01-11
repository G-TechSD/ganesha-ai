"""
Ganesha MCP Server

Exposes Ganesha's capabilities as MCP tools that can be used by:
- Claude Code
- Claudia Admin
- Any MCP-compatible client

This makes Ganesha a composable building block in the AI ecosystem.

Run:
    python -m ganesha.mcp.server

Or add to claude_desktop_config.json:
    {
        "mcpServers": {
            "ganesha": {
                "command": "python",
                "args": ["-m", "ganesha.mcp.server"]
            }
        }
    }
"""

import asyncio
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

# MCP SDK (install with: pip install mcp)
try:
    from mcp.server import Server
    from mcp.server.stdio import stdio_server
    from mcp.types import Tool, TextContent
    HAS_MCP = True
except ImportError:
    HAS_MCP = False
    print("MCP SDK not installed. Run: pip install mcp", file=sys.stderr)


# Import Ganesha components
sys.path.insert(0, str(Path(__file__).parent.parent.parent))
from ganesha.core.engine import GaneshaEngine, Action, ActionType, ExecutionPlan
from ganesha.providers.llm import create_provider_chain, AsyncProviderWrapper


class GaneshaMCPServer:
    """
    MCP Server exposing Ganesha tools.

    Tools:
    - ganesha_execute: Execute a system task
    - ganesha_plan: Plan actions without executing
    - ganesha_rollback: Rollback a session
    - ganesha_history: Get session history
    """

    def __init__(self):
        self.server = Server("ganesha")
        self.engine: Optional[GaneshaEngine] = None
        self._setup_tools()
        self._setup_handlers()

    def _setup_tools(self):
        """Register MCP tools."""
        self.server.list_tools = self._list_tools
        self.server.call_tool = self._call_tool

    def _setup_handlers(self):
        """Set up additional handlers."""
        pass

    async def _list_tools(self) -> List[Tool]:
        """List available tools."""
        return [
            Tool(
                name="ganesha_execute",
                description=(
                    "Execute a system administration task using AI. "
                    "Ganesha translates natural language into safe, executable commands. "
                    "Example: 'install docker' or 'find all large files'"
                ),
                inputSchema={
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Task in plain English",
                        },
                        "auto_approve": {
                            "type": "boolean",
                            "description": "Skip user confirmation (DANGEROUS)",
                            "default": False,
                        },
                    },
                    "required": ["task"],
                },
            ),
            Tool(
                name="ganesha_plan",
                description=(
                    "Create an execution plan for a task WITHOUT executing it. "
                    "Returns the planned actions for review."
                ),
                inputSchema={
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Task to plan",
                        },
                    },
                    "required": ["task"],
                },
            ),
            Tool(
                name="ganesha_rollback",
                description="Rollback a previous Ganesha session's changes.",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "Session ID to rollback (or 'last' for most recent)",
                            "default": "last",
                        },
                    },
                },
            ),
            Tool(
                name="ganesha_history",
                description="Get recent Ganesha session history.",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Max sessions to return",
                            "default": 10,
                        },
                    },
                },
            ),
            Tool(
                name="ganesha_generate_code",
                description=(
                    "Generate code for a programming task. "
                    "Returns generated files without executing them."
                ),
                inputSchema={
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Code generation task",
                        },
                        "language": {
                            "type": "string",
                            "description": "Programming language",
                            "default": "typescript",
                        },
                        "framework": {
                            "type": "string",
                            "description": "Framework (e.g., react, next, express)",
                        },
                    },
                    "required": ["task"],
                },
            ),
        ]

    async def _call_tool(self, name: str, arguments: Dict[str, Any]) -> List[TextContent]:
        """Handle tool calls."""
        # Lazy init engine
        if not self.engine:
            chain = create_provider_chain()
            self.engine = GaneshaEngine(
                llm_provider=AsyncProviderWrapper(chain),
                auto_approve=False,
            )

        try:
            if name == "ganesha_execute":
                return await self._execute(arguments)
            elif name == "ganesha_plan":
                return await self._plan(arguments)
            elif name == "ganesha_rollback":
                return await self._rollback(arguments)
            elif name == "ganesha_history":
                return await self._history(arguments)
            elif name == "ganesha_generate_code":
                return await self._generate_code(arguments)
            else:
                return [TextContent(type="text", text=f"Unknown tool: {name}")]
        except Exception as e:
            return [TextContent(type="text", text=f"Error: {str(e)}")]

    async def _execute(self, args: Dict[str, Any]) -> List[TextContent]:
        """Execute a task."""
        task = args.get("task", "")
        auto_approve = args.get("auto_approve", False)

        if not task:
            return [TextContent(type="text", text="No task provided")]

        # Override auto_approve on engine
        original = self.engine.auto_approve
        self.engine.auto_approve = auto_approve

        results = []
        try:
            async for result in self.engine.execute(task):
                results.append(result.to_dict())
        finally:
            self.engine.auto_approve = original

        return [TextContent(
            type="text",
            text=json.dumps({"task": task, "results": results}, indent=2)
        )]

    async def _plan(self, args: Dict[str, Any]) -> List[TextContent]:
        """Plan without executing."""
        task = args.get("task", "")
        if not task:
            return [TextContent(type="text", text="No task provided")]

        plan = await self.engine.plan(task)

        plan_data = {
            "task": task,
            "plan_id": plan.id,
            "total_actions": plan.total_actions,
            "high_risk_count": len(plan.high_risk_actions),
            "actions": [a.to_dict() for a in plan.actions],
        }

        return [TextContent(type="text", text=json.dumps(plan_data, indent=2))]

    async def _rollback(self, args: Dict[str, Any]) -> List[TextContent]:
        """Rollback session."""
        session_id = args.get("session_id", "last")

        results = []
        async for result in self.engine.rollback(session_id if session_id != "last" else None):
            results.append(result.to_dict())

        return [TextContent(
            type="text",
            text=json.dumps({"session_id": session_id, "results": results}, indent=2)
        )]

    async def _history(self, args: Dict[str, Any]) -> List[TextContent]:
        """Get session history."""
        limit = args.get("limit", 10)

        sessions = sorted(self.engine.session_dir.glob("*.json"), reverse=True)[:limit]
        history = []

        for session_file in sessions:
            try:
                data = json.loads(session_file.read_text())
                history.append({
                    "id": data.get("id"),
                    "task": data.get("task"),
                    "state": data.get("state"),
                    "actions_count": len(data.get("executed_actions", [])),
                })
            except Exception:
                pass

        return [TextContent(type="text", text=json.dumps(history, indent=2))]

    async def _generate_code(self, args: Dict[str, Any]) -> List[TextContent]:
        """Generate code."""
        task = args.get("task", "")
        language = args.get("language", "typescript")
        framework = args.get("framework", "")

        if not task:
            return [TextContent(type="text", text="No task provided")]

        # Build code generation prompt
        system_prompt = f"""You are an expert {language} developer.
{f'Framework: {framework}' if framework else ''}

OUTPUT FORMAT:
For each file, use:

=== FILE: path/to/file.{language[:2]} ===
```{language}
// code here
```

RULES:
- Output ONLY code in the format above
- Use best practices
- Include proper types/interfaces
- Handle edge cases"""

        user_prompt = f"Generate code for: {task}"

        chain = create_provider_chain()
        response = await chain.generate(
            system_prompt=system_prompt,
            user_prompt=user_prompt,
            temperature=0.3,
            max_tokens=4000,
        )

        if response.error:
            return [TextContent(type="text", text=f"Error: {response.error}")]

        # Parse files from response
        import re
        files = []
        pattern = r'===\s*FILE:\s*(.+?)\s*===\s*\n```\w*\n([\s\S]*?)```'
        for path, content in re.findall(pattern, response.content):
            files.append({"path": path.strip(), "content": content.strip()})

        return [TextContent(
            type="text",
            text=json.dumps({
                "task": task,
                "language": language,
                "framework": framework,
                "files_count": len(files),
                "files": files,
                "model": response.model,
                "provider": response.provider,
            }, indent=2)
        )]

    async def run(self):
        """Run the MCP server."""
        if not HAS_MCP:
            print("MCP SDK required. Install with: pip install mcp")
            return

        async with stdio_server() as (read_stream, write_stream):
            await self.server.run(read_stream, write_stream)


async def main():
    """Entry point."""
    server = GaneshaMCPServer()
    await server.run()


if __name__ == "__main__":
    asyncio.run(main())
