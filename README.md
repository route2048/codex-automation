# codex-automation

`codex-automation` is a local-first Rust CLI for setting up and supervising
Codex automation against one or more repositories.

This is an unofficial project. It is not affiliated with, endorsed by, or
sponsored by OpenAI.

The app keeps human-facing configuration in a thin Codex App workspace, while
durable runtime state lives in a local SQLite database under the OS application
data directory. Target repositories stay clean during setup.

## Install The Setup Skill

Most users start by installing the Codex setup skill. The skill can then
install the released binary, initialize the control workspace, register a
target repository, and hand off to Codex.

```bash
curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh
```

Restart or open a new Codex thread, then ask:

```text
Use $codex-automation-setup to enable codex-automation for this repository.
```

The setup skill installs the binary from GitHub Releases when
`codex-automation` is not already available on `PATH`.

Verify the installed binary with:

```bash
codex-automation --version
codex-automation doctor --json
codex-automation db doctor --json
codex-automation paths --json
```

`paths --json` reports `control_workspace: null` until a workspace is supplied
with `--workspace`; that is expected during first-run inspection.
If `--version` fails on an older release binary, run `codex-automation --help`
and `codex-automation doctor --json`, then update to a newer release.

Developers can install from source:

```bash
cargo install --path crates/codex-automation-cli --locked
```

During development, run it from the workspace:

```bash
cargo run --quiet -p codex-automation-cli --bin codex-automation -- doctor --json
```

## Layout

For normal use, keep the installed binary, control workspace, app state, and
target repos separate. The control workspace can live in any directory the user
wants Codex App to open:

```text
codex-automation/              # thin Codex App control workspace
target-repo/                   # product or OSS repository being automated
OS app data/codex-automation/  # SQLite, worktrees, logs, artifacts
```

Maintainers may also have a separate `codex-automation-src/` source checkout,
but end users do not need it after installing the binary.

The generated control workspace looks like this:

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

Generated workspace defaults are maintained as normal source files under
`crates/codex-automation-core/templates/control-workspace/` and embedded into
the binary at build time.

## Bootstrap

Initialize a control workspace and register a target in one command:

```bash
codex-automation init <target-path-or-git-url> --workspace ./codex-automation --profile balanced --json
```

Preview the same setup without writing files or cloning Git URLs:

```bash
codex-automation init <target-path-or-git-url> --workspace ./codex-automation --profile balanced --plan --json
```

The plan reports whether the chosen control workspace already exists, whether
it looks like a source checkout, and where app-state artifacts would be written.
`init` refuses to initialize a non-empty directory without
`codex-automation.toml` unless `--overwrite-workspace` is passed.

The init command clones or resolves the target, runs doctor checks,
initializes or reuses the thin control workspace, registers the target in
SQLite, loads the default runnable workers, generates a target pack, runs the
first heartbeat, creates a Codex App handoff package, and prints handoff
information for the supervising agent. The heartbeat does not launch detached
or headless Codex processes. The handoff includes the resolved binary path, so
agents can use an absolute command when `codex-automation` is not on `PATH`.
Setup output includes target Git state before and after setup, plus an
`unchanged` flag.

Manual bootstrap uses the same primitives:

```bash
codex-automation workspace init ./codex-automation
codex-automation target add my-app --repo ./target-repo --workspace ./codex-automation
codex-automation worker add my-app --from-file ./codex-automation/workers/repo-maintainer.toml
codex-automation worker add my-app --from-file ./codex-automation/workers/ops-analyst.toml
codex-automation worker add my-app --from-file ./codex-automation/workers/release-manager.toml
codex-automation target pack my-app --json
codex-automation heartbeat run my-app --json
codex-automation db doctor --json
codex-automation target status my-app --json
```

## Update

Refresh the setup skill from GitHub Releases, then let the skill update the
binary and validate local state:

```bash
curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh
python3 ~/.codex/skills/codex-automation-setup/scripts/update.py \
  --workspace ./codex-automation \
  --target-id my-app \
  --json
```

The updater replaces the release binary, runs `codex-automation update --json`,
applies database migrations, checks registered targets, regenerates the target
pack for the selected target, and runs a heartbeat dry-run without starting
detached workers.

