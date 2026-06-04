# Control Plane Protocol

The control plane is coordinated by the CLI and SQLite database.

## State Owners

- Thin control workspace: human-facing config and reports.
- SQLite app database: workspaces, targets, workers, workorders, results,
  approvals, loop runs, runner package records, and events.
- OS app data: worktrees, logs, artifacts, and backups.
- Target repo: product source only.

Do not place queue state, worker registries, runner logs, or worktrees in the
target repo during normal operation.

## Workspace

Create the control workspace with:

```bash
codex-automation workspace init <control-workspace>
```

This writes `README.md`, `AGENTS.md`, `codex-automation.toml`, `workers/`,
`targets/`, and `reports/`. It also records the workspace in SQLite.

## Target

Register a target with:

```bash
codex-automation target add <id> --repo <repo-path> --workspace <control-workspace>
codex-automation target pack <id> --json
```

The CLI writes `targets/<id>.toml` for humans and records the target row in
SQLite. `target pack` scans the repository and writes generated context under
OS app data `artifacts/targets/<id>/`. The target repo is not modified.

## Result

Worker result submission is the primary completion signal:

```bash
codex-automation result submit <target-id> \
  --workorder-id <workorder-id> \
  --status fixed \
  --summary "..." \
  --next-action no_action
```

The CLI validates target existence, required fields, and known result statuses,
then records a single SQLite transaction:

- upsert the workorder summary state
- insert the result submission
- append an event

JSON result files are import/export artifacts:

```bash
codex-automation result submit <target-id> --from-file result.json
```

## Current Result Statuses

- `approval_required`
- `blocked`
- `discovery_no_findings`
- `discovery_findings`
- `safe_fix_candidate`
- `tests_passed`
- `tests_failed`
- `fixed`
- `failed`
- `needs_more_investigation`
- `runner_lost_before_result`
- `staging_deploy_blocked`
- `staging_deployed`
- `stale_or_invalid`

Statuses that require human judgment map to `needs_user`. Successful statuses
map to `completed`. Everything else is retained as a submitted or failed
workorder state until the controller decides the next action.

## Worker

Workers are editable role definitions, not processes. Keep worker TOML in the
thin control workspace:

```text
workers/control-plane.toml
workers/repo-maintainer.toml
workers/ops-analyst.toml
workers/release-manager.toml
```

Load or update a worker with:

```bash
codex-automation worker add <target-id> --from-file workers/repo-maintainer.toml
codex-automation worker list <target-id> --json
codex-automation worker status <target-id> repo-maintainer --json
```

Required fields live under `[worker]`: `id`, `name`, `description`, `skills`,
`allowed_workorder_kinds`, `sandbox`, `approval_policy`, `autonomy_profile`,
and `max_concurrency`. `custom_instructions` and `[config]` are optional
extension points.

`workers/control-plane.toml` is the orchestration actor definition. It lives in
the same directory for readability, but it is not a runnable worker selected by
runner dispatch. Target-specific instructions live under
`targets/<target-id>.toml`.

Runner dispatch with `--worker` validates that the worker allows the selected
workorder kind before creating a runner package.

Preview the merged prompt without creating runner state:

```bash
codex-automation prompt render <target-id> --workorder-id <workorder-id> --worker repo-maintainer --json
```

## Workorder

Create workorders through the CLI:

```bash
codex-automation workorder create <target-id> \
  --id <workorder-id> \
  --kind repo_discovery \
  --title "Inspect target repository" \
  --payload-json '{"scope":"read_only"}'
```

Inspect workorders with:

```bash
codex-automation workorder list <target-id> --json
codex-automation workorder status <target-id> <workorder-id> --json
```

## Loop

`loop run` performs one bounded control-plane step:

```bash
codex-automation loop run <target-id> --json
```

If a queued workorder exists, it is marked `ready_for_worker`. If the target is
idle, the loop creates a read-only `repo_discovery` workorder. Detached worker
execution is not started by this command.

## Heartbeat

`heartbeat run` is the normal orchestration entrypoint:

```bash
codex-automation heartbeat run <target-id> --json
CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 codex-automation heartbeat run <target-id> --execute --json
```

The heartbeat refreshes runner state, regenerates the target pack, advances one
loop step, selects a compatible worker for ready work, and creates a runner
package. With `--execute`, it also starts the runner after checking the
execution gate and worker concurrency.

## Runner

Runner dispatch creates a package for a Codex worker:

```bash
codex-automation runner dispatch <target-id> --workorder-id <workorder-id> --worker <worker-id> --json
CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 \
  codex-automation runner dispatch <target-id> --workorder-id <workorder-id> --worker <worker-id> --execute --json
codex-automation runner refresh <target-id> --json
codex-automation runner list <target-id> --json
codex-automation runner status <target-id> <runner-id> --json
```

The package is stored under OS app data `artifacts/runners/` and contains:

- `prompt.md`: the worker prompt, boundaries, workorder payload, and result
  contract
- `result.schema.json`: the final JSON result schema for `codex exec`
- `command.json`: the machine-readable runner metadata stored in SQLite

`--execute` is gated by `CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1`. When
enabled, the launcher starts `codex exec` with the generated prompt on stdin,
captures stdout/stderr logs, writes the final message to the package directory,
and stores the PID in SQLite. It never passes `--model` or
`model_reasoning_effort`.

Workers should submit results with `codex-automation result submit`. If their
sandbox prevents that, the final response can be the schema-matching JSON
result; `runner refresh` ingests it into the normal result table.

## Approval

Approval packages are recorded in SQLite:

```bash
codex-automation approval request <target-id> --workorder-id <workorder-id> --reason "..."
codex-automation approval list <target-id> --json
codex-automation approval record <target-id> <approval-id> --decision approved --message "..."
```
