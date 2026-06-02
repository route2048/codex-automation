#!/usr/bin/env python3
"""Agent-first installer for wiring a repository into codex-automation."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any


AUTOMATION_REPO = Path(__file__).resolve().parents[1]


def is_git_url(value: str) -> bool:
    """Return whether a target argument looks like a Git URL."""
    return "://" in value or value.startswith("git@")


def repo_name_from_url(url: str) -> str:
    """Derive a checkout directory name from a Git URL."""
    cleaned = url.rstrip("/").removesuffix(".git")
    name = re.split(r"[:/]", cleaned)[-1]
    if not name:
        raise ValueError(f"cannot derive repository name from URL: {url}")
    return name


def run_command(argv: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> dict[str, Any]:
    """Run a command and return a JSON-friendly record."""
    completed = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    record: dict[str, Any] = {
        "argv": argv,
        "cwd": str(cwd),
        "returncode": completed.returncode,
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
    }
    if completed.returncode != 0:
        raise RuntimeError(json.dumps(record, indent=2, sort_keys=True))
    return record


def run_json_command(argv: list[str], *, cwd: Path, env: dict[str, str]) -> dict[str, Any]:
    """Run a command that prints JSON and return its parsed payload."""
    record = run_command(argv, cwd=cwd, env=env)
    try:
        return json.loads(str(record["stdout"]))
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"command did not print JSON: {argv}\n{record['stdout']}") from exc


def automation_env(repo: Path) -> dict[str, str]:
    """Build an environment for running the local automation binary."""
    env = os.environ.copy()
    return env


def clone_or_pull(url: str, workspace: Path) -> dict[str, Any]:
    """Clone a Git URL or fast-forward an existing checkout."""
    workspace.mkdir(parents=True, exist_ok=True)
    destination = (workspace / repo_name_from_url(url)).resolve()
    if destination.exists():
        if not (destination / ".git").exists():
            raise FileExistsError(f"checkout path exists but is not a Git repo: {destination}")
        run_command(["git", "pull", "--ff-only"], cwd=destination)
        return {"kind": "git_url", "action": "pulled", "url": url, "path": str(destination)}
    run_command(["git", "clone", url, str(destination)], cwd=workspace)
    return {"kind": "git_url", "action": "cloned", "url": url, "path": str(destination)}


def resolve_target(target: str, workspace: Path) -> dict[str, Any]:
    """Resolve a local target path or materialize a Git URL."""
    if is_git_url(target):
        return clone_or_pull(target, workspace)
    path = Path(target).expanduser().resolve()
    if not path.is_dir():
        raise NotADirectoryError(f"target repository does not exist: {target}")
    return {"kind": "local_path", "action": "resolved", "path": str(path)}


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
    if cargo and (repo / "Cargo.toml").is_file():
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


def setup_target(
    *,
    target: str,
    control_workspace: Path,
    clone_dir: Path,
    target_id: str | None,
    profile: str,
    overwrite_workspace: bool,
) -> dict[str, Any]:
    """Clone or resolve a target and register it in a thin control workspace."""
    repo = AUTOMATION_REPO
    env = automation_env(repo)
    target_record = resolve_target(target, clone_dir)
    target_path = Path(str(target_record["path"])).resolve()
    if target_id:
        resolved_target_id = target_id
    elif is_git_url(target):
        resolved_target_id = repo_name_from_url(target)
    else:
        resolved_target_id = target_path.name
    doctor = run_json_command(automation_command(repo, "doctor", "--json"), cwd=repo, env=env)
    db = run_json_command(automation_command(repo, "db", "doctor", "--json"), cwd=repo, env=env)
    workspace_config = control_workspace / "codex-automation.toml"
    if workspace_config.exists() and not overwrite_workspace:
        workspace_payload = run_json_command(
            automation_command(repo, "workspace", "status", str(control_workspace), "--json"),
            cwd=repo,
            env=env,
        )
        workspace_action = "reused"
    else:
        init_args = ["workspace", "init", str(control_workspace), "--json"]
        if overwrite_workspace:
            init_args.insert(-1, "--overwrite")
        workspace_payload = run_json_command(automation_command(repo, *init_args), cwd=repo, env=env)
        workspace_action = "initialized"
    target_payload = run_json_command(
        automation_command(
            repo,
            "target",
            "add",
            resolved_target_id,
            "--repo",
            str(target_path),
            "--workspace",
            str(control_workspace),
            "--profile",
            profile,
            "--json",
        ),
        cwd=repo,
        env=env,
    )
    default_worker = control_workspace / "workers" / "repo-discovery.toml"
    if not default_worker.is_file():
        raise FileNotFoundError(f"default worker definition is missing: {default_worker}")
    worker_payload = run_json_command(
        automation_command(
            repo,
            "worker",
            "add",
            resolved_target_id,
            "--from-file",
            str(default_worker),
            "--json",
        ),
        cwd=repo,
        env=env,
    )
    target_pack = run_json_command(
        automation_command(repo, "target", "pack", resolved_target_id, "--json"),
        cwd=repo,
        env=env,
    )
    heartbeat = run_json_command(
        automation_command(repo, "heartbeat", "run", resolved_target_id, "--json"),
        cwd=repo,
        env=env,
    )
    target_status = run_json_command(
        automation_command(repo, "target", "status", resolved_target_id, "--json"),
        cwd=repo,
        env=env,
    )
    targets = run_json_command(
        automation_command(repo, "target", "list", "--workspace", str(control_workspace), "--json"),
        cwd=repo,
        env=env,
    )
    return {
        "status": "ready_for_handoff",
        "target": target_record,
        "target_id": resolved_target_id,
        "automation_repo": str(repo),
        "doctor": doctor,
        "db": db,
        "workspace_action": workspace_action,
        "workspace": workspace_payload,
        "target_registration": target_payload,
        "worker_registration": worker_payload,
        "target_pack": target_pack,
        "heartbeat": heartbeat,
        "target_status": target_status,
        "targets": targets,
        "handoff": {
            "control_workspace": str(control_workspace),
            "target_config_path": target_payload["target_config_path"],
            "worker_config_path": str(default_worker),
            "next_prompt": "Open the control workspace in Codex App. Inspect the heartbeat output and runner package before enabling execution.",
        },
    }


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", help="Target repository path or Git URL")
    parser.add_argument("--workspace", type=Path, default=Path("codex-automation"), help="Thin Codex App control workspace")
    parser.add_argument("--clone-dir", type=Path, default=Path("targets"), help="Clone directory for Git URL targets")
    parser.add_argument("--target-id", help="Stable target id; defaults to repo name")
    parser.add_argument("--profile", default="balanced", choices=["observe", "suggest", "balanced", "aggressive", "release"])
    parser.add_argument("--overwrite-workspace", action="store_true", help="Overwrite codex-automation.toml in the control workspace")
    return parser.parse_args()


def main() -> int:
    """Run the setup workflow."""
    args = parse_args()
    payload = setup_target(
        target=args.target,
        control_workspace=args.workspace.expanduser().resolve(),
        clone_dir=args.clone_dir.expanduser().resolve(),
        target_id=args.target_id,
        profile=args.profile,
        overwrite_workspace=args.overwrite_workspace,
    )
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
