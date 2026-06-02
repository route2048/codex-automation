# Contributing

Thanks for helping improve `codex-automation`.

## Development Loop

1. Run `cargo fmt --all -- --check` before committing.
2. Run `cargo test --workspace` before committing behavior changes.
3. Add or update focused CLI smoke tests for control-plane state changes.
4. Do not add model overrides to Codex CLI invocations.
5. Do not make push, deploy, destructive, auth, payment, secret, dependency, or
   database-schema behavior automatic without an approval package.
6. Keep target repositories clean during setup; runtime state belongs in the OS
   app data directory and the thin control workspace contains human-facing
   config only.

## Useful Checks

```bash
cargo fmt --all -- --check
cargo test --workspace
python3 scripts/build_public_export.py --output .public-export/codex-automation --check-only
```

For agent-first bootstrap smoke tests, use a fixture target and temporary app
state:

```bash
python3 skills/codex-automation-dev/scripts/verify_clean_install.py --repo . --json
python3 skills/codex-automation-dev/scripts/verify_skill_install.py --repo . --install-setup-skill --install-dev-skill --overwrite --json
```

When Docker verification is explicitly wanted:

```bash
bash skills/codex-automation-dev/scripts/verify_docker_install.sh .
```
