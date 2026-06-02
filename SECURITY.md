# Security Policy

`codex-automation` is local-first automation software. Treat generated
workorders, worker prompts, runner logs, and result JSON as potentially
sensitive because they may contain repository paths, code excerpts, or operator
decisions.

## Reporting

Please report security-sensitive issues privately to the repository owner until
a public disclosure process exists.

## Boundaries

- Do not commit target `.codex-automation/` state from private repositories.
- Do not store secrets in workorders, prompts, results, runner logs, or approval
  package messages.
- Do not bypass approval gates for deploy, push, destructive, auth, payment,
  dependency, database-schema, provider-state, or secret-handling changes.
- Do not add model overrides to Codex CLI runner commands; use local Codex
  configuration.
