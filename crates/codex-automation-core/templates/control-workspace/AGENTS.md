# AGENTS.md

This directory is the human-facing codex-automation control workspace.

- Keep heavy runtime state out of this directory.
- Use `codex-automation target list --json` before adding targets.
- Customize orchestration in `workers/control-plane.toml`.
- Customize runnable workers under `workers/` and load them with `codex-automation worker add`.
- Customize target-specific instructions under `targets/*.toml`.
- Use `codex-automation heartbeat run <target-id> --json` for one bounded control-plane step.
- Use `codex-automation prompt render <target-id> --workorder-id <id> --worker <worker-id> --json` to preview the merged prompt.
- Use `codex-automation result submit` to record worker results.
- Do not edit app-managed SQLite, worktrees, logs, or runner state by hand.
