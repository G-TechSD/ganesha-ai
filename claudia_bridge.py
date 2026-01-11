"""
Claudia Bridge - Integration between Ganesha and Claudia Admin

This module allows Ganesha to:
1. Fetch work packets from Claudia's queue
2. Execute code generation using local LLMs
3. Report results back to Claudia
4. Apply generated code to GitLab

Usage:
    from claudia_bridge import ClaudiaBridge

    bridge = ClaudiaBridge()
    packets = bridge.get_pending_packets()
    for packet in packets:
        result = bridge.execute_packet(packet)
        bridge.report_result(result)
"""

import os
import json
import requests
from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any
from pathlib import Path
from datetime import datetime

from providers import ProviderChain, create_default_chain, LLMResponse


@dataclass
class WorkPacket:
    """A work packet from Claudia."""
    id: str
    title: str
    description: str
    phase: str  # scaffold, shared, features, integration, polish
    project_id: str
    project_name: str
    tasks: List[Dict[str, Any]] = field(default_factory=list)
    acceptance_criteria: List[str] = field(default_factory=list)
    tech_stack: List[str] = field(default_factory=list)
    file_context: List[str] = field(default_factory=list)


@dataclass
class ExecutionResult:
    """Result of executing a packet."""
    packet_id: str
    success: bool
    files: List[Dict[str, str]] = field(default_factory=list)  # [{path, content}]
    errors: List[str] = field(default_factory=list)
    iterations: int = 1
    confidence: float = 0.0
    duration_ms: int = 0
    model_used: str = ""
    provider_used: str = ""


