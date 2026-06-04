---
name: codex-automation-setup
description: Bootstrap or update codex-automation for a target repository with the Rust CLI, including clone or pull guidance, app database checks, thin Codex App workspace setup, target registration, result CLI guidance, and control-plane handoff.
metadata:
  short-description: Set up codex-automation for a repo
---

# codex-automation-setup

Use this skill when a user asks to install, update, bootstrap, or hand off a
repository to `codex-automation`.

## Workflow

1. Confirm the target repository path or Git URL from the user request.
2. Ensure the `codex-automation` binary is installed and available on `PATH`.
   When testing an unpublished build, set `CODEX_AUTOMATION_BIN=<path-to-binary>`.
3. Install or refresh this skill from the embedded binary assets:
   `codex-automation skill install codex-automation-setup --json`.
   If it reports `needs_overwrite`, preserve local edits unless the user wants
   the packaged skill version:
   `codex-automation skill install codex-automation-setup --overwrite --json`.
4. Run the doctor command:
   `codex-automation doctor --json`.
5. Run the app database check:
   `codex-automation db doctor --json`.
6. Inspect the resolved state and control-workspace paths:
   `codex-automation paths --json`.
7. For a one-command agent-first install, run:
   `codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --json`.
   Use `--profile observe`, `suggest`, `aggressive`, or `release` only when the
   user or target policy calls for that autonomy level.
8. Manual setup path:
   `codex-automation workspace init <control-workspace> --json`.
9. Register the target repo:
   `codex-automation target add <id> --repo <target> --workspace <control-workspace> --profile balanced --json`.
10. Load the default runnable worker definitions:
   `codex-automation worker add <id> --from-file <control-workspace>/workers/repo-maintainer.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/ops-analyst.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/release-manager.toml --json`.
11. Generate repository context:
   `codex-automation target pack <id> --json`.
12. Run the first heartbeat:
   `codex-automation heartbeat run <id> --json`.
13. Inspect the registered target:
   `codex-automation target status <id> --json`.
14. Record worker results with:
    `codex-automation result submit <id> --workorder-id <workorder> --status fixed --summary "..." --next-action no_action --json`.
15. Start detached execution only when explicitly requested:
    `CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run <id> --execute --json`.
16. Refresh runner state with:
    `codex-automation runner refresh <id> --json`.
17. Re-inspect paths after setup:
    `codex-automation paths --workspace <control-workspace> --json`.
18. Open the thin control workspace in Codex App and continue from `README.md`,
    `AGENTS.md`, `workers/control-plane.toml`, and `targets/<id>.toml`.

## Boundaries

- Do not push, deploy, delete, or edit target source files during setup.
- Keep setup writes inside the thin control workspace and OS app data.
- Do not write queue, worker registry, result state, or worktrees into the
  target repo during setup.
- Treat `codex-automation.toml`, `targets/<id>.toml`, and SQLite as the source
  of truth.
- Do not treat email, silence, or generated summaries as approvals.
- Do not enable auto-commit during setup. It is a post-bootstrap control-plane
  operation and must be represented as an explicit CLI policy.
- Do not start detached runners during setup unless the user explicitly asks
  for execution and the environment gate is set.
- Do not assume this skill is active merely because it exists in the repository.
  It must be installed under `$CODEX_HOME/skills/codex-automation-setup` and
  Codex must be restarted before the skill is discoverable.

When the user provides a Git URL instead of a local path, prefer:

```bash
codex-automation init <git-url> --workspace <control-workspace> --clone-dir <checkout-dir> --json
```

The init command clones the target or fast-forwards an existing checkout, runs
doctor and database checks, installs this skill if needed, initializes or reuses
the control workspace, registers the target, and prints handoff details.

## Update Flow

For app updates, upgrade or reinstall the `codex-automation` binary, run
`codex-automation skill install codex-automation-setup --overwrite --json` when
the setup workflow should be refreshed, then run `codex-automation doctor --json`,
`codex-automation db doctor --json`, and `codex-automation target list --json`.
Future schema migrations should be exposed as explicit `codex-automation db
migrate` commands.
