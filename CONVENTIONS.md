# homeos — Conventions

How code is written in homeos. Read before implementing.

## Tech Stack

- Rust (latest stable)
- clap / clap_complete (CLI argument parsing and shell completion)
- serde / yaml_serde (configuration parsing)
- ureq (HTTP client for GitHub API)
- dirs (OS-appropriate data directory resolution)

## Test Pattern

Unit tests follow the **3A pattern**:

- **Arrange** — set up the preconditions
- **Act** — explicitly invoke the function or method under test
- **Assert** — verify the outcomes

Fixtures handle Arrange only. The Act call must be visible in the test body, not hidden inside a fixture helper.

## Lint / Format / Test Commands

The commands below must pass before a task is marked complete in `PRD.md`:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## File and Symbol Ordering

Functions, methods, and CLI subcommands across the affected files must be ordered consistently with `README.md`. Fix ordering inconsistencies in pre-existing code when you touch the area, not just in newly added code.

## Commit Messages

Subject line: present-imperative, ≤ 72 characters. Body optional and free-form. The implementation LLM has discretion on wording — no template, no enforced prefixes.
