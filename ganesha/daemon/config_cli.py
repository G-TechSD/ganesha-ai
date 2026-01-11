"""
Ganesha Daemon Configuration CLI

Manage access control policies for the privileged daemon.

Usage:
    ganesha-config show                      # Show current config
    ganesha-config set-level standard        # Set access level
    ganesha-config whitelist add "apt *"     # Add whitelist pattern
    ganesha-config blacklist add "rm -rf *"  # Add blacklist pattern
    ganesha-config preset                    # Interactive preset selector
"""

import argparse
import json
import sys
from pathlib import Path

try:
    from colorama import Fore, Style, init
    init()
    HAS_COLOR = True
except ImportError:
    HAS_COLOR = False
    class Fore:
        RED = GREEN = YELLOW = CYAN = MAGENTA = WHITE = ""
    class Style:
        BRIGHT = RESET_ALL = DIM = ""

from .access_control import (
    AccessLevel,
    AccessPolicy,
    load_policy,
    save_policy,
    PRESETS,
    ALWAYS_DENIED,
    DEFAULT_CONFIG,
)


CONFIG_PATH = Path.home() / ".ganesha" / "privilege.json"
SYSTEM_CONFIG_PATH = Path("/etc/ganesha/privilege.json")


def print_banner():
    """Print configuration banner."""
    print(f"""
{Fore.CYAN}{Style.BRIGHT}╔═══════════════════════════════════════════════════════════════╗
║           GANESHA ACCESS CONTROL CONFIGURATION                ║
╚═══════════════════════════════════════════════════════════════╝{Style.RESET_ALL}
""")


def print_presets():
    """Print available presets with descriptions."""
    print(f"\n{Fore.YELLOW}Available Access Level Presets:{Style.RESET_ALL}\n")

    preset_info = [
        ("restricted", "Read-only commands only. Cannot modify system.",
         Fore.GREEN, "SAFE"),
        ("standard", "Common sysadmin tasks. Safe modifications.",
         Fore.CYAN, "RECOMMENDED"),
        ("elevated", "Package management, service control.",
         Fore.YELLOW, "MODERATE RISK"),
        ("full_access", "Full system access. Very dangerous!",
         Fore.RED, "DANGEROUS"),
        ("whitelist", "Only explicitly allowed commands.",
         Fore.GREEN, "CUSTOM"),
        ("blacklist", "Everything except explicitly denied.",
         Fore.YELLOW, "CUSTOM"),
    ]

    for level, desc, color, badge in preset_info:
        print(f"  {color}{Style.BRIGHT}[{badge}]{Style.RESET_ALL} {Style.BRIGHT}{level}{Style.RESET_ALL}")
        print(f"         {Style.DIM}{desc}{Style.RESET_ALL}")
        print()


def show_config(args):
    """Show current configuration."""
    print_banner()

    # Try to load config
    config_path = SYSTEM_CONFIG_PATH if SYSTEM_CONFIG_PATH.exists() else CONFIG_PATH
    policy = load_policy(config_path if config_path.exists() else None)

    print(f"{Fore.CYAN}Current Configuration:{Style.RESET_ALL}")
    print(f"  Config file: {config_path if config_path.exists() else 'default'}")
    print()

    # Access level
    level_colors = {
        AccessLevel.RESTRICTED: Fore.GREEN,
        AccessLevel.STANDARD: Fore.CYAN,
        AccessLevel.ELEVATED: Fore.YELLOW,
        AccessLevel.FULL_ACCESS: Fore.RED,
        AccessLevel.WHITELIST: Fore.GREEN,
        AccessLevel.BLACKLIST: Fore.YELLOW,
    }
    color = level_colors.get(policy.level, Fore.WHITE)
    print(f"  {Fore.WHITE}Access Level:{Style.RESET_ALL} {color}{Style.BRIGHT}{policy.level.value}{Style.RESET_ALL}")

    # Settings
    print(f"\n{Fore.CYAN}Settings:{Style.RESET_ALL}")
    print(f"  Require approval for high risk: {policy.require_approval_for_high_risk}")
    print(f"  Audit all commands: {policy.audit_all_commands}")
    print(f"  Max execution time: {policy.max_execution_time}s")

    # Whitelist
    if policy.whitelist:
        print(f"\n{Fore.GREEN}Whitelist patterns:{Style.RESET_ALL}")
        for pattern in policy.whitelist:
            print(f"  + {pattern}")

    # Blacklist
    if policy.blacklist:
        print(f"\n{Fore.RED}Blacklist patterns:{Style.RESET_ALL}")
        for pattern in policy.blacklist:
            print(f"  - {pattern}")

    # Always denied
    print(f"\n{Fore.RED}Always Denied (hardcoded):{Style.RESET_ALL}")
    print(f"  {len(ALWAYS_DENIED)} security-critical patterns")
    print(f"  {Style.DIM}(rm -rf /, fork bombs, disk wiping, etc.){Style.RESET_ALL}")


