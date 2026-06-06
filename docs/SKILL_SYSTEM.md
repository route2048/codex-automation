# Skill System

`codex-automation` treats skills as reusable worker operating manuals.

The setup entrypoint is `skills/codex-automation-setup/`. It is the only skill
normal users install. It bootstraps this tool and hands ongoing work to the CLI
control surface.

The maintainer verification entrypoint is `skills/codex-automation-dev/`. It
runs release-readiness checks for this project itself. It is public so
contributors can reproduce verification, but it is not part of end-user setup.

The release operator skill is private maintainer infrastructure. It is not
included in this repository or installed by users because it can push public
branches, create tags, and verify GitHub Releases.

## Implemented

- `codex-automation-setup`: install, update, doctor, initialize a thin control
  workspace, register target repos, and explain result submission.
- `codex-automation-dev`: verify Rust tests, clean local install, optional
  Docker install, release-packaged setup skill installation, and fixture
  dry-run smoke before publication.
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
`codex-automation result submit` when available. Result JSON is also a first
class handoff path: a worker can return a final JSON object with `workorder_id`,
`status`, `summary`, and `next_action`; the controller can save it to the runner
package `result.json`, and `runner refresh` ingests it into the same result
table.
