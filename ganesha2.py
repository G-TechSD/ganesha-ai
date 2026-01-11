#!/usr/bin/env python3
"""
Ganesha 2.0 - The Remover of Obstacles

Modernized version with:
- Local LLM support (LM Studio, Ollama)
- Cloud fallback (Anthropic, OpenAI)
- Provider chain with automatic failover
- Simplified dependencies
- Same powerful features as original

Usage:
    ganesha2 "install docker and configure it"
    ganesha2 --code "create a React login form"
    ganesha2 --provider local "optimize disk usage"
    ganesha2 --A "update all packages"  # Auto-approve
    ganesha2 --rollback last
    ganesha2 --interactive
"""

import argparse
import sys
import os
import json
import subprocess
import platform
import shutil
import tempfile
import threading
from datetime import datetime
from pathlib import Path
from typing import Optional, List, Dict, Any
import time
import shlex

# Try colorama, gracefully degrade if not available
try:
    from colorama import Fore, Style, init as colorama_init
    colorama_init(autoreset=True)
except ImportError:
    # Fallback: no colors
    class Fore:
        RED = CYAN = GREEN = YELLOW = WHITE = ""
    class Style:
        BRIGHT = RESET_ALL = ""

# Import our provider abstraction
from providers import (
    LLMResponse,
    LMStudioProvider,
    OllamaProvider,
    AnthropicProvider,
    OpenAIProvider,
    ProviderChain,
    create_default_chain
)

# ------------------------------ Configuration ------------------------------

VERSION = "2.0.0"
LOG_DIR = Path.home() / "ganesha_logs"
LOG_DIR.mkdir(exist_ok=True)

# Default timeouts
TIMEOUT_SECONDS = 300
MAX_RETRIES = 3

# Built-in commands per OS (don't need path checking)
BUILTIN_COMMANDS = {
    "Windows": ['move', 'copy', 'del', 'dir', 'echo', 'cls', 'cd', 'type', 'mkdir', 'rmdir'],
    "Linux": ['mv', 'cp', 'rm', 'ls', 'echo', 'clear', 'cd', 'cat', 'mkdir', 'rmdir'],
    "Darwin": ['mv', 'cp', 'rm', 'ls', 'echo', 'clear', 'cd', 'cat', 'mkdir', 'rmdir']
}

# ------------------------------ Utilities ------------------------------

class Spinner:
    """Simple spinner for long-running operations."""
    def __init__(self, message: str = "Processing"):
        self.chars = ['|', '/', '-', '\\']
        self.idx = 0
        self.running = False
        self.thread = None
        self.message = message

    def start(self):
        self.running = True
        self.thread = threading.Thread(target=self._spin, daemon=True)
        self.thread.start()

    def _spin(self):
        while self.running:
            char = self.chars[self.idx % len(self.chars)]
            print(f"\r{self.message}... {char}", end='', flush=True)
            self.idx += 1
            time.sleep(0.1)

    def stop(self):
        self.running = False
        if self.thread:
            self.thread.join(timeout=0.5)
        print("\r" + " " * (len(self.message) + 10) + "\r", end='')


def gather_system_info() -> Dict[str, Any]:
    """Gather system information to provide context to the LLM."""
    info = {
        "os": platform.system(),
        "os_version": platform.version(),
        "platform": platform.platform(),
        "arch": platform.machine(),
        "hostname": platform.node(),
        "python": platform.python_version(),
        "processor": platform.processor(),
        "cwd": os.getcwd()
    }

    # Add memory/disk info if psutil available
    try:
        import psutil
        mem = psutil.virtual_memory()
        info["ram_total_gb"] = round(mem.total / (1024**3), 2)
        info["ram_available_gb"] = round(mem.available / (1024**3), 2)
        disk = shutil.disk_usage("/")
        info["disk_total_gb"] = round(disk.total / (1024**3), 2)
        info["disk_free_gb"] = round(disk.free / (1024**3), 2)
    except ImportError:
        pass

    return info


