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

## Release Procedure

1. Branch `release/v<X.Y.Z>` from main. Day-to-day work happens on main and is pushed continuously (each push runs the Build workflow); the release branch exists only to carry the version bump through a PR. The `release/` prefix keeps the branch name distinct from the `v<X.Y.Z>` tag.
2. Bump `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows, commit `Bump version to <X.Y.Z>`, and push the branch.
3. Open a PR titled `v<X.Y.Z>` with a Summary (user-facing changes since the last tag).
4. The Build workflow runs on the PR automatically (`pull_request` trigger) against the merge preview. When it is green, merge with a merge commit (`gh pr merge --merge`).
5. Merging updates main on origin, which runs Build once more — this time on the merge commit itself. Wait for green.
6. Sync local main (`git checkout main && git pull`), tag the merge commit `v<X.Y.Z>`, and push the tag (`git push origin v<X.Y.Z>`).
7. The tag push runs the Release workflow (six targets, CRT check, release creation). Wait for green, confirm all six assets, then rewrite the release notes: `## Highlights` (user-facing prose), `## What's Changed` (PR link), and the `**Full Changelog**` compare link.
8. Delete the release branch locally and on origin.

gh CLI note (Windows/PowerShell): pass multi-line PR bodies and release notes via files (`--body-file` / `--notes-file`) — inline strings lose newlines or collide with flag parsing.
