#!/usr/bin/env python3
"""Run codex-automation binary doctor checks."""

from __future__ import annotations

import os
import shutil
import subprocess


def automation_command(*args: str) -> list[str]:
    """Build a command for the installed codex-automation binary."""
    configured = os.environ.get("CODEX_AUTOMATION_BIN")
    if configured:
        return [configured, *args]
    installed = shutil.which("codex-automation")
    if installed:
        return [installed, *args]
    raise RuntimeError("codex-automation binary is unavailable; install it or set CODEX_AUTOMATION_BIN")


def main() -> int:
    """Run the package doctor command."""
    completed = subprocess.run(automation_command("doctor", "--json"), check=False, text=True)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
