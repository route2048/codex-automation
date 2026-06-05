#!/usr/bin/env python3
"""Update codex-automation, then validate the local automation state."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from install_binary import (
    DEFAULT_REPO,
    DEFAULT_VERSION,
    default_install_dir,
    ensure_binary,
    install_binary,
)


def resolve_binary(args: argparse.Namespace) -> tuple[Path, dict[str, object]]:
    """Install or resolve the binary for an update run."""
    configured = os.environ.get("CODEX_AUTOMATION_BIN")
    if configured:
        path = Path(configured).expanduser()
        if not path.is_file():
            raise FileNotFoundError(f"CODEX_AUTOMATION_BIN does not exist: {path}")
        return path, {
            "status": "external_binary",
            "binary": str(path),
            "updated": False,
            "reason": "CODEX_AUTOMATION_BIN is set",
        }
    install_dir = args.install_dir.expanduser() if args.install_dir else default_install_dir()
    if args.check:
        path = ensure_binary(repo=args.repo, version=args.version, install_dir=install_dir)
        return path, {
            "status": "checked",
            "binary": str(path),
            "updated": False,
            "version": args.version,
        }
    payload = install_binary(
        repo=args.repo,
        version=args.version,
        install_dir=install_dir,
        force=True,
    )
    payload["updated"] = True
    return Path(str(payload["binary"])), payload


def run_json(argv: list[str]) -> dict[str, object]:
    """Run a JSON-producing command."""
    completed = subprocess.run(argv, check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        raise RuntimeError(
            json.dumps(
                {
                    "argv": argv,
                    "returncode": completed.returncode,
                    "stdout": completed.stdout.strip(),
                    "stderr": completed.stderr.strip(),
                },
                indent=2,
                sort_keys=True,
            )
        )
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"command did not print JSON: {argv}\n{completed.stdout}") from exc


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse update wrapper arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=os.environ.get("CODEX_AUTOMATION_REPO", DEFAULT_REPO))
    parser.add_argument("--version", default=os.environ.get("CODEX_AUTOMATION_VERSION", DEFAULT_VERSION))
    parser.add_argument(
        "--install-dir",
        type=Path,
        default=Path(os.environ["CODEX_AUTOMATION_INSTALL_DIR"]).expanduser()
        if os.environ.get("CODEX_AUTOMATION_INSTALL_DIR")
        else None,
    )
    parser.add_argument("--workspace", help="Optional thin control workspace")
    parser.add_argument("--target-id", help="Optional target id to inspect")
    parser.add_argument("--check", action="store_true", help="Inspect without replacing the binary")
    parser.add_argument("--json", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    """Update the binary and validate app state."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    binary, binary_update = resolve_binary(args)
    command = [str(binary), "update", "--json"]
    if args.workspace:
        command.extend(["--workspace", args.workspace])
    if args.target_id:
        command.extend(["--target-id", args.target_id])
    if args.check:
        command.append("--check")
    state_update = run_json(command)
    payload = {
        "status": state_update["status"],
        "binary_update": binary_update,
        "state_update": state_update,
    }
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(f"{payload['status']}: {binary}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
