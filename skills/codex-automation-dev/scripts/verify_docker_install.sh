#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-${CODEX_AUTOMATION_REPO:-$(pwd)}}"
IMAGE="${CODEX_AUTOMATION_DOCKER_IMAGE:-rust:1-bookworm}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codex-automation-docker.XXXXXX")"

cleanup() {
  if [ "${CODEX_AUTOMATION_DOCKER_KEEP_TEMP:-0}" != "1" ]; then
    rm -rf "$WORK_DIR"
  else
    echo "keeping Docker verification work dir: $WORK_DIR" >&2
  fi
}
trap cleanup EXIT

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is not installed or not on PATH" >&2
  exit 127
fi

if [ ! -f "$REPO/Cargo.toml" ]; then
  echo "codex-automation repo not found: $REPO" >&2
  exit 2
fi

docker run --rm \
  -e CARGO_TERM_COLOR=never \
  -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" \
  -e VERIFY_IMAGE="$IMAGE" \
  -v "$REPO":/repo:ro \
  -v "$WORK_DIR":/work \
  "$IMAGE" \
  bash -lc '
set -euo pipefail
apt-get update
apt-get install -y --no-install-recommends python3 git ca-certificates

mkdir -p /work/codex-automation /work/cargo-home /work/cargo-target
tar \
  --exclude=.git \
  --exclude=.archive \
  --exclude=.public-export \
  --exclude=target \
  --exclude=reports \
  --exclude=codex-automation.json \
  --exclude=__pycache__ \
  --exclude="*.pyc" \
  -C /repo \
  -cf - . | tar -C /work/codex-automation -xf -
cd /work/codex-automation

export PATH="/usr/local/cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export CARGO_HOME=/work/cargo-home
export CARGO_TARGET_DIR=/work/cargo-target
cargo install --path crates/codex-automation-cli --locked --root /work/codex-install
export PATH="/work/codex-install/bin:$PATH"
export CODEX_AUTOMATION_HOME=/work/codex-state
export CODEX_AUTOMATION_BIN=/work/codex-install/bin/codex-automation

codex-automation doctor --json
codex-automation db doctor --json

cp -R tests/fixtures/node-package /work/target
python3 scripts/setup.py /work/target \
  --workspace /work/control \
  --clone-dir /work/clones \
  --target-id docker-node \
  --profile observe > /work/setup.json

codex-automation target list --json
codex-automation target pack docker-node --json
codex-automation heartbeat run docker-node --dry-run --json

test ! -e /work/target/.codex-automation
test -f /work/control/codex-automation.toml
test -f /work/codex-state/codex-automation.sqlite

python3 - <<PY
import json
from pathlib import Path

payload = json.loads(Path("/work/setup.json").read_text())
assert payload["status"] == "ready_for_handoff", payload["status"]
assert any(item.get("status") == "package_ready" for item in payload["heartbeat"]["dispatched"])
print(json.dumps({
    "status": "ok",
    "image": "'"$IMAGE"'",
    "target_id": "docker-node",
    "setup": payload["status"],
    "target_clean": not Path("/work/target/.codex-automation").exists(),
    "control_workspace": Path("/work/control/codex-automation.toml").is_file(),
    "database": Path("/work/codex-state/codex-automation.sqlite").is_file(),
}, indent=2, sort_keys=True))
PY
'
