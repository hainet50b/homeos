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

1. Branch `v<X.Y.Z>` from main (work accumulates on local main between releases; the release branch carries it to origin).
2. Bump `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows, commit `Bump version to <X.Y.Z>`, and push the branch.
3. Trigger the Build workflow on the branch: `gh workflow run Build --ref v<X.Y.Z>`. Build runs only on push-to-main and manual dispatch, so PRs get no automatic checks — this dispatch is the pre-merge verification. Wait for green.
4. Open a PR titled `v<X.Y.Z>` with a Summary (user-facing changes since the last tag) and a Test plan checklist. Check items off as they verify.
5. Merge with a merge commit (`gh pr merge --merge`). The push to main triggers Build again on the merge commit; wait for green and update the PR checklist.
6. Tag the merge commit `v<X.Y.Z>` and push the tag with an explicit refspec — `git push origin refs/tags/v<X.Y.Z>` — the branch and tag share a name, so a bare name is ambiguous.
7. The tag push runs the Release workflow (six targets, CRT check, release creation). Wait for green, confirm all six assets, then rewrite the release notes: `## Highlights` (user-facing prose), `## What's Changed` (PR link), and the `**Full Changelog**` compare link.
8. Delete the release branch locally and on origin (`git push origin --delete refs/heads/v<X.Y.Z>`; the explicit refspec again disambiguates from the tag).

gh CLI note (Windows/PowerShell): pass multi-line PR bodies and release notes via files (`--body-file` / `--notes-file`) — inline strings lose newlines or collide with flag parsing.
