"""
Ganesha HTTP API Server

REST API for integrating Ganesha with web applications like Claudia Admin.

Endpoints:
- POST /execute     - Execute a task
- POST /plan        - Plan without executing
- POST /rollback    - Rollback a session
- GET  /history     - Get session history
- GET  /providers   - List available LLM providers
- GET  /health      - Health check

Run:
    python -m ganesha.api.server
    # or with uvicorn:
    uvicorn ganesha.api.server:app --host 0.0.0.0 --port 8420
"""

import asyncio
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

# FastAPI (install with: pip install fastapi uvicorn)
try:
    from fastapi import FastAPI, HTTPException, BackgroundTasks
    from fastapi.middleware.cors import CORSMiddleware
    from pydantic import BaseModel
    HAS_FASTAPI = True
except ImportError:
    HAS_FASTAPI = False
    print("FastAPI not installed. Run: pip install fastapi uvicorn", file=sys.stderr)

# Import Ganesha
sys.path.insert(0, str(Path(__file__).parent.parent.parent))
from ganesha.core.engine import GaneshaEngine
from ganesha.providers.llm import create_provider_chain, AsyncProviderWrapper

# Create app
if HAS_FASTAPI:
    app = FastAPI(
        title="Ganesha API",
        description="The Remover of Obstacles - AI-Powered System Control",
        version="3.0.0",
    )

    # CORS for Claudia integration
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],  # Configure for production
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )
else:
    app = None


# ═══════════════════════════════════════════════════════════════════════════
# MODELS
# ═══════════════════════════════════════════════════════════════════════════

if HAS_FASTAPI:

    class ExecuteRequest(BaseModel):
        task: str
        auto_approve: bool = False
        max_iterations: int = 3
        context: Optional[Dict[str, Any]] = None

    class PlanRequest(BaseModel):
        task: str
        context: Optional[Dict[str, Any]] = None

    class RollbackRequest(BaseModel):
        session_id: str = "last"

    class CodeGenRequest(BaseModel):
        task: str
        language: str = "typescript"
        framework: Optional[str] = None
        tech_stack: Optional[List[str]] = None

    class ActionResponse(BaseModel):
        id: str
        type: str
        command: str
        explanation: str
        risk_level: str

    class PlanResponse(BaseModel):
        plan_id: str
        task: str
        total_actions: int
        high_risk_count: int
        actions: List[ActionResponse]

    class ExecutionResultResponse(BaseModel):
        success: bool
        output: str
        error: str
        duration_ms: int
        action_id: Optional[str]

    class ExecuteResponse(BaseModel):
        session_id: str
        task: str
        results: List[ExecutionResultResponse]
        completed: bool

    class ProviderInfo(BaseModel):
        name: str
        available: bool
        models: List[str]

    class HealthResponse(BaseModel):
        status: str
        version: str
        providers_available: int


# ═══════════════════════════════════════════════════════════════════════════
# ENGINE SINGLETON
# ═══════════════════════════════════════════════════════════════════════════

_engine: Optional[GaneshaEngine] = None
_chain = None


def get_engine() -> GaneshaEngine:
    """Get or create the Ganesha engine."""
    global _engine, _chain

    if _engine is None:
        _chain = create_provider_chain()
        _engine = GaneshaEngine(
            llm_provider=AsyncProviderWrapper(_chain),
            auto_approve=False,
        )

    return _engine


def get_chain():
    """Get the provider chain."""
    global _chain
    if _chain is None:
        _chain = create_provider_chain()
    return _chain


# ═══════════════════════════════════════════════════════════════════════════
# ENDPOINTS
# ═══════════════════════════════════════════════════════════════════════════

