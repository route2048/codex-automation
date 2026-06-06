# Autonomy Policy

Autonomy controls how much work the control plane may delegate without asking a
human first.

## Profiles

- `observe`: read-only discovery and reports.
- `suggest`: read-only discovery plus patch proposals.
- `balanced`: small docs, tests, and local bug fixes may be attempted.
- `aggressive`: broader edits and refactors may be attempted within configured limits.
- `release`: release preparation is allowed, but push and deploy still require approval.

The current Rust core records the selected profile on each registered target.
Runner dispatch creates a Codex App handoff package for a worker. It must not
start headless Codex processes.
The target profile is an upper bound, not permission to skip explicit approval
gates.

## Implemented Today

- profile value is stored with the target registration
- worker definitions store sandbox, approval policy, autonomy profile, and
  allowed workorder kinds
- runner dispatch with `--worker` validates worker/workorder kind compatibility
- runner dispatch generates `prompt.md` and `command.json` under app-state
  artifacts
- runner dispatch creates handoff packages only
- runner refresh ingests submitted results or package `result.json` files
- worker results are validated before becoming durable state
- result statuses such as `approval_required` and `safe_fix_candidate` map to
  `needs_user` workorder state
- successful statuses map to `completed`
- failed or blocked statuses map to `failed`

## Non-Negotiable Future Gates

Future loop, runner, approval, update, deploy, and commit commands must require
explicit approval for deploys, pushes, dependency updates, auth, payments,
security-sensitive changes, database schema changes, secret handling, and
destructive commands.
