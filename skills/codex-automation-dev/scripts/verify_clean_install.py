#!/usr/bin/env python3
"""Verify codex-automation with a clean local install and temp app state."""

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


def run_command(argv: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> dict[str, Any]:
    """Run a command and return captured output."""
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


def resolve_repo(raw: str | None) -> Path:
    """Resolve the source or public export checkout."""
    candidates: list[Path] = []
    if raw:
        candidates.append(Path(raw).expanduser())
    cwd = Path.cwd()
    candidates.extend([cwd, *cwd.parents])
    for candidate in candidates:
        resolved = candidate.resolve()
        if (resolved / "Cargo.toml").is_file() and (
            resolved / "crates" / "codex-automation-cli"
        ).is_dir():
            return resolved
    raise RuntimeError("codex-automation repo not found; pass --repo or run from the repository")


def assert_status(payload: dict[str, Any], expected: str, label: str) -> None:
    """Assert a payload status value."""
    actual = payload.get("status")
    if actual != expected:
        raise AssertionError(f"{label} status is {actual!r}, expected {expected!r}")


def verify(repo: Path, fixture: Path, target_id: str, profile: str, keep_temp: bool) -> dict[str, Any]:
    """Run the clean install smoke."""
    temp_context = tempfile.TemporaryDirectory(prefix="codex-automation-clean-")
    temp_root = Path(temp_context.name)
    if keep_temp:
        temp_context.cleanup()
        temp_root.mkdir(parents=True, exist_ok=True)
        cleanup = None
    else:
        cleanup = temp_context

    install_root = temp_root / "install"
    app_home = temp_root / "state"
    codex_home = temp_root / "codex-home"
    cargo_target = temp_root / "cargo-target"
    control = temp_root / "control"
    clones = temp_root / "clones"
    target = temp_root / "target"
    shutil.copytree(fixture, target)

    install = run_command(
        [
            "cargo",
            "install",
            "--path",
            str(repo / "crates" / "codex-automation-cli"),
            "--locked",
            "--root",
            str(install_root),
        ],
        cwd=repo,
        env={**os.environ, "CARGO_TARGET_DIR": str(cargo_target)},
    )

    bin_path = install_root / "bin" / "codex-automation"
    if not bin_path.is_file():
        raise FileNotFoundError(f"installed binary missing: {bin_path}")

    env = {
        **os.environ,
        "PATH": f"{install_root / 'bin'}{os.pathsep}{os.environ.get('PATH', '')}",
        "CODEX_AUTOMATION_HOME": str(app_home),
        "CODEX_AUTOMATION_BIN": str(bin_path),
        "CODEX_HOME": str(codex_home),
    }

    doctor = run_json([str(bin_path), "doctor", "--json"], cwd=repo, env=env)
    assert_status(doctor, "ok", "doctor")
    db = run_json([str(bin_path), "db", "doctor", "--json"], cwd=repo, env=env)
    assert_status(db, "ok", "db doctor")
    skill_install = run_json(
        [
            str(bin_path),
            "skill",
            "install",
            "codex-automation-setup",
            "--codex-home",
            str(codex_home),
            "--json",
        ],
        cwd=repo,
        env=env,
    )
    assert_status(skill_install, "installed", "skill install")
    skill_status = run_json(
        [
            str(bin_path),
            "skill",
            "status",
            "codex-automation-setup",
            "--codex-home",
            str(codex_home),
            "--json",
        ],
        cwd=repo,
        env=env,
    )
    if not skill_status.get("installed"):
        raise AssertionError("embedded setup skill was not installed")

    setup = run_json(
        [
            str(bin_path),
            "init",
            str(target),
            "--workspace",
            str(control),
            "--clone-dir",
            str(clones),
            "--target-id",
            target_id,
            "--profile",
            profile,
            "--json",
        ],
        cwd=repo,
        env=env,
    )
    assert_status(setup, "ready_for_handoff", "setup")

    target_list = run_json([str(bin_path), "target", "list", "--json"], cwd=repo, env=env)
    assert_status(target_list, "ok", "target list")
    target_pack = run_json([str(bin_path), "target", "pack", target_id, "--json"], cwd=repo, env=env)
    assert_status(target_pack, "generated", "target pack")
    heartbeat = run_json(
        [str(bin_path), "heartbeat", "run", target_id, "--dry-run", "--json"],
        cwd=repo,
        env=env,
    )
    assert_status(heartbeat, "ok", "heartbeat")

    if (target / ".codex-automation").exists():
        raise AssertionError("target repo was modified with .codex-automation")
    if not (control / "codex-automation.toml").is_file():
        raise AssertionError("control workspace config was not written")
    if not (app_home / "codex-automation.sqlite").is_file():
        raise AssertionError("SQLite app database was not created")
    dispatched = setup.get("heartbeat", {}).get("dispatched", [])
    if not any(item.get("status") == "package_ready" for item in dispatched):
        raise AssertionError("setup heartbeat did not create a runner package")

    result = {
        "status": "ok",
        "repo": str(repo),
        "fixture": str(fixture),
        "target_id": target_id,
        "profile": profile,
        "temp_root": str(temp_root),
        "temp_kept": keep_temp,
        "installed_binary": str(bin_path),
        "checks": {
            "cargo_install": install["returncode"] == 0,
            "doctor": doctor["status"],
            "db_doctor": db["status"],
            "skill_install": skill_install["status"],
            "skill_status": skill_status["status"],
            "setup": setup["status"],
            "target_pack": target_pack["status"],
            "heartbeat": heartbeat["status"],
            "target_clean": not (target / ".codex-automation").exists(),
            "control_workspace": str(control / "codex-automation.toml"),
            "database": str(app_home / "codex-automation.sqlite"),
        },
    }
    if cleanup is not None:
        cleanup.cleanup()
    return result


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", help="codex-automation source or public export checkout")
    parser.add_argument("--fixture", default="tests/fixtures/node-package", help="Fixture target path relative to repo")
    parser.add_argument("--target-id", default="clean-node", help="Temporary target id")
    parser.add_argument("--profile", default="observe", choices=["observe", "suggest", "balanced", "aggressive", "release"])
    parser.add_argument("--keep-temp", action="store_true", help="Keep temporary install/app state for inspection")
    parser.add_argument("--json", action="store_true", help="Print JSON output")
    return parser.parse_args()


def main() -> int:
    """Run the verifier."""
    args = parse_args()
    repo = resolve_repo(args.repo)
    fixture = Path(args.fixture)
    if not fixture.is_absolute():
        fixture = repo / fixture
    if not fixture.is_dir():
        raise NotADirectoryError(f"fixture target does not exist: {fixture}")
    payload = verify(repo, fixture.resolve(), args.target_id, args.profile, args.keep_temp)
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