if HAS_FASTAPI:

    @app.get("/health", response_model=HealthResponse)
    async def health_check():
        """Health check endpoint."""
        chain = get_chain()
        available = len(chain.get_available_providers())
        return HealthResponse(
            status="healthy" if available > 0 else "degraded",
            version="3.0.0",
            providers_available=available,
        )

    @app.get("/providers", response_model=List[ProviderInfo])
    async def list_providers():
        """List available LLM providers."""
        chain = get_chain()
        providers = []
        for p in chain.providers:
            providers.append(ProviderInfo(
                name=p.name,
                available=p.is_available(),
                models=p.list_models() if p.is_available() else [],
            ))
        return providers

    @app.post("/plan", response_model=PlanResponse)
    async def create_plan(request: PlanRequest):
        """Create an execution plan without executing."""
        engine = get_engine()
        plan = await engine.plan(request.task, request.context)

        return PlanResponse(
            plan_id=plan.id,
            task=plan.task,
            total_actions=plan.total_actions,
            high_risk_count=len(plan.high_risk_actions),
            actions=[
                ActionResponse(
                    id=a.id,
                    type=a.type.value,
                    command=a.command,
                    explanation=a.explanation,
                    risk_level=a.risk_level,
                )
                for a in plan.actions
            ],
        )

    @app.post("/execute", response_model=ExecuteResponse)
    async def execute_task(request: ExecuteRequest):
        """Execute a task."""
        engine = get_engine()

        # Temporarily set auto_approve
        original = engine.auto_approve
        engine.auto_approve = request.auto_approve

        results = []
        try:
            async for result in engine.execute(
                request.task,
                request.context,
                request.max_iterations,
            ):
                results.append(ExecutionResultResponse(
                    success=result.success,
                    output=result.output[:5000] if result.output else "",
                    error=result.error,
                    duration_ms=result.duration_ms,
                    action_id=result.action_id,
                ))
        finally:
            engine.auto_approve = original

        return ExecuteResponse(
            session_id=engine.current_session.id if engine.current_session else "",
            task=request.task,
            results=results,
            completed=True,
        )

    @app.post("/rollback")
    async def rollback_session(request: RollbackRequest):
        """Rollback a session."""
        engine = get_engine()

        session_id = request.session_id if request.session_id != "last" else None
        results = []

        async for result in engine.rollback(session_id):
            results.append({
                "success": result.success,
                "output": result.output[:1000],
                "error": result.error,
            })

        return {"session_id": request.session_id, "results": results}

    @app.get("/history")
    async def get_history(limit: int = 10):
        """Get session history."""
        engine = get_engine()
        sessions = sorted(engine.session_dir.glob("*.json"), reverse=True)[:limit]

        history = []
        for session_file in sessions:
            try:
                data = json.loads(session_file.read_text())
                history.append({
                    "id": data.get("id"),
                    "task": data.get("task"),
                    "state": data.get("state"),
                    "started_at": data.get("started_at"),
                    "actions_count": len(data.get("executed_actions", [])),
                })
            except Exception:
                pass

        return {"sessions": history}

    @app.post("/generate-code")
    async def generate_code(request: CodeGenRequest):
        """Generate code without executing."""
        chain = get_chain()

        framework_str = f"\nFramework: {request.framework}" if request.framework else ""
        tech_str = f"\nTech Stack: {', '.join(request.tech_stack)}" if request.tech_stack else ""

        system_prompt = f"""You are an expert {request.language} developer.{framework_str}{tech_str}

OUTPUT FORMAT:
=== FILE: path/to/file ===
```{request.language}
// code
```

RULES:
- Output ONLY code
- Use best practices
- Include proper types
- Handle errors"""

        response = await chain.generate(
            system_prompt=system_prompt,
            user_prompt=f"Generate: {request.task}",
            temperature=0.3,
            max_tokens=4000,
        )

        if response.error:
            raise HTTPException(status_code=500, detail=response.error)

        # Parse files
        import re
        files = []
        pattern = r'===\s*FILE:\s*(.+?)\s*===\s*\n```\w*\n([\s\S]*?)```'
        for path, content in re.findall(pattern, response.content):
            files.append({"path": path.strip(), "content": content.strip()})

        return {
            "task": request.task,
            "language": request.language,
            "framework": request.framework,
            "files": files,
            "model": response.model,
            "provider": response.provider,
        }


# ═══════════════════════════════════════════════════════════════════════════
# MAIN
# ═══════════════════════════════════════════════════════════════════════════

def main():
    """Run the API server."""
    if not HAS_FASTAPI:
        print("FastAPI required. Install with: pip install fastapi uvicorn")
        return

    import uvicorn
    uvicorn.run(
        "ganesha.api.server:app",
        host="0.0.0.0",
        port=8420,  # GANESHA on a phone keypad
        reload=True,
    )


if __name__ == "__main__":
    main()
