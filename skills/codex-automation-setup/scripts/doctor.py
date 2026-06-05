#!/usr/bin/env python3
"""Run codex-automation binary doctor checks."""

from __future__ import annotations

import os
import subprocess
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


def main() -> int:
    """Run the package doctor command."""
    completed = subprocess.run(automation_command("doctor", "--json"), check=False, text=True)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
