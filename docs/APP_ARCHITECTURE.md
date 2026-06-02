# CLI App Architecture

`codex-automation` is a CLI-first local application.

## Directory Roles

```text
codex-automation-src/          # source checkout for this project
codex-automation/              # thin Codex App control workspace
target-repo/                   # product or OSS repository being automated
OS app data/codex-automation/  # SQLite, worktrees, logs, artifacts, backups
```

The control workspace exists so a human and Codex App have a clean project to
open. It should contain only human-facing config and reports:

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

Heavy runtime state does not belong there.

## App State

Durable application state lives in the OS app data directory:

```text
macOS:   ~/Library/Application Support/codex-automation/
Linux:   $XDG_STATE_HOME/codex-automation/ or ~/.local/state/codex-automation/
Windows: %LOCALAPPDATA%/codex-automation/
```

The app state root contains:

```text
codex-automation.sqlite
worktrees/
logs/
artifacts/
backups/
```

`CODEX_AUTOMATION_HOME` overrides this location for tests and advanced local
development.

## SQLite Boundary

SQLite is the source of truth for:

- registered control workspaces
- registered targets
- worker definitions
- workorders
- result submissions
- event history
- runner package records
- approval records
- loop runs

Worker result JSON is an import/export artifact. The primary operation is:

```bash
codex-automation result submit <target-id> ...
```

This lets the CLI validate required fields, status values, target existence,
and state transitions inside one transaction.

## Bootstrap

```bash
codex-automation workspace init ~/workspace/codex-automation
codex-automation target add my-app --repo ~/workspace/target-repo --workspace ~/workspace/codex-automation
codex-automation target pack my-app --json
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/repo-discovery.toml
codex-automation db doctor --json
codex-automation heartbeat run my-app --json
CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run my-app --execute --json
codex-automation runner refresh my-app --json
codex-automation result submit my-app --workorder-id demo --status fixed --summary "..." --next-action no_action
```

The target repository is not modified during this bootstrap path. Runner
packages are created under the OS app-state `artifacts/runners/` directory.
Runner logs live under OS app-state `logs/runners/`.
