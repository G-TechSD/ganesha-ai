#!/usr/bin/env python3
"""
Ganesha CLI - The Remover of Obstacles

A command-line interface that brings the power of AI to system control.
Local-first, safe by default, with the aesthetic of old-school BBS.

Usage:
    ganesha "install docker and configure it"
    ganesha --code "create a React component"
    ganesha --auto "update all packages"
    ganesha --rollback
    ganesha --interactive
"""

import argparse
import asyncio
import sys
from pathlib import Path
from typing import List, Optional

# Terminal styling - graceful degradation
try:
    from colorama import Fore, Style, Back, init as colorama_init
    colorama_init(autoreset=True)
    HAS_COLOR = True
except ImportError:
    class Fore:
        RED = GREEN = YELLOW = CYAN = MAGENTA = WHITE = BLUE = ""
    class Style:
        BRIGHT = DIM = RESET_ALL = ""
    class Back:
        BLACK = ""
    HAS_COLOR = False


# ═══════════════════════════════════════════════════════════════════════════
# ASCII ART & BRANDING
# ═══════════════════════════════════════════════════════════════════════════

BANNER = f"""{Fore.CYAN}{Style.BRIGHT}
 ██████╗  █████╗ ███╗   ██╗███████╗███████╗██╗  ██╗ █████╗
██╔════╝ ██╔══██╗████╗  ██║██╔════╝██╔════╝██║  ██║██╔══██╗
██║  ███╗███████║██╔██╗ ██║█████╗  ███████╗███████║███████║
██║   ██║██╔══██║██║╚██╗██║██╔══╝  ╚════██║██╔══██║██╔══██║
╚██████╔╝██║  ██║██║ ╚████║███████╗███████║██║  ██║██║  ██║
 ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝
{Style.RESET_ALL}{Fore.YELLOW}         ══════ The Remover of Obstacles ══════{Style.RESET_ALL}
{Fore.WHITE}{Style.DIM}              Local AI • Safe by Default • v3.0{Style.RESET_ALL}
"""

MINI_BANNER = f"{Fore.CYAN}◆ GANESHA{Style.RESET_ALL}"

DIVIDER = f"{Fore.CYAN}{'─' * 60}{Style.RESET_ALL}"
DOUBLE_DIVIDER = f"{Fore.CYAN}{'═' * 60}{Style.RESET_ALL}"


# ═══════════════════════════════════════════════════════════════════════════
# CONSOLE OUTPUT HELPERS
# ═══════════════════════════════════════════════════════════════════════════

def print_banner(mini: bool = False):
    """Print the Ganesha banner."""
    if mini:
        print(MINI_BANNER)
    else:
        print(BANNER)


def print_info(msg: str):
    print(f"{Fore.CYAN}ℹ{Style.RESET_ALL} {msg}")


def print_success(msg: str):
    print(f"{Fore.GREEN}✓{Style.RESET_ALL} {msg}")


def print_error(msg: str):
    print(f"{Fore.RED}✗{Style.RESET_ALL} {msg}")


def print_warning(msg: str):
    print(f"{Fore.YELLOW}⚠{Style.RESET_ALL} {msg}")


def print_action(index: int, total: int, command: str, explanation: str, risk: str = "low"):
    """Print an action for user review."""
    risk_colors = {
        "low": Fore.GREEN,
        "medium": Fore.YELLOW,
        "high": Fore.RED,
        "critical": f"{Fore.RED}{Style.BRIGHT}",
    }
    risk_color = risk_colors.get(risk, Fore.WHITE)

    print(f"\n{Fore.CYAN}[{index}/{total}]{Style.RESET_ALL} {risk_color}[{risk.upper()}]{Style.RESET_ALL}")
    print(f"{Fore.WHITE}{Style.BRIGHT}Command:{Style.RESET_ALL} {command}")
    print(f"{Fore.WHITE}{Style.DIM}Explanation:{Style.RESET_ALL} {explanation}")


def print_result(success: bool, output: str, error: str, duration_ms: int):
    """Print execution result."""
    if success:
        print_success(f"Completed in {duration_ms}ms")
        if output:
            # Truncate long output
            lines = output.strip().split('\n')
            if len(lines) > 10:
                print('\n'.join(lines[:10]))
                print(f"{Style.DIM}... ({len(lines) - 10} more lines){Style.RESET_ALL}")
            else:
                print(output.strip())
    else:
        print_error(f"Failed: {error}")


# ═══════════════════════════════════════════════════════════════════════════
# USER CONSENT SYSTEM
# ═══════════════════════════════════════════════════════════════════════════

