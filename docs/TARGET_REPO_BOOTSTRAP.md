# Target Repo Bootstrap

Bootstrap registers a target repository without writing runtime state into the
target repo.

## Shape

```text
~/workspace/codex-automation/       # thin Codex App control workspace
├── AGENTS.md
├── README.md
├── codex-automation.toml
├── workers/
│   ├── control-plane.toml
│   ├── repo-maintainer.toml
│   ├── ops-analyst.toml
│   └── release-manager.toml
├── targets/
│   └── my-app.toml
└── reports/

OS app data/codex-automation/
├── codex-automation.sqlite
├── worktrees/
│   └── my-app/                    # shared worker worktree
├── logs/
├── artifacts/
└── backups/

~/workspace/target-repo/            # target repository
```

The source checkout is only needed by maintainers. End users install the binary
and can put the control workspace anywhere, for example
`~/project01/codex-automation`.

The target repository is not modified by `workspace init` or `target add`.
Runner dispatch materializes a shared Git worktree under OS app data and hands
that worktree to Codex workers as their working directory. The canonical target
repository remains the integration source and should not be edited directly by
workers. Dispatch requires the target to be a Git checkout with at least one
commit.

## Commands

Install the setup skill from GitHub Releases:

```bash
curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh
```

The setup skill installs the released binary when `codex-automation` is not
already available on `PATH`.

Confirm the binary and inspect existing state before setup:

```bash
codex-automation --version
codex-automation doctor --json
codex-automation db doctor --json
codex-automation paths --json
codex-automation target list --json
```

If `paths --json` shows `control_workspace: null`, no workspace was supplied to
that command. Pass `--workspace <control-workspace>` when inspecting a chosen
control workspace.
If an older release binary rejects `--version`, continue with `--help` and
`doctor --json`, then update the binary.

Preview setup before writing:

```bash
codex-automation init ~/workspace/target-repo --workspace ~/workspace/codex-automation --profile balanced --plan --json
```

One-command setup:

```bash
codex-automation init ~/workspace/target-repo --workspace ~/workspace/codex-automation --profile balanced --json
```

Use one-command setup only for a new target registration. If `target list
--json` already shows the target, use the update flow instead. `init` creates
the first workorder and Codex App handoff package, but it does not launch
detached or headless Codex processes. The handoff points Codex App at the
shared worktree, not the canonical target repository.
The handoff includes the resolved binary path for agents whose shell does not
have `codex-automation` on `PATH`.

Manual setup:

```bash
codex-automation workspace init ~/workspace/codex-automation
codex-automation paths --workspace ~/workspace/codex-automation --json
codex-automation target add my-app --repo ~/workspace/target-repo --workspace ~/workspace/codex-automation
codex-automation target pack my-app --json
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/repo-maintainer.toml
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/ops-analyst.toml
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/release-manager.toml
codex-automation db doctor --json
codex-automation target status my-app --json
codex-automation heartbeat run my-app --json
```

Target packs include Git branch/head/dirty counts when the target is a Git
checkout. Treat dirty state as a context signal; setup still must not edit the
target repo. The shared worktree is created from the committed target `HEAD`;
uncommitted target changes are reported as context but are not copied into the
worktree.

Customize `workers/control-plane.toml`, runnable `workers/*.toml`, and
`targets/my-app.toml` before execution when the target needs local policy.
Preview the merged prompt with:

```bash
codex-automation prompt render my-app --workorder-id <id> --worker repo-maintainer --json
```

For a one-command setup from a local path or Git URL:

```bash
codex-automation init <target-path-or-git-url> --workspace ~/workspace/codex-automation --clone-dir ~/workspace/checkouts --profile balanced --json
```

## Result Submission

Worker results should be recorded through the CLI:

```bash
codex-automation result submit my-app \
  --workorder-id <id> \
  --status fixed \
  --summary "..." \
  --next-action no_action
```

The CLI validates required fields, known status values, target registration, and
records one SQLite transaction. JSON files remain useful as import/export
artifacts:

```bash
codex-automation result submit my-app --from-file result.json
```

## Reset Or Uninstall

Preview removal:

```bash
codex-automation uninstall --workspace ~/workspace/codex-automation --json
```

Remove generated automation state and the setup skill:

```bash
codex-automation uninstall \
  --remove-app-state \
  --remove-skills \
  --remove-control-workspace \
  --workspace ~/workspace/codex-automation \
  --yes \
  --json
```

Target repositories are never removed by uninstall.

## Update

Refresh the setup skill, replace the binary, migrate the local database, and
dry-run the control loop:

```bash
curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh
python3 ~/.codex/skills/codex-automation-setup/scripts/update.py \
  --workspace ~/workspace/codex-automation \
  --target-id my-app \
  --json
```

For validation without replacing the binary:

```bash
codex-automation update --workspace ~/workspace/codex-automation --target-id my-app --check --json
```
