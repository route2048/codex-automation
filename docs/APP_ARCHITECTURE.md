# CLI App Architecture

`codex-automation` is a CLI-first local application.

## Directory Roles

```text
codex-automation/              # thin Codex App control workspace
target-repo/                   # product or OSS repository being automated
OS app data/codex-automation/  # SQLite, worktrees, logs, artifacts, backups
```

The source checkout is a maintainer concern. Installed users only need the
binary, a control workspace, target repositories, and app-managed state.

The control workspace exists so a human and Codex App have a clean project to
open. It should contain only human-facing config and reports:

```text
codex-automation/
├── AGENTS.md
├── README.md
├── codex-automation.toml
├── workers/
│   ├── control-plane.toml
│   ├── repo-maintainer.toml
│   ├── ops-analyst.toml
│   └── release-manager.toml
├── targets/
│   └── <target-id>.toml
└── reports/
```

Heavy runtime state does not belong there.

Default control-workspace files are stored as normal source templates under
`crates/codex-automation-core/templates/control-workspace/` and embedded into
the binary with `include_str!`. This keeps the distributed binary
self-contained while making worker definitions and generated workspace files
easy to review in the source tree.

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

Use `codex-automation paths --json` to inspect the resolved app-state paths.
When a control workspace is available, include it to see both sides of the app:

```bash
codex-automation paths --workspace ~/workspace/codex-automation --json
```

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
codex-automation skill install codex-automation-setup --json
codex-automation init ~/workspace/target-repo --workspace ~/workspace/codex-automation --profile balanced --json
```

Manual bootstrap uses the same lower-level commands:

```bash
codex-automation workspace init ~/workspace/codex-automation
codex-automation paths --workspace ~/workspace/codex-automation --json
codex-automation target add my-app --repo ~/workspace/target-repo --workspace ~/workspace/codex-automation
codex-automation target pack my-app --json
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/repo-maintainer.toml
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/ops-analyst.toml
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/release-manager.toml
codex-automation db doctor --json
codex-automation heartbeat run my-app --json
codex-automation prompt render my-app --workorder-id demo --worker repo-maintainer --json
CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run my-app --execute --json
codex-automation runner refresh my-app --json
codex-automation result submit my-app --workorder-id demo --status fixed --summary "..." --next-action no_action
```

The target repository is not modified during this bootstrap path. Runner
packages are created under the OS app-state `artifacts/runners/` directory.
Runner logs live under OS app-state `logs/runners/`.

## Install And Uninstall Boundary

Released users install the binary from GitHub Releases, `scripts/install.sh`,
Homebrew, or another package manager. The binary can then install the embedded
setup skill and initialize targets.

`codex-automation uninstall` owns only codex-automation-created runtime files:

```bash
codex-automation uninstall --workspace ~/workspace/codex-automation --json
codex-automation uninstall \
  --remove-app-state \
  --remove-skills \
  --remove-control-workspace \
  --workspace ~/workspace/codex-automation \
  --yes \
  --json
```

It never removes target repositories. It does not remove the binary because
that belongs to the installer or package manager used to install it.