class ClaudiaBridge:
    """
    Bridge between Ganesha CLI and Claudia Admin.

    Can work in two modes:
    1. API mode: Connect to running Claudia server
    2. File mode: Read/write directly to shared config files
    """

    def __init__(
        self,
        claudia_url: str = "http://localhost:3000",
        config_dir: Optional[Path] = None,
        provider_chain: Optional[ProviderChain] = None
    ):
        self.claudia_url = claudia_url.rstrip("/")
        self.config_dir = config_dir or Path.home() / ".claudia"
        self.config_dir.mkdir(exist_ok=True)
        self.provider_chain = provider_chain or create_default_chain()

        # Shared files with Claudia
        self.queue_file = self.config_dir / "execution_queue.json"
        self.state_file = self.config_dir / "agent_state.json"
        self.results_file = self.config_dir / "execution_results.json"

    def check_claudia_connection(self) -> bool:
        """Check if Claudia server is running."""
        try:
            response = requests.get(f"{self.claudia_url}/api/providers", timeout=5)
            return response.status_code == 200
        except:
            return False

    def get_queue_from_file(self) -> List[WorkPacket]:
        """Read queue from shared file (localStorage export)."""
        if not self.queue_file.exists():
            return []

        try:
            with open(self.queue_file) as f:
                data = json.load(f)

            packets = []
            for item in data:
                project = item.get("project", {})
                for packet in item.get("packets", []):
                    packets.append(WorkPacket(
                        id=packet.get("id", ""),
                        title=packet.get("title", ""),
                        description=packet.get("description", ""),
                        phase=packet.get("phase", "features"),
                        project_id=item.get("projectId", ""),
                        project_name=project.get("name", ""),
                        tasks=packet.get("tasks", []),
                        acceptance_criteria=packet.get("acceptanceCriteria", []),
                        tech_stack=project.get("techStack", []),
                        file_context=project.get("files", [])
                    ))

            return packets
        except Exception as e:
            print(f"Error reading queue: {e}")
            return []

    def get_pending_packets(self) -> List[WorkPacket]:
        """Get all pending work packets."""
        # Try API first
        if self.check_claudia_connection():
            try:
                response = requests.get(f"{self.claudia_url}/api/queue")
                if response.ok:
                    data = response.json()
                    # Parse API response into WorkPackets
                    return self._parse_api_queue(data)
            except:
                pass

        # Fall back to file
        return self.get_queue_from_file()

    def _parse_api_queue(self, data: Dict) -> List[WorkPacket]:
        """Parse API queue response."""
        packets = []
        for item in data.get("queue", []):
            packets.append(WorkPacket(
                id=item.get("packetId", ""),
                title=item.get("title", ""),
                description=item.get("description", ""),
                phase=item.get("phase", "features"),
                project_id=item.get("projectId", ""),
                project_name=item.get("projectName", ""),
                tasks=item.get("tasks", []),
                acceptance_criteria=item.get("acceptanceCriteria", [])
            ))
        return packets

    def execute_packet(
        self,
        packet: WorkPacket,
        max_iterations: int = 3,
        min_confidence: float = 0.7,
        debug: bool = False
    ) -> ExecutionResult:
        """
        Execute a work packet using local LLM.

        Uses the Wiggum Loop (Ralph says "I'm helping!"):
        - Generate code
        - Self-critique
        - Iterate until quality threshold or max iterations
        """
        start_time = datetime.now()
        all_files = []
        errors = []

        for iteration in range(1, max_iterations + 1):
            print(f"\n[Iteration {iteration}/{max_iterations}] {packet.title}")

            # Build generation prompt
            gen_prompt = self._build_generation_prompt(packet, errors)
            gen_response = self.provider_chain.generate(
                system_prompt=self._get_code_gen_system_prompt(),
                user_prompt=gen_prompt,
                temperature=0.3,
                max_tokens=4000
            )

            if gen_response.error:
                errors.append(f"Generation error: {gen_response.error}")
                continue

            # Parse generated files
            files = self._parse_code_output(gen_response.content)
            if not files:
                errors.append("No files parsed from output")
                continue

            all_files = files  # Replace with latest iteration

            if debug:
                print(f"  Generated {len(files)} files")
                for f in files:
                    print(f"    - {f['path']}")

            # Self-critique
            critique = self._self_critique(packet, files)
            confidence = critique.get("confidence", 0.5)
            issues = critique.get("issues", [])

            print(f"  Confidence: {confidence*100:.0f}%")

            if confidence >= min_confidence:
                print(f"  Quality threshold met!")
                duration = int((datetime.now() - start_time).total_seconds() * 1000)
                return ExecutionResult(
                    packet_id=packet.id,
                    success=True,
                    files=files,
                    iterations=iteration,
                    confidence=confidence,
                    duration_ms=duration,
                    model_used=gen_response.model,
                    provider_used=gen_response.provider
                )

            if issues:
                print(f"  Issues: {issues[:2]}")
                errors = issues  # Use issues for next iteration context

        # Max iterations reached
        duration = int((datetime.now() - start_time).total_seconds() * 1000)
        return ExecutionResult(
            packet_id=packet.id,
            success=len(all_files) > 0,  # Partial success if we have files
            files=all_files,
            errors=errors,
            iterations=max_iterations,
            confidence=0.5,
            duration_ms=duration,
            model_used=gen_response.model if 'gen_response' in dir() else "",
            provider_used=gen_response.provider if 'gen_response' in dir() else ""
        )

    def _get_code_gen_system_prompt(self) -> str:
        """System prompt for code generation."""
        return """You are an expert developer generating production-ready code.

OUTPUT FORMAT:
For each file, use this exact format:

=== FILE: path/to/file.tsx ===
```typescript
// file contents
```

RULES:
- Output ONLY code in the format above
- Use proper imports and exports
- Follow modern best practices
- Include TypeScript types
- Handle edge cases
- No explanations, just code"""

    def _build_generation_prompt(self, packet: WorkPacket, previous_issues: List[str] = None) -> str:
        """Build the user prompt for code generation."""
        tasks_str = "\n".join(f"- {t.get('description', t)}" for t in packet.tasks) if packet.tasks else packet.description
        criteria_str = "\n".join(f"- {c}" for c in packet.acceptance_criteria) if packet.acceptance_criteria else "N/A"
        tech_str = ", ".join(packet.tech_stack) if packet.tech_stack else "TypeScript, React, Tailwind"

        prompt = f"""PROJECT: {packet.project_name}
TECH STACK: {tech_str}

FEATURE: {packet.title}
{packet.description}

TASKS:
{tasks_str}

ACCEPTANCE CRITERIA:
{criteria_str}

Generate all necessary files to implement this feature."""

        if previous_issues:
            prompt += f"\n\nFIX THESE ISSUES FROM PREVIOUS ATTEMPT:\n" + "\n".join(f"- {i}" for i in previous_issues[:5])

        return prompt

    def _parse_code_output(self, output: str) -> List[Dict[str, str]]:
        """Parse code blocks from LLM output."""
        import re
        files = []

        # Pattern: === FILE: path === followed by code block
        pattern = r'===\s*FILE:\s*(.+?)\s*===\s*\n```\w*\n([\s\S]*?)```'
        matches = re.findall(pattern, output)

        for path, content in matches:
            path = path.strip()
            content = content.strip()
            if path and content and not path.startswith(".."):
                files.append({"path": path, "content": content})

        return files

    def _self_critique(self, packet: WorkPacket, files: List[Dict]) -> Dict:
        """Self-critique the generated code."""
        system_prompt = """You are a senior code reviewer. Evaluate this code critically.

OUTPUT JSON ONLY:
{
  "confidence": 0.0 to 1.0,
  "issues": ["list of problems"],
  "suggestions": ["improvements"],
  "passes_criteria": true/false
}"""

        files_str = "\n\n".join(f"// {f['path']}\n{f['content'][:1000]}" for f in files[:5])
        criteria_str = "\n".join(f"- {c}" for c in packet.acceptance_criteria)

        user_prompt = f"""FEATURE: {packet.title}

ACCEPTANCE CRITERIA:
{criteria_str}

GENERATED CODE:
{files_str}

Evaluate this code critically."""

        response = self.provider_chain.generate(
            system_prompt=system_prompt,
            user_prompt=user_prompt,
            temperature=0.2,
            max_tokens=1000
        )

        if response.error:
            return {"confidence": 0.5, "issues": [response.error]}

        # Parse JSON from response
        try:
            import re
            json_match = re.search(r'\{[\s\S]*\}', response.content)
            if json_match:
                return json.loads(json_match.group())
        except:
            pass

        return {"confidence": 0.6, "issues": ["Could not parse critique"]}

    def save_result(self, result: ExecutionResult):
        """Save execution result to file."""
        results = []
        if self.results_file.exists():
            try:
                with open(self.results_file) as f:
                    results = json.load(f)
            except:
                pass

        results.append({
            "packet_id": result.packet_id,
            "success": result.success,
            "files_count": len(result.files),
            "iterations": result.iterations,
            "confidence": result.confidence,
            "duration_ms": result.duration_ms,
            "model": result.model_used,
            "provider": result.provider_used,
            "timestamp": datetime.now().isoformat(),
            "errors": result.errors
        })

        with open(self.results_file, 'w') as f:
            json.dump(results, f, indent=2)

    def write_files_to_disk(self, result: ExecutionResult, output_dir: Path) -> List[str]:
        """Write generated files to disk."""
        written = []
        for file_info in result.files:
            path = output_dir / file_info["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(file_info["content"])
            written.append(str(path))
        return written


def run_claudia_packet(
    packet_id: Optional[str] = None,
    output_dir: Optional[str] = None,
    debug: bool = False
):
    """CLI entry point for executing Claudia packets."""
    bridge = ClaudiaBridge()

    # Get packets
    packets = bridge.get_pending_packets()
    if not packets:
        print("No pending packets in queue")
        return

    # Filter to specific packet if requested
    if packet_id:
        packets = [p for p in packets if p.id == packet_id]
        if not packets:
            print(f"Packet {packet_id} not found")
            return

    print(f"\nFound {len(packets)} packet(s) to execute")

    for packet in packets:
        print(f"\n{'='*60}")
        print(f"Executing: {packet.title}")
        print(f"Project: {packet.project_name}")
        print(f"Phase: {packet.phase}")
        print(f"{'='*60}")

        result = bridge.execute_packet(packet, debug=debug)
        bridge.save_result(result)

        if result.success:
            print(f"\n✅ Success: {len(result.files)} files generated")
            print(f"   Iterations: {result.iterations}")
            print(f"   Confidence: {result.confidence*100:.0f}%")
            print(f"   Duration: {result.duration_ms}ms")

            if output_dir:
                written = bridge.write_files_to_disk(result, Path(output_dir))
                print(f"   Written to: {output_dir}")
                for f in written[:5]:
                    print(f"     - {f}")
        else:
            print(f"\n❌ Failed: {', '.join(result.errors[:3])}")


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Execute Claudia packets via Ganesha")
    parser.add_argument("--packet", help="Specific packet ID to execute")
    parser.add_argument("--output", "-o", help="Output directory for generated files")
    parser.add_argument("--debug", action="store_true", help="Show debug output")

    args = parser.parse_args()
    run_claudia_packet(args.packet, args.output, args.debug)