class CLIConsentHandler:
    """
    Handle user consent through the terminal.

    Supports:
    - Individual action approval
    - Approve all remaining
    - Skip action
    - Cancel execution
    - Modify action (reply to LLM)
    """

    def __init__(self, auto_approve: bool = False):
        self.auto_approve = auto_approve

    async def request_consent(self, plan, auto_approve: bool = False) -> tuple[bool, List[str]]:
        """Request user consent for an execution plan."""
        if auto_approve or self.auto_approve:
            return True, [a.id for a in plan.actions]

        approved_ids = []
        all_approved = False

        print(f"\n{DOUBLE_DIVIDER}")
        print(f"{Fore.CYAN}{Style.BRIGHT}EXECUTION PLAN{Style.RESET_ALL}")
        print(f"{Fore.WHITE}Task: {plan.task}{Style.RESET_ALL}")
        print(f"{Fore.WHITE}Actions: {plan.total_actions}{Style.RESET_ALL}")

        high_risk = plan.high_risk_actions
        if high_risk:
            print(f"{Fore.RED}{Style.BRIGHT}⚠ {len(high_risk)} HIGH RISK action(s){Style.RESET_ALL}")

        print(DIVIDER)

        for i, action in enumerate(plan.actions, 1):
            print_action(i, plan.total_actions, action.command, action.explanation, action.risk_level)

            if all_approved:
                approved_ids.append(action.id)
                print(f"{Fore.GREEN}Auto-approved{Style.RESET_ALL}")
                continue

            # Get user input
            print(f"\n{Fore.CYAN}Execute? {Style.RESET_ALL}[{Fore.GREEN}y{Style.RESET_ALL}es / {Fore.RED}n{Style.RESET_ALL}o / {Fore.YELLOW}a{Style.RESET_ALL}ll / {Fore.RED}c{Style.RESET_ALL}ancel]: ", end="")

            try:
                choice = input().strip().lower()
            except (EOFError, KeyboardInterrupt):
                print(f"\n{Fore.RED}Cancelled{Style.RESET_ALL}")
                return False, []

            if choice in ('y', 'yes', ''):
                approved_ids.append(action.id)
                print_success("Approved")
            elif choice in ('a', 'all'):
                all_approved = True
                approved_ids.append(action.id)
                print_success("Approved all remaining")
            elif choice in ('c', 'cancel', 'q', 'quit'):
                print_warning("Execution cancelled")
                return False, []
            else:
                print_warning("Skipped")

        if not approved_ids:
            print_warning("No actions approved")
            return False, []

        print(f"\n{DIVIDER}")
        print_info(f"Executing {len(approved_ids)}/{plan.total_actions} actions...")
        return True, approved_ids


# ═══════════════════════════════════════════════════════════════════════════
# INTERACTIVE MODE
# ═══════════════════════════════════════════════════════════════════════════

async def interactive_mode(engine, auto_approve: bool = False):
    """Run Ganesha in interactive REPL mode."""
    print_banner()
    print(f"{Fore.WHITE}Type your requests in plain English. Commands:{Style.RESET_ALL}")
    print(f"  {Fore.CYAN}exit{Style.RESET_ALL}     - Quit")
    print(f"  {Fore.CYAN}rollback{Style.RESET_ALL} - Undo last session")
    print(f"  {Fore.CYAN}history{Style.RESET_ALL}  - Show session history")
    print(f"  {Fore.CYAN}auto{Style.RESET_ALL}     - Toggle auto-approve mode")
    print(DIVIDER)

    while True:
        try:
            print(f"\n{Fore.CYAN}ganesha>{Style.RESET_ALL} ", end="")
            task = input().strip()
        except (EOFError, KeyboardInterrupt):
            print(f"\n{Fore.CYAN}Farewell, obstacle-free travels!{Style.RESET_ALL}")
            break

        if not task:
            continue

        task_lower = task.lower()

        if task_lower in ('exit', 'quit', 'q'):
            print(f"{Fore.CYAN}Farewell, obstacle-free travels!{Style.RESET_ALL}")
            break

        elif task_lower == 'rollback':
            print_info("Rolling back last session...")
            async for result in engine.rollback():
                print_result(result.success, result.output, result.error, result.duration_ms)

        elif task_lower == 'history':
            await show_history(engine)

        elif task_lower == 'auto':
            engine.auto_approve = not engine.auto_approve
            state = "ON" if engine.auto_approve else "OFF"
            color = Fore.YELLOW if engine.auto_approve else Fore.GREEN
            print(f"{color}Auto-approve: {state}{Style.RESET_ALL}")
            if engine.auto_approve:
                print_warning("Commands will execute without confirmation!")

        else:
            # Execute task
            async for result in engine.execute(task):
                print_result(result.success, result.output, result.error, result.duration_ms)


