#!/usr/bin/env python3
"""Inspect an existing codex-automation installation with the binary CLI."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

from install_binary import ensure_binary


def automation_command(*args: str) -> list[str]:
    """Build a command for the installed codex-automation binary."""
    configured = os.environ.get("CODEX_AUTOMATION_BIN")
    if configured:
        return [configured, *args]
    installed = ensure_binary(
        repo=os.environ.get("CODEX_AUTOMATION_REPO", "route2048/codex-automation"),
        version=os.environ.get("CODEX_AUTOMATION_VERSION", "latest"),
        install_dir=Path(os.environ["CODEX_AUTOMATION_INSTALL_DIR"]).expanduser()
        if os.environ.get("CODEX_AUTOMATION_INSTALL_DIR")
        else None,
    )
    return [str(installed), *args]


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse update wrapper arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", help="Optional thin control workspace")
    parser.add_argument("--target-id", help="Optional target id to inspect")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    """Validate app state and print target status."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    subprocess.run(automation_command("doctor", "--json"), check=True)
    subprocess.run(automation_command("db", "doctor", "--json"), check=True)
    target_list = ["target", "list", "--json"]
    if args.workspace:
        target_list.extend(["--workspace", args.workspace])
    subprocess.run(automation_command(*target_list), check=True)
    if args.target_id:
        subprocess.run(automation_command("target", "status", args.target_id, "--json"), check=True)
        subprocess.run(automation_command("target", "pack", args.target_id, "--json"), check=True)
        subprocess.run(
            automation_command("heartbeat", "run", args.target_id, "--dry-run", "--json"),
            check=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
