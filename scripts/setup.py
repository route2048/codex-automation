#!/usr/bin/env python3
"""Compatibility wrapper for the binary-first setup command."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def automation_command(*args: str) -> list[str]:
    """Build a command for the installed or source-built Rust CLI."""
    configured = os.environ.get("CODEX_AUTOMATION_BIN")
    if configured:
        return [configured, *args]
    installed = shutil.which("codex-automation")
    if installed:
        return [installed, *args]
    for candidate in [
        REPO_ROOT / "target" / "release" / "codex-automation",
        REPO_ROOT / "target" / "debug" / "codex-automation",
    ]:
        if candidate.is_file():
            return [str(candidate), *args]
    cargo = shutil.which("cargo")
    if cargo:
        return [
            cargo,
            "run",
            "--quiet",
            "-p",
            "codex-automation-cli",
            "--bin",
            "codex-automation",
            "--",
            *args,
        ]
    raise RuntimeError("codex-automation binary is unavailable; install it or set CODEX_AUTOMATION_BIN")


def main(argv: list[str] | None = None) -> int:
    """Delegate to `codex-automation init` and preserve JSON output by default."""
    args = list(sys.argv[1:] if argv is None else argv)
    if "--json" not in args:
        args.append("--json")
    completed = subprocess.run(
        automation_command("init", *args),
        cwd=REPO_ROOT,
        check=False,
    )
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