def set_level(args):
    """Set access level."""
    try:
        level = AccessLevel(args.level)
    except ValueError:
        print(f"{Fore.RED}Invalid level: {args.level}{Style.RESET_ALL}")
        print_presets()
        return

    config_path = CONFIG_PATH if not args.system else SYSTEM_CONFIG_PATH
    policy = load_policy(config_path if config_path.exists() else None)
    policy.level = level
    save_policy(policy, config_path)

    color = {
        AccessLevel.RESTRICTED: Fore.GREEN,
        AccessLevel.STANDARD: Fore.CYAN,
        AccessLevel.ELEVATED: Fore.YELLOW,
        AccessLevel.FULL_ACCESS: Fore.RED,
    }.get(level, Fore.WHITE)

    print(f"{Fore.GREEN}Access level set to:{Style.RESET_ALL} {color}{Style.BRIGHT}{level.value}{Style.RESET_ALL}")
    print(f"Config saved to: {config_path}")

    if level == AccessLevel.FULL_ACCESS:
        print(f"\n{Fore.RED}{Style.BRIGHT}⚠ WARNING: Full access mode is DANGEROUS!")
        print(f"All commands will be allowed except hardcoded security blocks.{Style.RESET_ALL}")


def add_whitelist(args):
    """Add whitelist pattern."""
    config_path = CONFIG_PATH if not args.system else SYSTEM_CONFIG_PATH
    policy = load_policy(config_path if config_path.exists() else None)

    if args.pattern not in policy.whitelist:
        policy.whitelist.append(args.pattern)
        save_policy(policy, config_path)
        print(f"{Fore.GREEN}Added to whitelist:{Style.RESET_ALL} {args.pattern}")
    else:
        print(f"Pattern already in whitelist: {args.pattern}")


def remove_whitelist(args):
    """Remove whitelist pattern."""
    config_path = CONFIG_PATH if not args.system else SYSTEM_CONFIG_PATH
    policy = load_policy(config_path if config_path.exists() else None)

    if args.pattern in policy.whitelist:
        policy.whitelist.remove(args.pattern)
        save_policy(policy, config_path)
        print(f"{Fore.YELLOW}Removed from whitelist:{Style.RESET_ALL} {args.pattern}")
    else:
        print(f"Pattern not in whitelist: {args.pattern}")


def add_blacklist(args):
    """Add blacklist pattern."""
    config_path = CONFIG_PATH if not args.system else SYSTEM_CONFIG_PATH
    policy = load_policy(config_path if config_path.exists() else None)

    if args.pattern not in policy.blacklist:
        policy.blacklist.append(args.pattern)
        save_policy(policy, config_path)
        print(f"{Fore.RED}Added to blacklist:{Style.RESET_ALL} {args.pattern}")
    else:
        print(f"Pattern already in blacklist: {args.pattern}")


def remove_blacklist(args):
    """Remove blacklist pattern."""
    config_path = CONFIG_PATH if not args.system else SYSTEM_CONFIG_PATH
    policy = load_policy(config_path if config_path.exists() else None)

    if args.pattern in policy.blacklist:
        policy.blacklist.remove(args.pattern)
        save_policy(policy, config_path)
        print(f"{Fore.GREEN}Removed from blacklist:{Style.RESET_ALL} {args.pattern}")
    else:
        print(f"Pattern not in blacklist: {args.pattern}")


def test_command(args):
    """Test if a command would be allowed."""
    from .access_control import AccessController

    policy = load_policy()
    controller = AccessController(policy)

    allowed, risk_level, reason = controller.check_command(args.command)

    print(f"\n{Fore.CYAN}Testing command:{Style.RESET_ALL} {args.command}")
    print(f"Current policy: {policy.level.value}")
    print()

    if allowed:
        color = {
            "low": Fore.GREEN,
            "medium": Fore.YELLOW,
            "high": Fore.RED,
        }.get(risk_level, Fore.WHITE)
        print(f"{Fore.GREEN}{Style.BRIGHT}✓ ALLOWED{Style.RESET_ALL} [{color}{risk_level}{Style.RESET_ALL}]")
    else:
        print(f"{Fore.RED}{Style.BRIGHT}✗ DENIED{Style.RESET_ALL}")

    print(f"Reason: {reason}")


