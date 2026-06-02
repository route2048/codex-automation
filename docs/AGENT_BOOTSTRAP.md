# Agent Bootstrap

This document is for an AI agent that has pulled `codex-automation` and needs
to bring a target repository under automation.

## Required Order

1. Inspect this repository with `git status --short`.
2. Verify the Rust CLI with `cargo test --workspace`.
3. Run:
   `cargo run --quiet -p codex-automation-cli --bin codex-automation -- doctor --json`.
4. Run:
   `cargo run --quiet -p codex-automation-cli --bin codex-automation -- db doctor --json`.
5. Pick or create a thin control workspace for Codex App:
   `codex-automation workspace init <control-workspace> --json`.
6. Register the target repo without writing runtime state into it:
   `codex-automation target add <id> --repo <target-repo> --workspace <control-workspace> --json`.
7. Load the default worker definition:
   `codex-automation worker add <id> --from-file <control-workspace>/workers/repo-discovery.toml --json`.
8. Generate repository context with:
    `codex-automation target pack <id> --json`.
9. Start the first bounded control-plane heartbeat with:
    `codex-automation heartbeat run <id> --json`.
10. Inspect target registration:
   `codex-automation target status <id> --json`.
11. Inspect runner packages with:
    `codex-automation runner list <id> --json`.
12. Open the control workspace in Codex App. Continue from `README.md`,
   `AGENTS.md`, and `targets/<id>.toml`.
13. Record worker results through:
   `codex-automation result submit <id> ... --json`.
14. Start a detached worker only when explicitly allowed:
    `CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run <id> --execute --json`.
15. Refresh runner state with:
    `codex-automation runner refresh <id> --json`.
16. Use `result submit --from-file result.json` only when a worker has already
    produced a result artifact.

For a one-command bootstrap from a local path or Git URL:

```bash
python3 scripts/setup.py <target-path-or-git-url> --workspace <control-workspace> --profile balanced
```

## Handoff Boundary

The setup skill and CLI prepare a thin local control workspace plus app-managed
SQLite state. Runtime state, worktrees, logs, runner data, and artifacts belong
in the OS app data directory, not in the target repository and not in the human
control workspace.
