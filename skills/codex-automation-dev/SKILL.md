---
name: codex-automation-dev
description: Maintainer-only verification for codex-automation before publication or release, including Rust tests, clean local install, optional Docker clean install, release-packaged setup skill installation checks, and fixture dry-run smoke without pushing, deploying, or starting real runners. Do not install this skill in normal end-user environments.
metadata:
  short-description: Verify codex-automation for release
---

# codex-automation-dev

Use this skill when developing, publishing, or validating `codex-automation`
itself. This is the maintainer/developer companion to
`codex-automation-setup`, not the user bootstrap path.

Normal users should only install `codex-automation-setup`. This dev skill may
live in the public source tree so contributors can reproduce release-readiness
checks, but it should not be installed as part of target-repository setup.

## Workflow

1. Confirm the checkout that should be verified.
2. Run standard Rust checks:
   `cargo fmt --all -- --check` and `cargo test --workspace`.
3. Run a clean local install smoke:
   `python3 skills/codex-automation-dev/scripts/verify_clean_install.py --repo <repo> --json`.
4. Verify local skill installation from the source skill directory:
   `python3 skills/codex-automation-dev/scripts/verify_skill_install.py --repo <repo> --install-setup-skill --install-dev-skill --overwrite --json`.
5. When evaluating unpublished setup-skill changes, do not mix the source skill
   with the latest GitHub Release binary. Install or expose the source binary
   first, or set `CODEX_AUTOMATION_BIN=<repo>/target/debug/codex-automation`.
6. Run Docker clean install only when the user explicitly wants disposable
   Linux verification and Docker is available:
   `bash skills/codex-automation-dev/scripts/verify_docker_install.sh <repo>`.
7. Keep target-specific smoke checks out of this public skill. If a maintainer
   needs project-specific validation, record it in a private report or a
   private skill that is not included in the public export.

## Boundaries

- Do not push to GitHub.
- Do not install this skill in normal user environments.
- Do not start headless Codex runners; verification should stop at handoff and
  result ingestion.
- Do not deploy or run Docker Compose.
- Keep all install smoke state under temporary directories by setting
  `CODEX_AUTOMATION_HOME`.
- Target repositories must stay clean; setup must not write
  `.codex-automation` into the target.
- If Docker fails because Docker is unavailable, low on disk, or cannot pull the
  image, report the failure separately from Rust/local install results.

## Success Criteria

- Rust fmt and tests pass.
- Clean local install reaches `ready_for_handoff`.
- Docker clean install reaches `ready_for_handoff` when Docker is run.
- The setup skill installs from the release/source skill directory and helper
  scripts run from outside the source checkout.
- Fixture heartbeat creates a runner package without starting real work.
