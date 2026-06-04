# Skill System

`codex-automation` treats skills as reusable worker operating manuals.

The setup entrypoint is `skills/codex-automation-setup/`. It bootstraps this
tool and hands ongoing work to the CLI control surface.

The maintainer verification entrypoint is `skills/codex-automation-dev/`. It
runs release-readiness checks for this project itself.

## Implemented

- `codex-automation-setup`: install, doctor, initialize a thin control
  workspace, register target repos, and explain result submission.
- `codex-automation-dev`: verify Rust tests, clean local install, optional
  Docker install, setup skill installation, and fixture dry-run smoke before
  publication.
- `control-plane`: orchestration instructions generated under
  `workers/control-plane.toml`.
- `repo-maintainer`: default runnable worker for discovery, focused fixes, and
  verification.
- `ops-analyst`: read-only diagnostics worker for logs, CI, incidents, and
  operational evidence.
- `release-manager`: read-only release-readiness worker with approval gates for
  publishing, deployment, and update decisions.

## Worker Model

Keep the default worker set small. Route most source work through
`repo-maintainer`, diagnostic evidence gathering through `ops-analyst`, and
release/update planning through `release-manager`. Add new workers only when a
target repeatedly needs a distinct sandbox, approval policy, or expertise
boundary.

Skills are selected through worker definitions. Runner packages include only
the relevant skill names, worker boundaries, and the current workorder
contract.

Custom instructions live in the same TOML files that define the actor:
`workers/control-plane.toml`, runnable `workers/*.toml`, and
`targets/<id>.toml`. Use `codex-automation prompt render` to preview the
merged prompt before execution.

Bootstrap registers targets in SQLite and writes human-facing target config
under `targets/<id>.toml` in the thin control workspace. Future generated target
skills should be written to app-managed artifacts or explicitly exported into
the control workspace when a human wants to inspect them.

Generated runner prompts should ask workers to report through
`codex-automation result submit`. Result JSON files are useful as import/export
artifacts, but the CLI submission event is the primary state transition. When a
worker sandbox cannot write app-state directly, the worker can return a final
JSON object matching the generated `result.schema.json`; `runner refresh`
ingests it into the same result table.
