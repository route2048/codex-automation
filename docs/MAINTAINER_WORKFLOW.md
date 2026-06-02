# Maintainer Workflow

`codex-automation` can be developed from a private working checkout while the
public repository is built from an allowlisted export.

## Public Release Loop

1. Develop the public Rust app under `crates/`, plus `docs/`, `skills/`,
   `scripts/setup.py`, `scripts/build_public_export.py`, and fixture data.
2. Keep runtime state, generated app-state artifacts, reports, archives, and
   worker checkouts out of the public tree.
3. Run the public test and export gates:

```bash
cargo test --workspace
python3 scripts/build_public_export.py --output .public-export/codex-automation --overwrite
python3 scripts/build_public_export.py --output .public-export/codex-automation --check-only
```

4. Publish from `.public-export/codex-automation` or from an orphan public
   branch with only manifest-allowed files.

Do not push a private working checkout or its history directly to a public
GitHub repository.

## Updating a Target

Keep the source checkout separate from the thin Codex App workspace:

```bash
cargo install --path crates/codex-automation-cli --locked
codex-automation workspace init ./codex-automation
codex-automation target add my-app --repo ./target-repo --workspace ./codex-automation
codex-automation target pack my-app --json
codex-automation worker add my-app --from-file ./codex-automation/workers/repo-discovery.toml
codex-automation heartbeat run my-app --json
codex-automation db doctor --json
```

For a new local or cloned target, use the agent-first installer:

```bash
python3 scripts/setup.py <target-path-or-git-url> --workspace ./codex-automation --profile balanced
```

The setup script initializes or reuses the thin control workspace, registers the
target in SQLite, writes `targets/<id>.toml`, loads the default worker,
generates a target pack, and keeps worktrees, logs, and DB state in the OS app
data directory.

Record worker results through the CLI:

```bash
codex-automation result submit <target-id> --workorder-id <id> --status fixed --summary "..." --next-action no_action
```