def extract_json(text: str) -> Optional[Dict]:
    """Extract JSON from LLM response text."""
    try:
        # Try to find JSON block
        start = text.find('{')
        end = text.rfind('}') + 1
        if start >= 0 and end > start:
            return json.loads(text[start:end])
    except json.JSONDecodeError:
        pass

    # Try to find in code block
    try:
        if "```json" in text:
            json_str = text.split("```json")[1].split("```")[0].strip()
            return json.loads(json_str)
        elif "```" in text:
            json_str = text.split("```")[1].split("```")[0].strip()
            return json.loads(json_str)
    except (IndexError, json.JSONDecodeError):
        pass

    return None


# ------------------------------ LLM Interface ------------------------------

def generate_commands(
    task: str,
    provider_chain: ProviderChain,
    system_info: Dict[str, Any],
    conversation_history: Optional[List[Dict]] = None,
    debug: bool = False
) -> Optional[Dict]:
    """
    Send task to LLM and get commands to execute.

    Returns dict with 'commands' or 'command' key, or None on failure.
    """
    system_prompt = f"""You are a cross-platform system expert. Your task is to translate natural language requests into executable commands.

SYSTEM INFORMATION:
{json.dumps(system_info, indent=2)}

RESPONSE FORMAT (JSON only):
{{
  "commands": [
    {{"command": "actual command here", "explanation": "what this does"}}
  ]
}}

RULES:
- Provide ONLY valid JSON, no other text
- Use appropriate commands for the user's OS ({system_info['os']})
- Each command must have 'command' and 'explanation' keys
- Commands should be safe and reversible when possible
- Handle dependencies appropriately
- If task is complete or just informational, use: {{"task_complete": true, "message": "..."}}
"""

    # Build conversation
    messages_context = ""
    if conversation_history:
        for msg in conversation_history[-5:]:  # Last 5 messages
            messages_context += f"\n{msg['role'].upper()}: {msg['content']}"

    user_prompt = f"{messages_context}\n\nUSER REQUEST: {task}"

    spinner = Spinner("Thinking")
    spinner.start()

    response = provider_chain.generate(
        system_prompt=system_prompt,
        user_prompt=user_prompt,
        temperature=0.3,
        max_tokens=2000
    )

    spinner.stop()

    if debug:
        print(f"\n{Style.BRIGHT}[DEBUG] Provider: {response.provider}, Model: {response.model}{Style.RESET_ALL}")
        print(f"{Style.BRIGHT}[DEBUG] Raw response:{Style.RESET_ALL}\n{response.content}\n")

    if response.error:
        print(f"{Fore.RED}Error: {response.error}{Style.RESET_ALL}")
        return None

    parsed = extract_json(response.content)
    if not parsed:
        print(f"{Fore.RED}Could not parse response as JSON{Style.RESET_ALL}")
        if debug:
            print(f"Response was: {response.content[:500]}")
        return None

    return parsed


# ------------------------------ Command Execution ------------------------------

def execute_commands(
    commands: List[Dict],
    logger: Any,
    allow_all: bool = False,
    debug: bool = False
) -> tuple[bool, str]:
    """
    Execute a list of commands with user confirmation.

    Returns (success, error_message)
    """
    executed = []
    all_confirmed = allow_all

    for i, cmd_info in enumerate(commands, 1):
        if isinstance(cmd_info, dict):
            cmd = cmd_info.get('command', '')
            explanation = cmd_info.get('explanation', 'No explanation')
        else:
            cmd = str(cmd_info)
            explanation = "No explanation"

        total = len(commands)
        print(f"\n{Fore.CYAN}Command {i}/{total}: {cmd}{Style.RESET_ALL}")
        print(f"{Fore.GREEN}Explanation: {explanation}{Style.RESET_ALL}")

        if not all_confirmed:
            choice = input("Execute? (yes/no/all): ").strip().lower()
            if choice == 'all':
                all_confirmed = True
            elif not choice.startswith('yes'):
                print(f"{Fore.YELLOW}Skipped{Style.RESET_ALL}")
                continue

        try:
            result = subprocess.run(
                cmd,
                shell=True,
                capture_output=True,
                text=True,
                timeout=TIMEOUT_SECONDS
            )

            if result.returncode != 0:
                print(f"{Fore.RED}Command failed (exit {result.returncode}){Style.RESET_ALL}")
                if result.stderr:
                    print(f"{Fore.RED}{result.stderr[:500]}{Style.RESET_ALL}")
                return False, result.stderr or f"Exit code {result.returncode}"

            if result.stdout:
                print(result.stdout)

            executed.append(cmd)
            logger.log({"executed": cmd, "success": True})

        except subprocess.TimeoutExpired:
            print(f"{Fore.RED}Command timed out{Style.RESET_ALL}")
            return False, "Timeout"
        except Exception as e:
            print(f"{Fore.RED}Error: {e}{Style.RESET_ALL}")
            return False, str(e)

    return True, ""


