# IDEAS.md

Parking lot for implementation ideas and future directions that are not yet PRD tasks. Entries are non-binding sketches owned by the human and the conversational LLM. Ralph never implements directly from this file — when an idea matures, it is promoted to a Tasks entry in `PRD.md` and removed from here.

## Package drift detection against plugin templates

*(noted 2026-06-06)*

Packages created with `--plugin` render the plugin's templates (`<action>.<ext>.tmpl` with `params` substituted) into concrete action scripts at `package add` time. After that, the rendered scripts can drift from the plugin in two ways:

- `plugin refresh` updates the plugin's templates, but existing packages keep the scripts rendered from the old templates
- the user hand-edits a rendered script, diverging from what the template would produce

Idea: a check that re-renders the current templates with the package's recorded `params` and diffs the result against the on-disk scripts, reporting in-sync / drifted per action script. Possible surfaces: a dedicated command (`homeos package drift [<package>]`), a flag on an existing command, or integration into `plugin refresh` (after refreshing templates, list packages whose scripts now drift).

Open questions:

- Hand edits are legitimate (rendered scripts are meant to be editable), so drift is information, not an error — what should the exit code / JSON shape communicate?
- Should `plugin refresh` offer to re-render drifted packages' scripts, and how does that interact with preserving hand edits?
- Scope: only packages with a `plugin:` entry; packages without a plugin have nothing to drift from.

## Git bootstrap experience on fresh machines

*(noted 2026-06-07)*

Git is a legitimate, permanent prerequisite — it is homeos's storage engine, and the product workflow (the AI agent committing after every mutation, the user pushing to a remote) uses the git CLI regardless of what homeos does internally. Vendoring a Rust git implementation (gitoxide / libgit2) was considered and rejected: it would only move the prerequisite, not remove it, while adding dependency weight and credential/SSH behavior differences against system git.

What CAN improve is how a fresh machine (notably Windows, which ships without git) experiences the gap:

1. **Fail informatively.** `Command::new("git")` spawn failure currently surfaces as the bare `Error: program not found` (the same unfriendly error class the pwsh fallback work eliminated). Catch the `NotFound` spawn error in `src/git.rs` (and any other direct git invocation) and emit a dedicated reason (e.g., `git-not-found`) with install guidance: `winget install --id Git.Git -e` on Windows, the platform package manager elsewhere. Note the self-help route through homeos itself is blocked: `plugin add` needs `git clone`, so the winget plugin cannot be used to install git.
2. **Bootstrap assist in `install.ps1`.** The installer already does version checks and completion setup; add git detection and offer (or run after confirmation) `winget install --id Git.Git -e` — winget is preinstalled on Windows 11 / modern Windows 10, making it the one package manager reliably present on a fresh machine. `install.sh` needs at most a hint: macOS bootstraps git via xcode-select on first use, and Linux conventions vary too much to act on.

Together these shrink the fresh-Windows path to: `irm ... | iex` → (git installed on the spot if missing) → `homeos init`. The README Prerequisites section (Git 2.28+) stays as-is; only the way it gets satisfied becomes automated.
