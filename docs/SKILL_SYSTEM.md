# Skill System

`codex-automation` treats skills as reusable worker operating manuals.

The setup entrypoint is `skills/codex-automation-setup/`. It bootstraps this
tool and hands ongoing work to the CLI control surface.

## Implemented

- `codex-automation-setup`: install, doctor, initialize a thin control
  workspace, register target repos, and explain result submission.
- `repo-discovery`: default read-only worker definition generated under
  `workers/repo-discovery.toml`.

## Planned Lanes

- `repo-discovery`: inspect a target repo and produce findings.
- `code-maintenance`: perform small approved source, test, and docs fixes.
- `test-runner`: run target-local verification commands.
- `log-analysis`: parse logs and propose root-cause investigations.
- `release-planning`: inspect staging release readiness behind approval gates.
- `deploy`: prepare deploy workorders behind approval gates.

Skills are selected through worker definitions. Runner packages include only
the relevant skill names, worker boundaries, and the current workorder
contract.

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
