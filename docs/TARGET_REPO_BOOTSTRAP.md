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
├── logs/
├── artifacts/
└── backups/

~/workspace/target-repo/            # target repository
```

The source checkout is only needed by maintainers. End users install the binary
and can put the control workspace anywhere, for example
`~/project01/codex-automation`.

The target repository is not modified by `workspace init` or `target add`.

## Commands

Install the setup skill from GitHub Releases:

```bash
curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh
```

The setup skill installs the released binary when `codex-automation` is not
already available on `PATH`.

One-command setup:

```bash
codex-automation init ~/workspace/target-repo --workspace ~/workspace/codex-automation --profile balanced --json
```

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
