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

1. Confirm the target repository path or Git URL from the user request. Local
   target paths must be Git checkouts with at least one commit before worker
   handoff can create the shared worktree.
2. Resolve the binary command. First run:
   `command -v codex-automation || true`.
   If the binary is missing, run this skill's installer:
   `python3 <this-skill>/scripts/install_binary.py --json`.
   Use the `binary` path from the installer JSON for the remaining commands, or
   export `CODEX_AUTOMATION_BIN=<binary>`. When testing an unpublished build,
   set `CODEX_AUTOMATION_BIN=<path-to-binary>` before running any command.
3. Confirm the resolved binary with:
   `codex-automation --version`.
   If the binary is not on `PATH`, use the absolute binary path returned by the
   installer. `codex-automation --help` is also acceptable when diagnosing an
   older release binary that does not support `--version`; record that as a
   release/source contract mismatch and continue with `doctor --json`.
4. Run setup checks sequentially, not in parallel:
   `codex-automation doctor --json`.
   `codex-automation db doctor --json`.
   `codex-automation paths --json`.
   `paths --json` reports OS app-state paths. Its `control_workspace` field is
   `null` unless `--workspace <control-workspace>` is provided; this is expected
   and does not mean setup failed.
5. Inspect existing targets:
   `codex-automation target list --json`.
   If the requested target is already registered, prefer the Update Flow below.
   If it is not registered, continue with New Setup.
6. Choose a control workspace. Prefer an explicit user-visible directory that
   Codex App can open. If no workspace is specified and the user wants the CLI
   default, `init` uses `codex-automation` under the current directory.
   If that directory already exists and looks like a source repository, choose
   another empty/generated control workspace instead of writing into it.
7. Prefer a preview before writes when the workspace is implicit, already
   exists, or could collide with a source checkout:
   `codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --plan --json`.

## New Setup

1. For a one-command agent-first install, run:
   `codex-automation init <target-path-or-git-url> --workspace <control-workspace> --profile balanced --json`.
   Use `--profile observe`, `suggest`, `aggressive`, or `release` only when the
   user or target policy calls for that autonomy level.
2. Manual setup path:
   `codex-automation workspace init <control-workspace> --json`.
3. Register the target repo:
   `codex-automation target add <id> --repo <target> --workspace <control-workspace> --profile balanced --json`.
4. Load the default runnable worker definitions:
   `codex-automation worker add <id> --from-file <control-workspace>/workers/repo-maintainer.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/ops-analyst.toml --json`.
   `codex-automation worker add <id> --from-file <control-workspace>/workers/release-manager.toml --json`.
5. Generate repository context:
   `codex-automation target pack <id> --json`.
6. Run the first heartbeat:
   `codex-automation heartbeat run <id> --json`.
   `init` also runs this first heartbeat. It may report `dry_run: false`, but
   setup must not launch Codex processes; the expected effect is a Codex App
   handoff package for a worker thread and a shared Git worktree under OS app
   data. The worker should open the runner package `working_directory`, not the
   canonical target repository path.
   The setup output and runner package include the resolved `binary_path`; use
   that absolute path when `codex-automation` is not on `PATH`.
   Inspect `target_git.before`, `target_git.after`, and
   `target_git.unchanged` in the setup JSON before concluding setup touched the
   target repo. Dirty target state should be treated as pre-existing context
   when `unchanged: true`; uncommitted target changes are not copied into the
   shared worktree.
7. Inspect the registered target:
   `codex-automation target status <id> --json`.
8. Record worker results with:
    `codex-automation result submit <id> --workorder-id <workorder> --status fixed --summary "..." --next-action no_action --json`.
9. Hand the generated runner package to Codex App. Inspect:
    `codex-automation runner list <id> --json`.
10. Refresh runner state after a result is submitted or saved to the package
    `result.json`:
    `codex-automation runner refresh <id> --json`.
11. Re-inspect paths after setup:
    `codex-automation paths --workspace <control-workspace> --json`.
12. Open the thin control workspace in Codex App for orchestration. Open the
    runner package `working_directory` path for worker execution.
13. To reset a local setup, preview with:
    `codex-automation uninstall --workspace <control-workspace> --json`.
14. Remove generated state only with explicit flags:
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
- Do not start detached or headless Codex runners during setup. The supported
  path is Codex App handoff plus result ingestion.
- Do not ask the user to install the binary manually when this skill can install
  a released binary itself.

When the user provides a Git URL instead of a local path, prefer:

```bash
codex-automation init <git-url> --workspace <control-workspace> --clone-dir <checkout-dir> --json
```

The init command clones the target or fast-forwards an existing checkout, runs
doctor and database checks, initializes or reuses the control workspace,
registers the target, creates the initial workorder and Codex App handoff
package, and prints handoff details.

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
Binary download checksum verification is fail-closed by default. Use
`--allow-missing-checksum` only for trusted local testing.

When testing an unpublished build, set `CODEX_AUTOMATION_BIN=<path-to-binary>`;
the updater then skips binary replacement and validates state with that binary.

Use this flow when `target list --json` shows the target is already registered.
For read-only validation without replacing the binary, pass `--check`.

## Uninstall Flow

`codex-automation uninstall` removes only codex-automation-owned local state.
It can remove app-state, the generated setup skill, and a generated control
workspace when explicitly requested with `--yes`. It must never delete target
repositories. The installed binary is owned by the installer or package manager
and is not removed by this command.

## Reporting

Do not write a target-repository handoff report solely for setup, even when the
target repository's `AGENTS.md` requires reports for source edits. Setup does
not edit target source files. If the user asks for a setup audit trail, write it
under the thin control workspace `reports/` directory.
