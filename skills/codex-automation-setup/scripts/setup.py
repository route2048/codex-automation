#!/usr/bin/env python3
"""Delegate setup to the installed codex-automation binary."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys


def automation_command(*args: str) -> list[str]:
    """Build a command for the installed codex-automation binary."""
    configured = os.environ.get("CODEX_AUTOMATION_BIN")
    if configured:
        return [configured, *args]
    installed = shutil.which("codex-automation")
    if installed:
        return [installed, *args]
    raise RuntimeError("codex-automation binary is unavailable; install it or set CODEX_AUTOMATION_BIN")


def main(argv: list[str] | None = None) -> int:
    """Run `codex-automation init` with pass-through arguments."""
    args = list(sys.argv[1:] if argv is None else argv)
    if "--json" not in args:
        args.append("--json")
    completed = subprocess.run(automation_command("init", *args), check=False)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
