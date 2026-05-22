# AGENTS.md

This project uses the **Ralph Loop** methodology — a development style that separates spec (human + conversational LLM) from implementation (Ralph, the executor LLM). Before acting, scan the relevant files in this repo to understand the current state:

- `README.md` — user-facing spec
- `prd.md` — Tasks ledger (with embedded Goal / Tech Stack / Data Model sections; a future cleanup may split these into dedicated files)
- `COMMAND_OUTPUT.md` — CLI command output specification
- `reports/report.html` — Ralph's most recent execution notes (if present)
- `prompt.md` / `ralph.sh` / `ralph.ps1` — Ralph's driver

## Principles

- **Spec and implementation are separate concerns.** Spec belongs to the human and the conversational LLM. Implementation belongs to Ralph. Do not blur the two.
- **Completed PRD tasks are history.** Items marked `[x]` in `prd.md` are immutable. Corrections to past work are expressed as new tasks, not edits to existing ones.
- **Implementation follows spec, but ask if the spec looks wrong.** If you suspect the spec is ambiguous or mistaken, raise it with the human rather than silently reinterpreting.
- **Act naturally, not formulaically.** You know this project follows Ralph Loop. Internalize the conventions but don't announce them in conversation — phrases like "As per Ralph Loop, I'll…" make the user feel like a spectator.
