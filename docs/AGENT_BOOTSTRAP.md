# Agent Bootstrap

This document is for an AI agent that has pulled `codex-automation` and needs
to bring a target repository under automation.

## Required Order

1. If working from a source checkout, inspect this repository with
   `git status --short` and verify the Rust CLI with `cargo test --workspace`.
2. If installing as an end user, install the release binary:
   `curl -fsSL https://raw.githubusercontent.com/route2048/codex-automation/main/scripts/install.sh | sh`.
3. If developing from source, install or expose the binary:
   `cargo install --path crates/codex-automation-cli --locked`.
4. Install the bundled setup skill:
   `codex-automation skill install codex-automation-setup --json`.
5. Run:
   `codex-automation doctor --json`.
6. Run:
   `codex-automation db doctor --json`.
7. Prefer the one-command bootstrap:
   `codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --json`.
8. If manual setup is required, pick or create a thin control workspace for Codex App:
   `codex-automation workspace init <control-workspace> --json`.
9. Register the target repo without writing runtime state into it:
   `codex-automation target add <id> --repo <target-repo> --workspace <control-workspace> --json`.
10. Load the default runnable worker definitions:
   `codex-automation worker add <id> --from-file <control-workspace>/workers/repo-maintainer.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/ops-analyst.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/release-manager.toml --json`.
11. Generate repository context with:
    `codex-automation target pack <id> --json`.
12. Start the first bounded control-plane heartbeat with:
    `codex-automation heartbeat run <id> --json`.
13. Inspect target registration:
   `codex-automation target status <id> --json`.
14. Inspect runner packages with:
    `codex-automation runner list <id> --json`.
15. Open the control workspace in Codex App. Continue from `README.md`,
   `AGENTS.md`, `workers/control-plane.toml`, and `targets/<id>.toml`.
16. Record worker results through:
   `codex-automation result submit <id> ... --json`.
17. Start a detached worker only when explicitly allowed:
    `CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run <id> --execute --json`.
18. Refresh runner state with:
    `codex-automation runner refresh <id> --json`.
19. Use `result submit --from-file result.json` only when a worker has already
    produced a result artifact.
20. Use `codex-automation prompt render <id> --workorder-id <workorder> --worker <worker-id> --json`
    to verify custom instructions before execution.
21. To reset an installation, dry-run first:
    `codex-automation uninstall --workspace <control-workspace> --json`.
22. Remove generated runtime state only with explicit flags:
    `codex-automation uninstall --remove-app-state --remove-skills --remove-control-workspace --workspace <control-workspace> --yes --json`.

For a one-command bootstrap from a local path or Git URL:

```bash
codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --json
```

## Handoff Boundary

The setup skill and CLI prepare a thin local control workspace plus app-managed
SQLite state. Runtime state, worktrees, logs, runner data, and artifacts belong
in the OS app data directory, not in the target repository and not in the human
control workspace.
