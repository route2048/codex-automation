# Publication Boundary

`codex-automation` can be developed from a private working checkout while a
sanitized public tree is published as the OSS repository.

Public copy must state that this is an unofficial project and is not affiliated
with, endorsed by, or sponsored by OpenAI.

## Public

- `crates/`
- `docs/`
- `skills/`
- `scripts/setup.py`
- `scripts/build_public_export.py`
- `tests/fixtures/`
- `README.md`
- `LICENSE`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `Cargo.toml`
- `Cargo.lock`
- `MANIFEST.public.json`
- `.gitignore`

## Local-Only

- `reports/`
- `.public-export/`
- `target/`
- generated app-state data outside the repository
- real provider, account, or local machine identifiers

## Export

Build and inspect a public tree:

```bash
python3 scripts/build_public_export.py --output .public-export/codex-automation --overwrite
python3 scripts/build_public_export.py --output .public-export/codex-automation --check-only
```

The export is intentionally allowlist-based. Do not publish directly from the
private working checkout unless the private paths above have been removed from
the branch and history.

The export builder also scans the generated tree for configured private content
patterns from `MANIFEST.public.json`. Add new exact private footprints to that
manifest before release if a maintainer pack introduces another provider,
domain, mirror remote, or local path that must never appear in public artifacts.

For ongoing development, use `docs/MAINTAINER_WORKFLOW.md`: keep the source
tree Rust-first and publish only the audited export or an equivalent clean
branch.
