#!/usr/bin/env python3
"""Install the codex-automation binary from GitHub Releases."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import stat
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path
from typing import Any


DEFAULT_REPO = "route2048/codex-automation"
DEFAULT_VERSION = "latest"


def default_install_dir() -> Path:
    """Return the default per-user install directory."""
    if os.name == "nt":
        profile = os.environ.get("USERPROFILE")
        if not profile:
            raise RuntimeError("USERPROFILE is required on Windows")
        return Path(profile) / ".local" / "bin"
    home = os.environ.get("HOME")
    if not home:
        raise RuntimeError("HOME is required")
    return Path(home) / ".local" / "bin"


def platform_asset() -> tuple[str, str, str]:
    """Return target triple, archive name, and installed binary name."""
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        return (
            "aarch64-apple-darwin",
            "codex-automation-aarch64-apple-darwin.tar.gz",
            "codex-automation",
        )
    if system == "Darwin" and machine in {"x86_64", "amd64"}:
        return (
            "x86_64-apple-darwin",
            "codex-automation-x86_64-apple-darwin.tar.gz",
            "codex-automation",
        )
    if system == "Linux" and machine in {"x86_64", "amd64"}:
        return (
            "x86_64-unknown-linux-gnu",
            "codex-automation-x86_64-unknown-linux-gnu.tar.gz",
            "codex-automation",
        )
    if system == "Windows" and machine in {"x86_64", "amd64"}:
        return (
            "x86_64-pc-windows-msvc",
            "codex-automation-x86_64-pc-windows-msvc.zip",
            "codex-automation.exe",
        )
    raise RuntimeError(f"unsupported platform: {system} {machine}")


def release_base_url(repo: str, version: str) -> str:
    """Return the GitHub Release download URL prefix."""
    if version == "latest":
        return f"https://github.com/{repo}/releases/latest/download"
    return f"https://github.com/{repo}/releases/download/{version}"


def download(url: str, destination: Path) -> None:
    """Download a release asset."""
    request = urllib.request.Request(url, headers={"User-Agent": "codex-automation-setup"})
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            destination.write_bytes(response.read())
    except urllib.error.HTTPError as exc:
        raise RuntimeError(f"failed to download {url}: HTTP {exc.code}") from exc


def parse_checksums(text: str) -> dict[str, str]:
    """Parse SHA256SUMS content."""
    checksums: dict[str, str] = {}
    for line in text.splitlines():
        parts = line.strip().split()
        if len(parts) >= 2:
            checksums[parts[-1]] = parts[0]
    return checksums


def verify_checksum(archive: Path, archive_name: str, checksum_file: Path) -> bool:
    """Verify an archive when SHA256SUMS contains a matching entry."""
    checksums = parse_checksums(checksum_file.read_text(encoding="utf-8"))
    expected = checksums.get(archive_name)
    if not expected:
        raise RuntimeError(f"SHA256SUMS did not contain {archive_name}")
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    if actual != expected:
        raise RuntimeError(f"checksum mismatch for {archive_name}")
    return True


def extract_archive(archive: Path, package_dir: Path) -> None:
    """Extract a release archive."""
    root = package_dir.resolve()
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as zf:
            for member in zf.namelist():
                destination = (package_dir / member).resolve()
                try:
                    destination.relative_to(root)
                except ValueError:
                    raise RuntimeError(f"refusing unsafe zip member: {member}")
            zf.extractall(package_dir)
        return
    with tarfile.open(archive, "r:gz") as tf:
        for member in tf.getmembers():
            destination = (package_dir / member.name).resolve()
            try:
                destination.relative_to(root)
            except ValueError:
                raise RuntimeError(f"refusing unsafe tar member: {member.name}")
        tf.extractall(package_dir)


def install_binary(
    *,
    repo: str,
    version: str,
    install_dir: Path,
    force: bool,
    allow_missing_checksum: bool = False,
) -> dict[str, Any]:
    """Install the release binary and return an operation payload."""
    target, archive_name, binary_name = platform_asset()
    binary_path = install_dir / binary_name
    if binary_path.is_file() and not force:
        return {
            "status": "already_installed",
            "binary": str(binary_path),
            "target": target,
            "version": version,
        }
    base_url = release_base_url(repo, version)
    with tempfile.TemporaryDirectory(prefix="codex-automation-install-") as raw_temp:
        temp = Path(raw_temp)
        archive = temp / archive_name
        checksum_file = temp / "SHA256SUMS"
        download(f"{base_url}/{archive_name}", archive)
        checksum_verified = False
        try:
            download(f"{base_url}/SHA256SUMS", checksum_file)
            checksum_verified = verify_checksum(archive, archive_name, checksum_file)
        except Exception as exc:
            if not allow_missing_checksum:
                raise RuntimeError(
                    "checksum verification failed; rerun with "
                    "--allow-missing-checksum only for trusted local testing"
                ) from exc
            checksum_verified = False
        package_dir = temp / "pkg"
        package_dir.mkdir()
        extract_archive(archive, package_dir)
        extracted = package_dir / binary_name
        if not extracted.is_file():
            extracted = package_dir / "codex-automation"
        if not extracted.is_file():
            raise RuntimeError(f"archive did not contain codex-automation binary for {target}")
        install_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(extracted, binary_path)
        if os.name != "nt":
            mode = binary_path.stat().st_mode
            binary_path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return {
        "status": "installed",
        "binary": str(binary_path),
        "target": target,
        "version": version,
        "archive": archive_name,
        "checksum_verified": checksum_verified,
    }


def find_binary(preferred_dir: Path | None = None) -> Path | None:
    """Return an existing binary path, if available."""
    configured = os.environ.get("CODEX_AUTOMATION_BIN")
    if configured:
        path = Path(configured).expanduser()
        if path.is_file():
            return path
        raise RuntimeError(f"CODEX_AUTOMATION_BIN does not exist: {path}")
    if preferred_dir:
        _, _, binary_name = platform_asset()
        preferred = preferred_dir / binary_name
        if preferred.is_file():
            return preferred
    found = shutil.which("codex-automation")
    return Path(found) if found else None


def ensure_binary(
    *,
    repo: str = DEFAULT_REPO,
    version: str = DEFAULT_VERSION,
    install_dir: Path | None = None,
    force: bool = False,
    allow_missing_checksum: bool = False,
) -> Path:
    """Return an existing binary or install one from GitHub Releases."""
    destination = install_dir or default_install_dir()
    if not force:
        existing = find_binary(destination)
        if existing:
            return existing
    payload = install_binary(
        repo=repo,
        version=version,
        install_dir=destination,
        force=force,
        allow_missing_checksum=allow_missing_checksum,
    )
    return Path(str(payload["binary"]))


def parse_args() -> argparse.Namespace:
    """Parse CLI arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=os.environ.get("CODEX_AUTOMATION_REPO", DEFAULT_REPO))
    parser.add_argument("--version", default=os.environ.get("CODEX_AUTOMATION_VERSION", DEFAULT_VERSION))
    parser.add_argument(
        "--install-dir",
        type=Path,
        default=Path(os.environ["CODEX_AUTOMATION_INSTALL_DIR"]).expanduser()
        if os.environ.get("CODEX_AUTOMATION_INSTALL_DIR")
        else default_install_dir(),
    )
    parser.add_argument("--force", action="store_true", help="Replace an existing binary")
    parser.add_argument(
        "--allow-missing-checksum",
        action="store_true",
        help="Allow install when SHA256SUMS is unavailable; use only for trusted testing.",
    )
    parser.add_argument("--json", action="store_true")
    return parser.parse_args()


def main() -> int:
    """Install the binary from GitHub Releases."""
    args = parse_args()
    payload = install_binary(
        repo=args.repo,
        version=args.version,
        install_dir=args.install_dir.expanduser(),
        force=args.force,
        allow_missing_checksum=args.allow_missing_checksum,
    )
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(f"{payload['status']}: {payload['binary']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