Use update when `codex-automation target list --json` shows the requested target
is already registered.

For state-only validation with the installed binary:

```bash
codex-automation update --workspace ./codex-automation --target-id my-app --check --json
```

## Uninstall

Preview removal first:

```bash
codex-automation uninstall --workspace ./codex-automation --json
```

Remove app-state, the setup skill, and a generated control workspace:

```bash
codex-automation uninstall \
  --remove-app-state \
  --remove-skills \
  --remove-control-workspace \
  --workspace ./codex-automation \
  --yes \
  --json
```

`uninstall` never deletes target repositories. It also does not remove the
binary because binary removal depends on the installer or package manager. For
`scripts/install.sh`, remove the installed file from `~/.local/bin` or the
directory passed with `--install-dir`.

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
codex-automation worker add my-app --from-file ./codex-automation/workers/repo-maintainer.toml
codex-automation worker list my-app --json
```

Worker TOML defines role, skills, allowed workorder kinds, sandbox, approval
policy, autonomy profile, concurrency, and `custom_instructions`. The
orchestration instructions live beside them in `workers/control-plane.toml`;
target-specific instructions live in `targets/<target-id>.toml`.

Create and inspect workorders:

```bash
codex-automation workorder create my-app \
  --id inspect-1 \
  --kind repo_discovery \
  --title "Inspect target repository" \
  --payload-json '{"scope":"read_only"}'
codex-automation workorder list my-app --json
codex-automation prompt render my-app --workorder-id inspect-1 --worker repo-maintainer --json
```

Run one control-plane step:

```bash
codex-automation target pack my-app --json
codex-automation heartbeat run my-app --json
```

Plan a Codex App runner handoff and record approvals:

```bash
codex-automation heartbeat run my-app --json
codex-automation runner refresh my-app --json
codex-automation runner list my-app --json
codex-automation runner status my-app <runner-id> --json
codex-automation approval request my-app --workorder-id inspect-1 --reason "Needs operator decision" --json
codex-automation approval record my-app approval-inspect-1 --decision approved --message "Approved" --json
```

Runner dispatch creates a handoff package for Codex App. `codex-automation`
does not launch headless Codex processes. Workers can record results through
`codex-automation result submit`, or a controller can save the worker's final
JSON object to the package `result.json` and run
`codex-automation runner refresh`.

Heartbeat creates runner handoff packages under the OS app-state artifacts
directory. Each package contains `prompt.md`, `handoff.md`, `result.json`, and
`command.json`. `runner refresh` ingests submitted results or package
`result.json` files.

Target packs include Git branch, head, dirty status, staged/unstaged counts,
and untracked counts when the target is a Git checkout.

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
- runner handoff package generation, refresh, list, and status
- approval request/list/record
- transactional result submission and listing
- safe uninstall planning/removal for app-state, setup skill, and generated
  control workspaces
- `paths --json` for inspecting control-workspace and app-state locations
- agent-first `init` command
- first-class `update` command for database migration, target validation,
  target-pack refresh, and heartbeat dry-run

Upcoming control-plane work should build on the SQLite boundary: richer
workorder generation should remain first-class CLI operations instead of
target-local files.

## Setup Skill

End users only install the setup skill:

```text
skills/codex-automation-setup/
```

Install the skill into `$CODEX_HOME/skills` from GitHub Releases, then restart
or open a new Codex thread:

```bash
curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh
```

After restart, ask:

```text
Use codex-automation-setup for this repository.
```

To inspect where a setup wrote control and app-state files:

```bash
codex-automation paths --workspace ./codex-automation --json
```

Do not install `codex-automation-dev` or any maintainer release skill in normal
user environments. They are for project verification and publication work, not
for bootstrapping target repositories.

## Test

```bash
cargo fmt --all -- --check
cargo test --workspace
```

## Maintainer Verification

The developer verification skill is public for maintainers and contributors,
but it is not part of the end-user setup path:

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

The release operator skill is intentionally private and is not shipped in this
repository. It owns public export, push, tag, GitHub Actions monitoring, and
release-asset smoke checks for maintainers.
