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

## Meaningful package ordering in the agent-maintained repository README

*(noted 2026-06-06)*

The "Maintaining the repository README" section of `templates/AGENTS.md.tmpl` has the agent keep a Packages table in `<data_dir>/README.md` in sync with `homeos.yml`, but says nothing about row order — in practice the agent mirrors `homeos.yml`'s alphabetical (BTreeMap) order, which reads arbitrarily once the package count grows.

Idea: instruct the agent to order (or group) the Packages table in a human-meaningful way — e.g., thematic groups (editors / shells / dev toolchains / GUI apps), dependencies adjacent to their dependents, or daily-driver tools first.

Open questions:

- Stability: ordering must not churn between sessions or agents, or every mutation commit drags reordering noise into the diff. The instruction needs a deterministic tiebreaker (e.g., alphabetical within a group).
- Where the grouping knowledge lives: pure agent judgment from the Purpose column, or explicit metadata in `homeos.yml` (e.g., a `tags`/`category` field — a larger change that would also benefit `package list`).
- Presentation: one table with a meaningful sort, or subsection headings per group?
- Implementation surface: likely a prose-only change to `AGENTS.md.tmpl`; note the maintainer has previously kept template prose edits out of the Ralph loop for voice consistency.

## Repository README Purpose column describes only the package itself

*(noted 2026-06-08)*

The "Filling in the tables" guidance in `templates/AGENTS.md.tmpl` defines the Packages table *Purpose* cell as "one short sentence — what the package does or why the user wants it." In practice the agent sometimes bleeds relationship facts into that cell — when a package has `depends_on`, script aliases, or dependents, it writes things like "required by X" or "installed before Y" into Purpose, which both duplicates the dedicated Dependencies column and shifts the cell from describing *the package* to describing *its role in the graph*.

Idea: tighten the Purpose-column instruction so it describes only the package's own identity and function (what the tool is, why the user keeps it), explicitly excluding relationship facts — dependencies, dependents, install order, aliases — which already live in their own column or are implied by the table structure.

Implementation surface: prose-only change to `AGENTS.md.tmpl`; the maintainer keeps template prose edits out of the Ralph loop for voice consistency.

## Reconcile a stale repository README against current AGENTS.md guidance

*(noted 2026-06-08)*

The agent maintains `<data_dir>/README.md` per the "Maintaining the repository README" rules in `templates/AGENTS.md.tmpl`, but those rules evolve (a new column, an ordering convention, the Purpose-scope tightening above) and a README created under older guidance — or one the user hand-drifted — silently stays out of conformance. Nothing currently prompts a re-alignment, so the README quietly diverges from what the current AGENTS.md says it should be.

Idea: instruct the agent that, when it next touches the README (or notices the mismatch at session start in the data dir), it should compare the existing file against the current guidance and, where it diverges, **propose** the correction to the user rather than silently restructuring. This is the umbrella mechanism for the specific README rules — meaningful ordering and the Purpose-column scope above each become "a thing to reconcile" rather than a one-off instruction.

Open questions:

- How aggressive: auto-fix the agent-owned Packages/Plugins sections (the ownership boundary already says the next update overwrites edits there) vs. propose-and-confirm? A wholesale restructure mid-session is surprising even within agent-owned sections, so leaning toward propose-first.
- Distinguish "stale vs. current conventions" (safe to realign) from intentional user content — the **Notes** section is the user's and must never be touched.
- Could be as light as a sentence folded into the ordering idea, or a standalone "keep the README current" instruction; decide when promoting.

## Background install execution with periodic progress reports

*(noted 2026-06-07)*

Large installs (GUI apps, toolchains, anything that downloads hundreds of MB) can run for many minutes with the agent silently blocked on the subprocess. From the user's side the session looks frozen — they can't tell a 10-minute download from a hung prompt (the no-tty failure mode `AGENTS.md.tmpl` already warns about).

Idea: instruct the agent in `templates/AGENTS.md.tmpl` to run **every** install in the background rather than blocking on it, polling until completion, and to report interim progress to the user roughly every minute while one is still running. Making backgrounding unconditional is deliberate: it removes any "is this install large?" judgment call (LLM agents follow uniform rules more reliably than conditional ones), small installs simply finish by the first poll, and the periodic-report behavior emerges naturally for long-running ones. Both behaviors are phrased as "if your runtime supports it", since capabilities vary by agent: some can run subprocesses in the background and poll them; others execute commands strictly synchronously and can do neither. Agents that can't comply fall back to synchronous execution and instead set expectations up front for known-heavy packages ("this download is large; expect several minutes of silence").

Open questions:

- The critical risk is fire-and-forget: with synchronous execution the exit code and stderr land in the agent's context automatically, but a backgrounded install requires the agent to come back and collect them. The instruction needs wording strong enough that the agent never proceeds to verification/commit without confirming completion and exit code — how to phrase that so it survives across agent implementations?
- What does a useful interim report contain when package-manager output isn't streamed back — elapsed time only, or tail of captured output?
- Implementation surface: prose-only change to `AGENTS.md.tmpl`; note the maintainer has previously kept template prose edits out of the Ralph loop for voice consistency.

## Git bootstrap experience on fresh machines

*(noted 2026-06-07)*

Git is a legitimate, permanent prerequisite — it is homeos's storage engine, and the product workflow (the AI agent committing after every mutation, the user pushing to a remote) uses the git CLI regardless of what homeos does internally. Vendoring a Rust git implementation (gitoxide / libgit2) was considered and rejected: it would only move the prerequisite, not remove it, while adding dependency weight and credential/SSH behavior differences against system git.

What CAN improve is how a fresh machine (notably Windows, which ships without git) experiences the gap:

1. **Fail informatively.** `Command::new("git")` spawn failure currently surfaces as the bare `Error: program not found` (the same unfriendly error class the pwsh fallback work eliminated). Catch the `NotFound` spawn error in `src/git.rs` (and any other direct git invocation) and emit a dedicated reason (e.g., `git-not-found`) with install guidance: `winget install --id Git.Git -e` on Windows, the platform package manager elsewhere. Note the self-help route through homeos itself is blocked: `plugin add` needs `git clone`, so the winget plugin cannot be used to install git.
2. **Bootstrap assist in `install.ps1`.** The installer already does version checks and completion setup; add git detection and offer (or run after confirmation) `winget install --id Git.Git -e` — winget is preinstalled on Windows 11 / modern Windows 10, making it the one package manager reliably present on a fresh machine. `install.sh` needs at most a hint: macOS bootstraps git via xcode-select on first use, and Linux conventions vary too much to act on.

Together these shrink the fresh-Windows path to: `irm ... | iex` → (git installed on the spot if missing) → `homeos init`. The README Prerequisites section (Git 2.28+) stays as-is; only the way it gets satisfied becomes automated.
