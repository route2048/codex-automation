# Agent Bootstrap

This document is for an AI agent that has pulled `codex-automation` and needs
to bring a target repository under automation.

## Required Order

1. If working from a source checkout, inspect this repository with
   `git status --short` and verify the Rust CLI with `cargo test --workspace`.
2. If installing as an end user, install the setup skill:
   `curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh`.
3. If developing from source, install or expose the binary:
   `cargo install --path crates/codex-automation-cli --locked`.
4. If the binary is not available, let the setup skill install it:
   `python3 <codex-automation-setup>/scripts/install_binary.py --json`.
   Use the `binary` path from the installer JSON when the install directory is
   not on `PATH`, or export `CODEX_AUTOMATION_BIN=<binary>`.
5. Confirm the binary:
   `codex-automation --version`.
   If an older release binary rejects `--version`, run `codex-automation
   --help` and continue with `doctor --json`; record the release/source
   contract mismatch.
6. Run these checks sequentially:
   `codex-automation doctor --json`.
   `codex-automation db doctor --json`.
   `codex-automation paths --json`.
   `paths --json` shows `control_workspace: null` until a workspace is supplied
   with `--workspace`; that is expected.
7. Inspect existing state:
   `codex-automation target list --json`.
   If the target is already registered, use the setup skill updater instead of
   running a fresh init.
8. Prefer the one-command bootstrap:
   `codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --json`.
9. Preview before writing when needed:
   `codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --plan --json`.
   Use this before writing into a workspace path that already exists or may be a
   source checkout. The plan reports workspace collision status and target Git
   baseline.
10. If manual setup is required, pick or create a thin control workspace for Codex App:
   `codex-automation workspace init <control-workspace> --json`.
11. Register the target repo without writing runtime state into it:
   `codex-automation target add <id> --repo <target-repo> --workspace <control-workspace> --json`.
12. Load the default runnable worker definitions:
   `codex-automation worker add <id> --from-file <control-workspace>/workers/repo-maintainer.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/ops-analyst.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/release-manager.toml --json`.
13. Generate repository context with:
    `codex-automation target pack <id> --json`.
    Target packs include Git branch/head/dirty counts when available.
14. Start the first bounded control-plane heartbeat with:
    `codex-automation heartbeat run <id> --json`.
    `init` also runs this first heartbeat. It creates a Codex App handoff
    package; it must not launch detached or headless Codex processes.
    Check `target_git.before`, `target_git.after`, and `target_git.unchanged`
    in setup output before claiming whether setup changed the target checkout.
15. Inspect target registration:
   `codex-automation target status <id> --json`.
16. Inspect runner packages with:
    `codex-automation runner list <id> --json`.
17. Open the control workspace in Codex App. Continue from `README.md`,
   `AGENTS.md`, `workers/control-plane.toml`, and `targets/<id>.toml`.
18. Record worker results through:
   `codex-automation result submit <id> ... --json`.
   If the command is not on `PATH`, use the `binary_path` printed by `init` or
   the absolute command recorded in runner `handoff.md`.
19. Create a Codex App handoff package:
    `codex-automation heartbeat run <id> --json`.
20. Refresh runner state after a result is submitted or saved to package
    `result.json`:
    `codex-automation runner refresh <id> --json`.
21. Use `result submit --from-file result.json` only when a worker has already
    produced a result artifact.
22. Use `codex-automation prompt render <id> --workorder-id <workorder> --worker <worker-id> --json`
    to verify custom instructions before execution.
23. To reset an installation, dry-run first:
    `codex-automation uninstall --workspace <control-workspace> --json`.
24. Remove generated runtime state only with explicit flags:
    `codex-automation uninstall --remove-app-state --remove-skills --remove-control-workspace --workspace <control-workspace> --yes --json`.
25. To update an existing install, refresh the setup skill and run the updater:
    `curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh`.
    `python3 ~/.codex/skills/codex-automation-setup/scripts/update.py --workspace <control-workspace> --target-id <id> --json`.

For a one-command bootstrap from a local path or Git URL:

```bash
codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --json
```

## Handoff Boundary

The setup skill and CLI prepare a thin local control workspace plus app-managed
SQLite state. Runtime state, worktrees, logs, runner data, and artifacts belong
in the OS app data directory, not in the target repository and not in the human
control workspace.

Do not write target-repository reports during setup unless target source files
were edited. Setup audit notes belong under the control workspace `reports/`
directory when the user asks for them.
