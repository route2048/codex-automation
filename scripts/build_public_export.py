#!/usr/bin/env python3
"""Build a sanitized public export tree from this private working checkout."""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]


def load_manifest() -> dict[str, Any]:
    """Load the public export manifest."""
    return json.loads((REPO_ROOT / "MANIFEST.public.json").read_text(encoding="utf-8"))


def should_skip(path: Path, manifest: dict[str, Any]) -> bool:
    """Return whether a path should be skipped during export."""
    if path.name in set(manifest.get("exclude_names", [])):
        return True
    return any(path.name.endswith(suffix) for suffix in manifest.get("exclude_suffixes", []))


def copy_entry(source: Path, destination: Path, manifest: dict[str, Any]) -> None:
    """Copy a manifest entry into the export directory."""
    if source.is_dir():
        for child in source.rglob("*"):
            if should_skip(child, manifest):
                continue
            relative = child.relative_to(source)
            target = destination / relative
            if child.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            elif child.is_file():
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(child, target)
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def iter_export_files(output: Path) -> list[Path]:
    """Return all regular files in a deterministic order."""
    return sorted(path for path in output.rglob("*") if path.is_file())


def audit_export(output: Path, manifest: dict[str, Any]) -> dict[str, Any]:
    """Check the export tree for private paths and known private content."""
    private_paths = {str(path) for path in manifest.get("private_paths", [])}
    forbidden_patterns = [str(pattern) for pattern in manifest.get("forbidden_content_patterns", [])]
    private_path_hits: list[str] = []
    for private_path in sorted(private_paths):
        if (output / private_path).exists():
            private_path_hits.append(private_path)
    content_hits: list[dict[str, str]] = []
    for path in iter_export_files(output):
        relative_path = str(path.relative_to(output))
        if relative_path == "MANIFEST.public.json":
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for pattern in forbidden_patterns:
            if pattern in text:
                content_hits.append(
                    {
                        "path": relative_path,
                        "pattern": pattern,
                    }
                )
    status = "ok" if not private_path_hits and not content_hits else "failed"
    return {
        "status": status,
        "file_count": len(iter_export_files(output)),
        "private_path_hits": private_path_hits,
        "content_hits": content_hits,
        "forbidden_content_patterns": forbidden_patterns,
    }


def build_export(output: Path, *, overwrite: bool) -> dict[str, Any]:
    """Build the public export tree."""
    manifest = load_manifest()
    output = output.expanduser().resolve()
    if output.exists():
        if not overwrite:
            raise FileExistsError(f"output already exists: {output}")
        shutil.rmtree(output)
    output.mkdir(parents=True)
    copied: list[str] = []
    for entry in manifest["include"]:
        source = REPO_ROOT / entry
        if not source.exists():
            raise FileNotFoundError(f"manifest entry does not exist: {entry}")
        copy_entry(source, output / entry, manifest)
        copied.append(entry)
    audit = audit_export(output, manifest)
    if audit["status"] != "ok":
        raise RuntimeError(f"public export audit failed: {audit}")
    return {
        "status": "exported",
        "output": str(output),
        "copied": copied,
        "audit": audit,
        "private_paths_excluded": manifest.get("private_paths", []),
    }


def check_export(output: Path) -> dict[str, Any]:
    """Audit an existing public export tree."""
    manifest = load_manifest()
    output = output.expanduser().resolve()
    if not output.is_dir():
        raise NotADirectoryError(f"export directory does not exist: {output}")
    audit = audit_export(output, manifest)
    return {"status": audit["status"], "output": str(output), "audit": audit}


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="Destination directory")
    parser.add_argument("--overwrite", action="store_true", help="Replace an existing destination")
    parser.add_argument("--check-only", action="store_true", help="Audit an existing destination without copying")
    return parser.parse_args()


def main() -> int:
    """Run the public export builder."""
    args = parse_args()
    payload = check_export(args.output) if args.check_only else build_export(args.output, overwrite=args.overwrite)
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0 if payload["status"] in {"ok", "exported"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
