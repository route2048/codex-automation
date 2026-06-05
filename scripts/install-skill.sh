#!/usr/bin/env sh
set -eu

REPO="route2048/codex-automation"
VERSION="latest"
CODEX_HOME="${CODEX_HOME:-${HOME}/.codex}"
SKILL_NAME="codex-automation-setup"
ARCHIVE="codex-automation-setup-skill.tar.gz"

usage() {
  cat <<'USAGE'
Install the codex-automation setup skill from GitHub Releases.

Usage:
  install-skill.sh [--version vX.Y.Z] [--codex-home DIR] [--repo owner/name]

Environment:
  CODEX_AUTOMATION_REPO      Override the GitHub repository.
  CODEX_AUTOMATION_VERSION   Override the release tag.
  CODEX_HOME                 Override the Codex home directory.
USAGE
}

REPO="${CODEX_AUTOMATION_REPO:-$REPO}"
VERSION="${CODEX_AUTOMATION_VERSION:-$VERSION}"

require_value() {
  if [ "$#" -lt 2 ]; then
    echo "$1 requires a value" >&2
    exit 2
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      require_value "$@"
      REPO="$2"
      shift 2
      ;;
    --version)
      require_value "$@"
      VERSION="$2"
      shift 2
      ;;
    --codex-home)
      require_value "$@"
      CODEX_HOME="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if command -v curl >/dev/null 2>&1; then
  fetch() {
    curl -fsSL "$1" -o "$2"
  }
elif command -v wget >/dev/null 2>&1; then
  fetch() {
    wget -qO "$2" "$1"
  }
else
  echo "curl or wget is required" >&2
  exit 1
fi

if [ "$VERSION" = "latest" ]; then
  base_url="https://github.com/${REPO}/releases/latest/download"
else
  base_url="https://github.com/${REPO}/releases/download/${VERSION}"
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

archive_path="${tmp_dir}/${ARCHIVE}"
checksum_path="${tmp_dir}/SHA256SUMS"
package_dir="${tmp_dir}/pkg"

echo "Downloading ${ARCHIVE} from ${REPO} (${VERSION})"
fetch "${base_url}/${ARCHIVE}" "$archive_path"

if fetch "${base_url}/SHA256SUMS" "$checksum_path" >/dev/null 2>&1; then
  expected="$(grep " ${ARCHIVE}\$" "$checksum_path" | awk '{print $1}')"
  if [ -n "$expected" ]; then
    if command -v shasum >/dev/null 2>&1; then
      actual="$(shasum -a 256 "$archive_path" | awk '{print $1}')"
    elif command -v sha256sum >/dev/null 2>&1; then
      actual="$(sha256sum "$archive_path" | awk '{print $1}')"
    else
      echo "warning: shasum or sha256sum not found; skipping checksum verification" >&2
      actual="$expected"
    fi
    if [ "$actual" != "$expected" ]; then
      echo "checksum mismatch for ${ARCHIVE}" >&2
      exit 1
    fi
  fi
fi

mkdir -p "$package_dir" "$CODEX_HOME/skills"
tar -xzf "$archive_path" -C "$package_dir"
if [ ! -f "$package_dir/${SKILL_NAME}/SKILL.md" ]; then
  echo "archive did not contain ${SKILL_NAME}/SKILL.md" >&2
  exit 1
fi

rm -rf "$CODEX_HOME/skills/${SKILL_NAME}"
cp -R "$package_dir/${SKILL_NAME}" "$CODEX_HOME/skills/${SKILL_NAME}"

echo "Installed ${SKILL_NAME} to ${CODEX_HOME}/skills/${SKILL_NAME}"
echo "Next:"
echo "  Start a new Codex thread and ask:"
echo "  Use \$codex-automation-setup to enable codex-automation for this repository."
