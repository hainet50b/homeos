# AGENTS.md

This project uses the **Ralph Loop** methodology — a development style that separates spec (human + conversational LLM) from implementation (Ralph, the executor LLM). Before acting, scan the relevant files in this repo to understand the current state:

- `README.md` — user-facing spec
- `SPEC.md` — developer-facing internal spec (data model, architecture)
- `PRD.md` — What / Why + open Tasks
- `CONVENTIONS.md` — how code is written here (Tech Stack, test pattern, lint/format/test commands, ordering, commit style)
- `COMMAND_OUTPUT.md` — CLI command output specification
- `reports/report.html` — Ralph's most recent execution notes (if present)
- `prompt.md` / `ralph.sh` / `ralph.ps1` — Ralph's driver

## Principles

- **Spec and implementation are separate concerns.** Spec belongs to the human and the conversational LLM. Implementation belongs to Ralph. Do not blur the two.
- **Completed PRD tasks are history.** Items marked `[x]` in `PRD.md` are immutable. Corrections to past work are expressed as new tasks, not edits to existing ones.
- **Implementation follows spec, but ask if the spec looks wrong.** If you suspect the spec is ambiguous or mistaken, raise it with the human rather than silently reinterpreting.
- **Act naturally, not formulaically.** You know this project follows Ralph Loop. Internalize the conventions but don't announce them in conversation — phrases like "As per Ralph Loop, I'll…" make the user feel like a spectator.

## Spec-layer maintenance

You should propose updates to the spec-layer files proactively — keeping them fresh is your beat, not Ralph's:

- `PRD.md`'s What / Why drift as goals sharpen in conversation.
- `SPEC.md` drifts most easily because no end user complains about an outdated internal spec. Whenever a new data model, module boundary, or invariant surfaces (or an existing one is implicitly changed), bring it up and propose the `SPEC.md` edit in the same conversation.
- `README.md` whenever the conversation introduces or reframes user-visible behaviour.
- `COMMAND_OUTPUT.md` whenever a command's canonical output changes.

PRD's Tasks section is managed jointly with the human. The spec-layer updates above are owned by the human and you; Ralph is never asked to update them, which preserves the spec/implementation separation principle above.
