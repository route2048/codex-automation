# Target Repo Bootstrap

Bootstrap registers a target repository without writing runtime state into the
target repo.

## Shape

```text
~/workspace/codex-automation-src/   # source checkout
~/workspace/codex-automation/       # thin Codex App control workspace
├── AGENTS.md
├── README.md
├── codex-automation.toml
├── workers/
│   └── repo-discovery.toml
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

The target repository is not modified by `workspace init` or `target add`.

## Commands

```bash
codex-automation workspace init ~/workspace/codex-automation
codex-automation target add my-app --repo ~/workspace/target-repo --workspace ~/workspace/codex-automation
codex-automation target pack my-app --json
codex-automation worker add my-app --from-file ~/workspace/codex-automation/workers/repo-discovery.toml
codex-automation db doctor --json
codex-automation target status my-app --json
codex-automation heartbeat run my-app --json
```

For a one-command setup from a local path or Git URL:

```bash
python3 scripts/setup.py <target-path-or-git-url> --workspace ~/workspace/codex-automation --profile balanced
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
