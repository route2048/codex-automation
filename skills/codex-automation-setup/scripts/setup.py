#!/usr/bin/env python3
"""Delegate setup to the repository-level Rust CLI installer."""

from __future__ import annotations

import os
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


def main(argv: list[str] | None = None) -> int:
    """Run the repository setup script with pass-through arguments."""
    repo_root = resolve_repo_root()
    args = sys.argv[1:] if argv is None else argv
    completed = subprocess.run([sys.executable, str(repo_root / "scripts" / "setup.py"), *args], check=False)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