# ------------------------------ Session Logging ------------------------------

class SessionLogger:
    """Simple JSON-lines logger for sessions."""

    def __init__(self, session_id: str):
        self.session_id = session_id
        self.log_file = LOG_DIR / f"session_{session_id}.log"
        self.executed_commands = []

    def log(self, data: Dict):
        """Append log entry."""
        entry = {
            "timestamp": datetime.now().isoformat(),
            "session_id": self.session_id,
            **data
        }
        with open(self.log_file, 'a') as f:
            f.write(json.dumps(entry) + "\n")

        if "executed" in data:
            self.executed_commands.append(data["executed"])

    def save_final(self, task: str, system_info: Dict):
        """Save final session state."""
        self.log({
            "action": "complete",
            "task": task,
            "executed_commands": self.executed_commands,
            "system_info": system_info
        })


def rollback_session(session_id: str, provider_chain: ProviderChain, allow_all: bool = False):
    """Rollback a previous session's commands."""
    if session_id == 'last':
        # Find most recent log
        logs = sorted(LOG_DIR.glob("session_*.log"), reverse=True)
        if not logs:
            print(f"{Fore.RED}No session logs found{Style.RESET_ALL}")
            return
        log_file = logs[0]
    else:
        log_file = LOG_DIR / f"session_{session_id}.log"

    if not log_file.exists():
        print(f"{Fore.RED}Session log not found: {log_file}{Style.RESET_ALL}")
        return

    # Find executed commands
    executed = []
    with open(log_file) as f:
        for line in f:
            try:
                entry = json.loads(line)
                if "executed_commands" in entry:
                    executed = entry["executed_commands"]
                elif "executed" in entry:
                    executed.append(entry["executed"])
            except json.JSONDecodeError:
                continue

    if not executed:
        print(f"{Fore.YELLOW}No executed commands found in session{Style.RESET_ALL}")
        return

    print(f"\n{Style.BRIGHT}Commands to rollback:{Style.RESET_ALL}")
    for cmd in executed:
        print(f"  {cmd}")

    # Ask LLM for inverse commands
    system_info = gather_system_info()
    task = f"Generate rollback/inverse commands for these executed commands:\n" + "\n".join(executed)

    result = generate_commands(task, provider_chain, system_info)
    if result and 'commands' in result:
        print(f"\n{Style.BRIGHT}Rollback commands:{Style.RESET_ALL}")
        logger = SessionLogger(f"rollback_{datetime.now().strftime('%Y%m%d%H%M%S')}")
        execute_commands(result['commands'], logger, allow_all=allow_all)


# ------------------------------ Interactive Mode ------------------------------

def interactive_mode(provider_chain: ProviderChain, debug: bool = False):
    """Interactive CLI mode."""
    print(f"\n{Style.BRIGHT}Ganesha Interactive Mode{Style.RESET_ALL}")
    print("Type your requests, 'exit' to quit, 'rollback' to undo\n")

    session_id = datetime.now().strftime('%Y%m%d%H%M%S')
    logger = SessionLogger(session_id)
    system_info = gather_system_info()
    conversation: List[Dict] = []

    while True:
        try:
            task = input(f"{Fore.CYAN}ganesha> {Style.RESET_ALL}").strip()
        except (EOFError, KeyboardInterrupt):
            print("\nExiting...")
            break

        if not task:
            continue
        if task.lower() in ('exit', 'quit', 'q'):
            break
        if task.lower() == 'rollback':
            rollback_session('last', provider_chain)
            continue

        conversation.append({"role": "user", "content": task})
        result = generate_commands(task, provider_chain, system_info, conversation, debug)

        if not result:
            continue

        if result.get('task_complete'):
            print(f"\n{Fore.GREEN}{result.get('message', 'Task complete')}{Style.RESET_ALL}\n")
            conversation.append({"role": "assistant", "content": result.get('message', '')})
            continue

        commands = result.get('commands', [])
        if result.get('command'):
            commands = [{"command": result['command'], "explanation": result.get('explanation', '')}]

        if commands:
            success, _ = execute_commands(commands, logger)
            logger.log({"task": task, "success": success})

    logger.save_final("interactive", system_info)
    print(f"{Fore.GREEN}Session saved: {session_id}{Style.RESET_ALL}")


