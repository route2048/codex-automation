#!/usr/bin/env python3
"""Inspect an existing codex-automation installation with the Rust CLI."""

from __future__ import annotations

import argparse
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


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse update wrapper arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", help="Optional thin control workspace")
    parser.add_argument("--target-id", help="Optional target id to inspect")
    return parser.parse_args(argv)


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


def main(argv: list[str] | None = None) -> int:
    """Validate the app state and print target status."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    repo_root = resolve_repo_root()
    subprocess.run(automation_command(repo_root, "doctor", "--json"), check=True)
    subprocess.run(automation_command(repo_root, "db", "doctor", "--json"), check=True)
    target_list = ["target", "list", "--json"]
    if args.workspace:
        target_list.extend(["--workspace", args.workspace])
    subprocess.run(automation_command(repo_root, *target_list), check=True)
    if args.target_id:
        subprocess.run(automation_command(repo_root, "target", "status", args.target_id, "--json"), check=True)
        subprocess.run(automation_command(repo_root, "target", "pack", args.target_id, "--json"), check=True)
        subprocess.run(automation_command(repo_root, "heartbeat", "run", args.target_id, "--dry-run", "--json"), check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
