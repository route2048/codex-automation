#!/usr/bin/env sh
set -eu

REPO="route2048/codex-automation"
VERSION="latest"
INSTALL_DIR="${HOME}/.local/bin"

usage() {
  cat <<'USAGE'
Install codex-automation from GitHub Releases.

Usage:
  install.sh [--version vX.Y.Z] [--install-dir DIR] [--repo owner/name]

Environment:
  CODEX_AUTOMATION_REPO         Override the GitHub repository.
  CODEX_AUTOMATION_VERSION      Override the release tag.
  CODEX_AUTOMATION_INSTALL_DIR  Override the destination directory.
USAGE
}

REPO="${CODEX_AUTOMATION_REPO:-$REPO}"
VERSION="${CODEX_AUTOMATION_VERSION:-$VERSION}"
INSTALL_DIR="${CODEX_AUTOMATION_INSTALL_DIR:-$INSTALL_DIR}"

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
    --install-dir)
      require_value "$@"
      INSTALL_DIR="$2"
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

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Darwin:arm64)
    target="aarch64-apple-darwin"
    archive="codex-automation-aarch64-apple-darwin.tar.gz"
    ;;
  Darwin:x86_64)
    target="x86_64-apple-darwin"
    archive="codex-automation-x86_64-apple-darwin.tar.gz"
    ;;
  Linux:x86_64|Linux:amd64)
    target="x86_64-unknown-linux-gnu"
    archive="codex-automation-x86_64-unknown-linux-gnu.tar.gz"
    ;;
  *)
    echo "unsupported platform: $os $arch" >&2
    exit 1
    ;;
esac

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

archive_path="${tmp_dir}/${archive}"
checksum_path="${tmp_dir}/SHA256SUMS"

echo "Downloading ${archive} from ${REPO} (${VERSION})"
fetch "${base_url}/${archive}" "$archive_path"

if fetch "${base_url}/SHA256SUMS" "$checksum_path" >/dev/null 2>&1; then
  expected="$(grep " ${archive}\$" "$checksum_path" | awk '{print $1}')"
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
      echo "checksum mismatch for ${archive}" >&2
      exit 1
    fi
  fi
fi

mkdir -p "$tmp_dir/pkg" "$INSTALL_DIR"
tar -xzf "$archive_path" -C "$tmp_dir/pkg"
if [ ! -f "$tmp_dir/pkg/codex-automation" ]; then
  echo "archive did not contain codex-automation binary for ${target}" >&2
  exit 1
fi
install -m 0755 "$tmp_dir/pkg/codex-automation" "$INSTALL_DIR/codex-automation"

echo "Installed codex-automation to ${INSTALL_DIR}/codex-automation"
echo "Next:"
echo "  codex-automation skill install codex-automation-setup --json"
echo "  codex-automation init <target-repo> --workspace <control-workspace> --json"
