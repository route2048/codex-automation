#!/usr/bin/env python3
"""Delegate setup to the installed codex-automation binary."""

from __future__ import annotations

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


def main(argv: list[str] | None = None) -> int:
    """Run `codex-automation init` with pass-through arguments."""
    args = list(sys.argv[1:] if argv is None else argv)
    if "--json" not in args:
        args.append("--json")
    completed = subprocess.run(automation_command("init", *args), check=False)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
