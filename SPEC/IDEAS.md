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
