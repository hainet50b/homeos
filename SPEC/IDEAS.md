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
