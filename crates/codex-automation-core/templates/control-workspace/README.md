# {{workspace_name}}

This is a thin codex-automation control workspace for Codex App.

Human-facing config lives here. App-managed state, SQLite, worktrees, logs,
runner data, and artifacts live in the OS application data directory.

Useful commands:

```bash
codex-automation db doctor --json
codex-automation target list --json
codex-automation worker list <target-id> --json
codex-automation target pack <target-id> --json
codex-automation heartbeat run <target-id> --json
codex-automation runner list <target-id> --json
codex-automation prompt render <target-id> --workorder-id <workorder-id> --worker repo-maintainer --json
codex-automation result list <target-id> --json
```

If `codex-automation` is not on `PATH`, use the `binary_path` printed by
`codex-automation init` or the absolute command recorded in runner
`handoff.md`.

Customize the automation by editing `workers/control-plane.toml`,
`workers/*.toml`, and `targets/*.toml`. Reload changed runnable workers with
`codex-automation worker add <target-id> --from-file workers/<worker>.toml`.

Runner handoffs include a `working_directory` field. Open that shared worktree
for worker execution; do not edit the canonical target repository path
directly.
