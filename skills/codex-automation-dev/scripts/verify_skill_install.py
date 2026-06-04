#!/usr/bin/env python3
"""Verify local Codex skill installation for codex-automation skills."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


def resolve_repo(raw: str | None) -> Path:
    """Resolve the codex-automation source or public export checkout."""
    candidates: list[Path] = []
    if raw:
        candidates.append(Path(raw).expanduser())
    cwd = Path.cwd()
    candidates.extend([cwd, *cwd.parents])
    for candidate in candidates:
        resolved = candidate.resolve()
        if (resolved / "Cargo.toml").is_file() and (resolved / "skills").is_dir():
            return resolved
    raise RuntimeError("codex-automation repo not found; pass --repo or run from the repository")


def ensure_binary(repo: Path) -> Path:
    """Build and return a fresh source-built binary for verification."""
    run_command(
        [
            "cargo",
            "build",
            "-p",
            "codex-automation-cli",
            "--bin",
            "codex-automation",
        ],
        cwd=repo,
        env=os.environ.copy(),
    )
    binary = repo / "target" / "debug" / "codex-automation"
    if not binary.is_file():
        raise FileNotFoundError(f"built binary missing: {binary}")
    return binary


def copy_skill(source: Path, destination: Path, overwrite: bool) -> dict[str, Any]:
    """Copy a skill directory into CODEX_HOME/skills."""
    if not source.is_dir():
        raise NotADirectoryError(f"skill source missing: {source}")
    if destination.exists():
        if not overwrite:
            return {"action": "kept_existing", "path": str(destination)}
        shutil.rmtree(destination)
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    return {"action": "installed", "path": str(destination)}


def run_command(argv: list[str], *, cwd: Path, env: dict[str, str]) -> dict[str, Any]:
    """Run a command and capture output."""
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


def run_json(argv: list[str], *, cwd: Path, env: dict[str, str]) -> dict[str, Any]:
    """Run a JSON-producing command."""
    record = run_command(argv, cwd=cwd, env=env)
    try:
        return json.loads(str(record["stdout"]))
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"command did not print JSON: {argv}\n{record['stdout']}") from exc


def verify(args: argparse.Namespace) -> dict[str, Any]:
    """Install and verify skills."""
    repo = resolve_repo(args.repo)
    binary = ensure_binary(repo)
    codex_home = Path(args.codex_home or os.environ.get("CODEX_HOME", "~/.codex")).expanduser().resolve()
    skills_root = codex_home / "skills"
    skills_root.mkdir(parents=True, exist_ok=True)
    state_temp = tempfile.TemporaryDirectory(prefix="codex-automation-skill-state-")
    env = {
        **os.environ,
        "CODEX_AUTOMATION_BIN": str(binary),
        "CODEX_AUTOMATION_HOME": state_temp.name,
        "CODEX_HOME": str(codex_home),
    }
    operations: dict[str, Any] = {}
    if args.install_setup_skill:
        command = [
            str(binary),
            "skill",
            "install",
            "codex-automation-setup",
            "--codex-home",
            str(codex_home),
            "--json",
        ]
        if args.overwrite:
            command.insert(-1, "--overwrite")
        operations["codex-automation-setup"] = run_json(
            command,
            cwd=repo,
            env=env,
        )
    if args.install_dev_skill:
        operations["codex-automation-dev"] = copy_skill(
            repo / "skills" / "codex-automation-dev",
            skills_root / "codex-automation-dev",
            args.overwrite,
        )

    required = {
        "codex-automation-setup": [
            "SKILL.md",
            "agents/openai.yaml",
            "scripts/doctor.py",
            "scripts/setup.py",
            "scripts/update.py",
        ],
        "codex-automation-dev": [
            "SKILL.md",
            "agents/openai.yaml",
            "scripts/verify_clean_install.py",
            "scripts/verify_docker_install.sh",
            "scripts/verify_skill_install.py",
        ],
    }
    installed: dict[str, Any] = {}
    for skill, files in required.items():
        skill_root = skills_root / skill
        installed[skill] = {
            "path": str(skill_root),
            "exists": skill_root.is_dir(),
            "missing": [file for file in files if not (skill_root / file).is_file()],
        }
        if installed[skill]["missing"]:
            raise AssertionError(f"{skill} is missing files: {installed[skill]['missing']}")

    temp = tempfile.TemporaryDirectory(prefix="codex-automation-skill-")
    temp_root = Path(temp.name)
    env["CODEX_AUTOMATION_HOME"] = str(temp_root / "state")
    doctor = run_json(
        [sys.executable, str(skills_root / "codex-automation-setup" / "scripts" / "doctor.py")],
        cwd=Path(temp.name),
        env=env,
    )
    if doctor.get("status") != "ok":
        raise AssertionError(f"setup skill doctor status is {doctor.get('status')!r}")
    temp.cleanup()
    state_temp.cleanup()

    return {
        "status": "ok",
        "repo": str(repo),
        "binary": str(binary),
        "codex_home": str(codex_home),
        "operations": operations,
        "installed": installed,
        "doctor": doctor,
        "doctor_state_root": str(temp_root / "state"),
        "restart_required": True,
    }


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", help="codex-automation source or public export checkout")
    parser.add_argument("--codex-home", help="Codex home directory; defaults to CODEX_HOME or ~/.codex")
    parser.add_argument("--install-setup-skill", action="store_true", help="Install or refresh codex-automation-setup")
    parser.add_argument("--install-dev-skill", action="store_true", help="Install or refresh codex-automation-dev")
    parser.add_argument("--overwrite", action="store_true", help="Replace existing installed skill directories")
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    return parser.parse_args()


def main() -> int:
    """Run the verifier."""
    payload = verify(parse_args())
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
