#!/usr/bin/env python3
"""Run codex-automation Rust CLI doctor checks."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def resolve_repo_root() -> Path:
    """Find the codex-automation source checkout for installed skill helpers."""
    candidates: list[Path] = []
    configured_repo = os.environ.get("CODEX_AUTOMATION_REPO")
    if configured_repo:
        candidates.append(Path(configured_repo).expanduser())
    cwd = Path.cwd()
    candidates.extend([cwd, *cwd.parents])
    for candidate in candidates:
        resolved = candidate.resolve()
        if (resolved / "Cargo.toml").is_file() and (
            resolved / "crates" / "codex-automation-cli"
        ).is_dir():
            return resolved
    raise RuntimeError(
        "codex-automation source checkout not found; run from the source repo or set CODEX_AUTOMATION_REPO"
    )


def automation_command(repo: Path, *args: str) -> list[str]:
    """Build a command for the local Rust automation binary."""
    configured = os.environ.get("CODEX_AUTOMATION_BIN")
    if configured:
        return [configured, *args]
    for candidate in [
        repo / "target" / "release" / "codex-automation",
        repo / "target" / "debug" / "codex-automation",
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
    installed = shutil.which("codex-automation")
    if installed:
        return [installed, *args]
    raise RuntimeError("codex-automation binary is unavailable; install Rust/Cargo or set CODEX_AUTOMATION_BIN")


def main() -> int:
    """Run the package doctor command from a checked-out source tree."""
    repo_root = resolve_repo_root()
    completed = subprocess.run(automation_command(repo_root, "doctor", "--json"), check=False, text=True)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