async def show_history(engine):
    """Show session history."""
    sessions = sorted(engine.session_dir.glob("*.json"), reverse=True)[:10]

    if not sessions:
        print_info("No session history")
        return

    print(f"\n{DIVIDER}")
    print(f"{Fore.CYAN}{Style.BRIGHT}RECENT SESSIONS{Style.RESET_ALL}")
    print(DIVIDER)

    import json
    for i, session_file in enumerate(sessions, 1):
        try:
            data = json.loads(session_file.read_text())
            task = data.get("task", "Unknown")[:50]
            state = data.get("state", "unknown")
            actions = len(data.get("executed_actions", []))
            session_id = data.get("id", session_file.stem)

            state_color = Fore.GREEN if state == "completed" else Fore.RED if state == "failed" else Fore.YELLOW
            print(f"{i}. {Fore.WHITE}{session_id}{Style.RESET_ALL}")
            print(f"   Task: {task}...")
            print(f"   State: {state_color}{state}{Style.RESET_ALL} | Actions: {actions}")
        except Exception:
            pass


# ═══════════════════════════════════════════════════════════════════════════
# MAIN ENTRY POINT
# ═══════════════════════════════════════════════════════════════════════════

def main():
    parser = argparse.ArgumentParser(
        description="Ganesha - The Remover of Obstacles",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=f"""
{Fore.CYAN}Examples:{Style.RESET_ALL}
  ganesha "install docker"
  ganesha --auto "update all packages"
  ganesha --code "create a login form component"
  ganesha --rollback
  ganesha --interactive

{Fore.YELLOW}The first AI-powered system control tool.{Style.RESET_ALL}
{Fore.WHITE}Predates Claude Code & OpenAI Codex CLI.{Style.RESET_ALL}
        """
    )

    parser.add_argument('task', nargs='*', help='Task in plain English')
    parser.add_argument('--auto', '-A', action='store_true',
                       help='Auto-approve all commands (DANGEROUS)')
    parser.add_argument('--code', action='store_true',
                       help='Code generation mode')
    parser.add_argument('--interactive', '-i', action='store_true',
                       help='Interactive REPL mode')
    parser.add_argument('--rollback', '-r', nargs='?', const='last',
                       help='Rollback session')
    parser.add_argument('--history', action='store_true',
                       help='Show session history')
    parser.add_argument('--provider', choices=['local', 'anthropic', 'openai'],
                       default='local', help='LLM provider')
    parser.add_argument('--debug', action='store_true',
                       help='Show debug output')
    parser.add_argument('--quiet', '-q', action='store_true',
                       help='Minimal output')
    parser.add_argument('--version', '-v', action='version', version='Ganesha 3.0')

    args = parser.parse_args()

    # Import engine components
    try:
        sys.path.insert(0, str(Path(__file__).parent.parent.parent))
        from ganesha.core.engine import GaneshaEngine
        from ganesha.providers.llm import create_provider_chain
    except ImportError:
        # Fallback for direct execution
        sys.path.insert(0, str(Path(__file__).parent.parent.parent))
        from providers import create_default_chain as create_provider_chain

        # Minimal engine for testing
        class GaneshaEngine:
            def __init__(self, *args, **kwargs):
                self.auto_approve = kwargs.get('auto_approve', False)
                self.session_dir = Path.home() / ".ganesha" / "sessions"
                self.session_dir.mkdir(parents=True, exist_ok=True)

    # Create provider chain
    provider_chain = create_provider_chain()

    # Check providers
    available = provider_chain.get_available_providers()
    if not available:
        print_error("No LLM providers available!")
        print_info("Configure LM Studio, Ollama, or set API keys")
        sys.exit(1)

    # Create consent handler
    consent = CLIConsentHandler(auto_approve=args.auto)

    # Create engine
    from ganesha.providers.llm import AsyncProviderWrapper
    engine = GaneshaEngine(
        llm_provider=AsyncProviderWrapper(provider_chain),
        consent_handler=consent,
        auto_approve=args.auto,
    )

    # Show banner unless quiet
    if not args.quiet:
        print_banner(mini=bool(args.task))

    # Handle modes
    if args.interactive:
        asyncio.run(interactive_mode(engine, args.auto))
        return

    if args.rollback:
        asyncio.run(do_rollback(engine, args.rollback))
        return

    if args.history:
        asyncio.run(show_history(engine))
        return

    if not args.task:
        parser.print_help()
        return

    # Execute task
    task = ' '.join(args.task)
    if args.code:
        task = f"[CODE MODE] {task}"

    if not args.quiet:
        print_info(f"Task: {task}")
        print_info(f"Provider: {available[0].name}")

    asyncio.run(do_execute(engine, task))


async def do_execute(engine, task: str):
    """Execute a task."""
    try:
        async for result in engine.execute(task):
            print_result(result.success, result.output, result.error, result.duration_ms)
    except Exception as e:
        print_error(str(e))
        sys.exit(1)


async def do_rollback(engine, session_id: str):
    """Rollback a session."""
    print_info(f"Rolling back session: {session_id}")
    try:
        async for result in engine.rollback(session_id if session_id != 'last' else None):
            print_result(result.success, result.output, result.error, result.duration_ms)
    except Exception as e:
        print_error(str(e))


if __name__ == "__main__":
    main()
