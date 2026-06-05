---
name: codex-automation-setup
description: Install the codex-automation binary when needed and bootstrap or update codex-automation for a target repository, including clone or pull guidance, app database checks, thin Codex App workspace setup, target registration, result CLI guidance, and control-plane handoff.
metadata:
  short-description: Set up codex-automation for a repo
---

# codex-automation-setup

Use this skill when a user asks to install, update, bootstrap, or hand off a
repository to `codex-automation`.

## Workflow

1. Confirm the target repository path or Git URL from the user request.
2. Ensure the binary is available. If `codex-automation` is not on `PATH`, run
   this skill's installer:
   `python3 <this-skill>/scripts/install_binary.py --json`.
   When testing an unpublished build, set `CODEX_AUTOMATION_BIN=<path-to-binary>`.
3. Run the doctor command:
   `codex-automation doctor --json`.
4. Run the app database check:
   `codex-automation db doctor --json`.
5. Inspect the resolved state and control-workspace paths:
   `codex-automation paths --json`.
6. For a one-command agent-first install, run:
   `codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --json`.
   Use `--profile observe`, `suggest`, `aggressive`, or `release` only when the
   user or target policy calls for that autonomy level.
7. Manual setup path:
   `codex-automation workspace init <control-workspace> --json`.
8. Register the target repo:
   `codex-automation target add <id> --repo <target> --workspace <control-workspace> --profile balanced --json`.
9. Load the default runnable worker definitions:
   `codex-automation worker add <id> --from-file <control-workspace>/workers/repo-maintainer.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/ops-analyst.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/release-manager.toml --json`.
10. Generate repository context:
   `codex-automation target pack <id> --json`.
11. Run the first heartbeat:
   `codex-automation heartbeat run <id> --json`.
12. Inspect the registered target:
   `codex-automation target status <id> --json`.
13. Record worker results with:
    `codex-automation result submit <id> --workorder-id <workorder> --status fixed --summary "..." --next-action no_action --json`.
14. Start detached execution only when explicitly requested:
    `CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run <id> --execute --json`.
15. Refresh runner state with:
    `codex-automation runner refresh <id> --json`.
16. Re-inspect paths after setup:
    `codex-automation paths --workspace <control-workspace> --json`.
17. Open the thin control workspace in Codex App and continue from `README.md`,
    `AGENTS.md`, `workers/control-plane.toml`, and `targets/<id>.toml`.
18. To reset a local setup, preview with:
    `codex-automation uninstall --workspace <control-workspace> --json`.
19. Remove generated state only with explicit flags:
    `codex-automation uninstall --remove-app-state --remove-skills --remove-control-workspace --workspace <control-workspace> --yes --json`.

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
- Do not ask the user to install the binary manually when this skill can install
  a released binary itself.

When the user provides a Git URL instead of a local path, prefer:

```bash
codex-automation init <git-url> --workspace <control-workspace> --clone-dir <checkout-dir> --json
```

The init command clones the target or fast-forwards an existing checkout, runs
doctor and database checks, initializes or reuses the control workspace,
registers the target, and prints handoff details.

## Update Flow

For app updates, refresh this setup skill from the latest GitHub Release first:

```bash
curl -fsSL https://github.com/route2048/codex-automation/releases/latest/download/install-skill.sh | sh
```

Then run this skill's updater:

```bash
python3 <this-skill>/scripts/update.py --workspace <control-workspace> --target-id <id> --json
```

The updater downloads the latest release binary, runs `codex-automation update
--json`, applies database migrations, checks app paths, lists registered
targets, regenerates the target pack when `--target-id` is provided, and runs a
heartbeat dry-run. It must not start detached runners during update.

When testing an unpublished build, set `CODEX_AUTOMATION_BIN=<path-to-binary>`;
the updater then skips binary replacement and validates state with that binary.

## Uninstall Flow

`codex-automation uninstall` removes only codex-automation-owned local state.
It can remove app-state, the generated setup skill, and a generated control
workspace when explicitly requested with `--yes`. It must never delete target
repositories. The installed binary is owned by the installer or package manager
and is not removed by this command.
