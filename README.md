# codex-automation

`codex-automation` is a local-first Rust CLI for setting up and supervising
Codex automation against one or more repositories.

This is an unofficial project. It is not affiliated with, endorsed by, or
sponsored by OpenAI.

The app keeps human-facing configuration in a thin Codex App workspace, while
durable runtime state lives in a local SQLite database under the OS application
data directory. Target repositories stay clean during setup.

## Install

Build or install the binary from a source checkout:

```bash
cargo install --path crates/codex-automation-cli --locked
codex-automation doctor --json
```

During development, run it from the workspace:

```bash
cargo run --quiet -p codex-automation-cli --bin codex-automation -- doctor --json
```

## Layout

For normal use, keep the source checkout, control workspace, app state, and
target repos separate:

```text
codex-automation-src/          # source checkout for this project
codex-automation/              # thin Codex App control workspace
target-repo/                   # product or OSS repository being automated
OS app data/codex-automation/  # SQLite, worktrees, logs, artifacts
```

The generated control workspace looks like this:

```text
codex-automation/
├── AGENTS.md
├── README.md
├── codex-automation.toml
├── workers/
│   └── repo-discovery.toml
├── targets/
│   └── <target-id>.toml
└── reports/
```

## Bootstrap

Initialize a control workspace and register a target:

```bash
codex-automation workspace init ./codex-automation
codex-automation target add my-app --repo ./target-repo --workspace ./codex-automation
codex-automation worker add my-app --from-file ./codex-automation/workers/repo-discovery.toml
codex-automation target pack my-app --json
codex-automation heartbeat run my-app --json
codex-automation db doctor --json
codex-automation target status my-app --json
```

For agent-first setup from a local path or Git URL:

```bash
python3 scripts/setup.py <target-path-or-git-url> --workspace ./codex-automation --profile balanced
```

The setup script clones or resolves the target, runs doctor checks, initializes
or reuses the thin control workspace, registers the target in SQLite, and prints
handoff information for the supervising agent.

## Result Recording

Workers should report through the CLI instead of writing control-plane state by
hand:

```bash
codex-automation result submit my-app \
  --workorder-id <id> \
  --status fixed \
  --summary "Applied the focused fix." \
  --next-action no_action
```

Existing JSON result artifacts can be imported:

```bash
codex-automation result submit my-app --from-file result.json
codex-automation result list my-app --json
```

## Control Loop

Register a worker definition from the control workspace:

```bash
codex-automation worker add my-app --from-file ./codex-automation/workers/repo-discovery.toml
codex-automation worker list my-app --json
```

Worker TOML defines role, skills, allowed workorder kinds, sandbox, approval
policy, autonomy profile, concurrency, and operating instructions.

Create and inspect workorders:

```bash
codex-automation workorder create my-app \
  --id inspect-1 \
  --kind repo_discovery \
  --title "Inspect target repository" \
  --payload-json '{"scope":"read_only"}'
codex-automation workorder list my-app --json
```

Run one control-plane step:

```bash
codex-automation target pack my-app --json
codex-automation heartbeat run my-app --json
```

Plan a runner handoff, optionally execute it, and record approvals:

```bash
codex-automation heartbeat run my-app --json
CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run my-app --execute --json
codex-automation runner refresh my-app --json
codex-automation runner list my-app --json
codex-automation runner status my-app <runner-id> --json
codex-automation approval request my-app --workorder-id inspect-1 --reason "Needs operator decision" --json
codex-automation approval record my-app approval-inspect-1 --decision approved --message "Approved" --json
```

Heartbeat creates runner packages under the OS app-state artifacts directory.
Each package contains `prompt.md`, `result.schema.json`, and `command.json`.
`--execute` starts `codex exec` only when
`CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1` is present and the selected worker
has available concurrency. The launcher does not pass a model override; Codex
uses the local user configuration. `runner refresh` tracks PID state and can
ingest a worker's final JSON result.

## Current Core

- Rust binary CLI with no Python runtime requirement for normal operation
- OS-specific app state directory with `CODEX_AUTOMATION_HOME` override
- SQLite schema for workspaces, targets, workers, workorders, results, events,
  loop runs, runner packages, and approvals
- thin Codex App control workspace generation
- target registration without modifying the target repo
- workspace-local worker TOML definitions
- worker add/list/status
- workorder creation/list/status
- target pack generation
- one-step loop planning and heartbeat orchestration
- runner package generation, worker concurrency enforcement, gated Codex exec
  launch, refresh, list, and status
- approval request/list/record
- transactional result submission and listing
- setup skill and agent-first setup script
- allowlisted public export for publishing from a private maintainer checkout

Upcoming control-plane work should build on the SQLite boundary: skill pack
export/install, richer workorder generation, and app update flows should become
first-class CLI operations instead of target-local files.

## Setup Skill

The setup skill lives at:

```text
skills/codex-automation-setup/
```

Codex does not automatically load skills from a cloned repository. Install the
skill into `$CODEX_HOME/skills` with the built-in `skill-installer`, then
restart Codex:

```text
Install the codex-automation-setup skill from github.com/<owner>/codex-automation at skills/codex-automation-setup.
```

After restart, ask:

```text
Use codex-automation-setup for this repository.
```

## Test

```bash
cargo fmt --all -- --check
cargo test --workspace
```

Build and inspect a sanitized public tree:

```bash
python3 scripts/build_public_export.py --output .public-export/codex-automation --overwrite
python3 scripts/build_public_export.py --output .public-export/codex-automation --check-only
```

Do not publish private runtime state directly. Public release artifacts should
include only the files listed in `MANIFEST.public.json`.

## Maintainer Verification

The developer verification skill lives at:

```text
skills/codex-automation-dev/
```

Use it before publishing or release work:

```bash
python3 skills/codex-automation-dev/scripts/verify_clean_install.py --repo . --json
python3 skills/codex-automation-dev/scripts/verify_skill_install.py --repo . --install-setup-skill --install-dev-skill --overwrite --json
```

For disposable Linux install verification, run Docker explicitly:

```bash
bash skills/codex-automation-dev/scripts/verify_docker_install.sh .
```

The Docker verifier uses a temporary container and must not be confused with
target app service startup. It does not push, deploy, or run real workers.