def interactive_preset(args):
    """Interactive preset selector."""
    print_banner()
    print_presets()

    print(f"{Fore.CYAN}Select a preset (enter number or name):{Style.RESET_ALL}")
    print("  1. restricted")
    print("  2. standard")
    print("  3. elevated")
    print("  4. full_access")
    print("  5. whitelist")
    print("  6. blacklist")
    print()

    try:
        choice = input(f"{Fore.YELLOW}> {Style.RESET_ALL}").strip().lower()
    except (KeyboardInterrupt, EOFError):
        print("\nCancelled")
        return

    level_map = {
        "1": "restricted", "restricted": "restricted",
        "2": "standard", "standard": "standard",
        "3": "elevated", "elevated": "elevated",
        "4": "full_access", "full_access": "full_access", "full": "full_access",
        "5": "whitelist", "whitelist": "whitelist",
        "6": "blacklist", "blacklist": "blacklist",
    }

    if choice not in level_map:
        print(f"{Fore.RED}Invalid choice{Style.RESET_ALL}")
        return

    # Create args object for set_level
    class Args:
        pass
    args = Args()
    args.level = level_map[choice]
    args.system = False

    set_level(args)


def reset_config(args):
    """Reset configuration to defaults."""
    config_path = CONFIG_PATH if not args.system else SYSTEM_CONFIG_PATH

    if config_path.exists():
        config_path.unlink()

    print(f"{Fore.GREEN}Configuration reset to defaults{Style.RESET_ALL}")
    print(f"Deleted: {config_path}")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Ganesha Daemon Access Control Configuration",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  ganesha-config show                      Show current configuration
  ganesha-config set-level standard        Set to standard access
  ganesha-config set-level restricted      Set to restricted (safe) mode
  ganesha-config whitelist add "apt *"     Allow apt commands
  ganesha-config blacklist add "rm -rf"    Block rm -rf
  ganesha-config test "apt update"         Test if command is allowed
  ganesha-config preset                    Interactive preset selector
  ganesha-config reset                     Reset to defaults
""",
    )

    parser.add_argument("--system", "-s", action="store_true",
                        help="Use system config (/etc/ganesha/)")

    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # show
    show_parser = subparsers.add_parser("show", help="Show current config")
    show_parser.set_defaults(func=show_config)

    # set-level
    level_parser = subparsers.add_parser("set-level", help="Set access level")
    level_parser.add_argument("level", choices=[
        "restricted", "standard", "elevated", "full_access", "whitelist", "blacklist"
    ])
    level_parser.set_defaults(func=set_level)

    # whitelist
    whitelist_parser = subparsers.add_parser("whitelist", help="Manage whitelist")
    whitelist_sub = whitelist_parser.add_subparsers(dest="action")
    whitelist_add = whitelist_sub.add_parser("add", help="Add pattern")
    whitelist_add.add_argument("pattern")
    whitelist_add.set_defaults(func=add_whitelist)
    whitelist_rm = whitelist_sub.add_parser("remove", help="Remove pattern")
    whitelist_rm.add_argument("pattern")
    whitelist_rm.set_defaults(func=remove_whitelist)

    # blacklist
    blacklist_parser = subparsers.add_parser("blacklist", help="Manage blacklist")
    blacklist_sub = blacklist_parser.add_subparsers(dest="action")
    blacklist_add = blacklist_sub.add_parser("add", help="Add pattern")
    blacklist_add.add_argument("pattern")
    blacklist_add.set_defaults(func=add_blacklist)
    blacklist_rm = blacklist_sub.add_parser("remove", help="Remove pattern")
    blacklist_rm.add_argument("pattern")
    blacklist_rm.set_defaults(func=remove_blacklist)

    # test
    test_parser = subparsers.add_parser("test", help="Test if command is allowed")
    test_parser.add_argument("command", nargs="+")
    test_parser.set_defaults(func=lambda a: test_command(
        type("Args", (), {"command": " ".join(a.command)})()
    ))

    # preset
    preset_parser = subparsers.add_parser("preset", help="Interactive preset selector")
    preset_parser.set_defaults(func=interactive_preset)

    # reset
    reset_parser = subparsers.add_parser("reset", help="Reset to defaults")
    reset_parser.set_defaults(func=reset_config)

    args = parser.parse_args()

    if args.command is None:
        # Default to show
        show_config(args)
    elif hasattr(args, "func"):
        args.func(args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
