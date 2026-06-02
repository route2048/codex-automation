---
name: codex-automation-dev
description: Run maintainer verification for codex-automation before publication or release, including Rust tests, public export audit, clean local install, optional Docker clean install, setup skill installation checks, and route2048 dry-run smoke without pushing, deploying, or starting real runners.
metadata:
  short-description: Verify codex-automation for release
---

# codex-automation-dev

Use this skill when developing, publishing, or validating `codex-automation`
itself. This is the maintainer/developer companion to
`codex-automation-setup`, not the user bootstrap path.

## Workflow

1. Confirm the source checkout and public export checkout.
2. Run standard Rust and export checks:
   `cargo fmt --all -- --check`, `cargo test --workspace`, and
   `python3 scripts/build_public_export.py --output <tmp-export> --overwrite`.
3. Run a clean local install smoke:
   `python3 skills/codex-automation-dev/scripts/verify_clean_install.py --repo <repo> --json`.
4. Verify local skill installation:
   `python3 skills/codex-automation-dev/scripts/verify_skill_install.py --repo <repo> --install-setup-skill --install-dev-skill --overwrite --json`.
5. Run Docker clean install only when the user explicitly wants disposable
   Linux verification and Docker is available:
   `bash skills/codex-automation-dev/scripts/verify_docker_install.sh <repo>`.
6. For route2048, use dry-run only:
   `codex-automation target status route2048 --json`,
   `codex-automation target pack route2048 --json`, and
   `codex-automation heartbeat run route2048 --dry-run --json`.

## Boundaries

- Do not push to GitHub.
- Do not run `--execute` unless the user explicitly asks for a real runner.
- Do not deploy or run Docker Compose.
- Keep all install smoke state under temporary directories by setting
  `CODEX_AUTOMATION_HOME`.
- Target repositories must stay clean; setup must not write
  `.codex-automation` into the target.
- If Docker fails because Docker is unavailable, low on disk, or cannot pull the
  image, report the failure separately from Rust/local install results.

## Success Criteria

- Rust fmt and tests pass.
- Public export audit reports no private path or content hits.
- Clean local install reaches `ready_for_handoff`.
- Docker clean install reaches `ready_for_handoff` when Docker is run.
- Installed setup skill helper scripts run from outside the source checkout.
- route2048 dry-run detects existing state without dispatching duplicate work.