# ------------------------------ Main ------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Ganesha 2.0 - AI-Powered Cross-Platform Command Executor",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  ganesha2 "install docker"
  ganesha2 --A "update all packages"       # Auto-approve
  ganesha2 --provider local "find large files"
  ganesha2 --rollback last
  ganesha2 --interactive
        """
    )

    parser.add_argument('task', nargs='*', help='Task in plain English')
    parser.add_argument('--A', action='store_true', help='Auto-approve all commands')
    parser.add_argument('--debug', action='store_true', help='Show debug output')
    parser.add_argument('--interactive', action='store_true', help='Interactive mode')
    parser.add_argument('--rollback', nargs='?', const='last', help='Rollback session')
    parser.add_argument('--provider', choices=['local', 'cloud', 'anthropic', 'openai'],
                       default='local', help='LLM provider preference')
    parser.add_argument('--version', action='version', version=f'Ganesha {VERSION}')

    args = parser.parse_args()

    # Configure provider chain based on preference
    if args.provider == 'local':
        chain = create_default_chain()
    elif args.provider == 'anthropic':
        chain = ProviderChain([AnthropicProvider()])
    elif args.provider == 'openai':
        chain = ProviderChain([OpenAIProvider()])
    else:
        chain = create_default_chain()

    # Check for available providers
    available = chain.get_available_providers()
    if not available:
        print(f"{Fore.RED}No LLM providers available!{Style.RESET_ALL}")
        print("Configure LM Studio, Ollama, or set ANTHROPIC_API_KEY/OPENAI_API_KEY")
        sys.exit(1)

    if args.debug:
        print(f"{Style.BRIGHT}Available providers:{Style.RESET_ALL}")
        for p in available:
            url = getattr(p, 'url', 'cloud')
            print(f"  - {p.name}: {url}")
        print()

    # Handle modes
    if args.interactive:
        interactive_mode(chain, args.debug)
        return

    if args.rollback:
        rollback_session(args.rollback, chain, args.A)
        return

    if not args.task:
        parser.print_help()
        return

    # Execute task
    task = ' '.join(args.task)
    session_id = datetime.now().strftime('%Y%m%d%H%M%S')
    logger = SessionLogger(session_id)
    system_info = gather_system_info()

    logger.log({"action": "start", "task": task, "system_info": system_info})

    result = generate_commands(task, chain, system_info, debug=args.debug)

    if not result:
        sys.exit(1)

    if result.get('task_complete'):
        print(f"\n{Fore.GREEN}{result.get('message', 'Task complete')}{Style.RESET_ALL}")
        sys.exit(0)

    commands = result.get('commands', [])
    if result.get('command'):
        commands = [{"command": result['command'], "explanation": result.get('explanation', '')}]

    if not commands:
        print(f"{Fore.YELLOW}No commands generated{Style.RESET_ALL}")
        sys.exit(0)

    print(f"\n{Style.BRIGHT}Generated {len(commands)} command(s):{Style.RESET_ALL}")
    success, error = execute_commands(commands, logger, allow_all=args.A, debug=args.debug)

    logger.save_final(task, system_info)

    if success:
        print(f"\n{Fore.GREEN}Completed successfully{Style.RESET_ALL}")
    else:
        print(f"\n{Fore.RED}Failed: {error}{Style.RESET_ALL}")
        sys.exit(1)


if __name__ == "__main__":
    main()
