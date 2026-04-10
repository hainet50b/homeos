Read the PRD at prd.md, README.md (as the specification), and COMMAND_OUTPUT.md (as the output specification), then follow these instructions:

1. Read all unchecked tasks (- [ ]) in the PRD and review the last 100 lines of `progress.md` (not the entire file) to understand recent remarks from previous work.
2. Select the next task to work on, considering dependencies between tasks and current project state. Process Tasks before Post Tasks.
3. Implement ONLY that one task.
4. Verify that functions, methods, and CLI subcommands across the affected files are ordered consistently with `README.md`. Fix any ordering inconsistencies, not just in code you added.
5. Write corresponding unit tests following the 3A pattern (Arrange / Act / Assert). Fixtures must only handle Arrange (preconditions). Act must explicitly call the method or function under test — never hide it inside a fixture.
6. Run `cargo fmt`.
7. Run `cargo clippy` and fix any warnings.
8. Run `cargo test` to verify all existing tests still pass.
9. If fmt, clippy, and tests pass, mark the task as checked (- [x]) in the PRD.
10. Append a progress entry to `progress.md` using the following format:

```
## Task: <task name>

**Timestamp:**

<run `date -u +%Y-%m-%dT%H:%M:%SZ` and paste the output here>

**Why this task:**

<brief reason for choosing this task — e.g., dependency order, prerequisite for other tasks, only remaining task>

**What was done:**

<summary of implementation — use multiple lines if needed>

**What was changed:**

<list of files added or modified — one per line>

**Remarks:**

<any issues encountered, workarounds applied, or lessons learned — write as much as needed>
```

11. Stage all changes and create a git commit with a descriptive message.
12. If ALL tasks in both Tasks and Post Tasks are now checked, include the exact text `<promise>COMPLETE</promise>` in your response.

IMPORTANT:
- Work on only ONE task, then stop.
- Do not proceed to the next task.
- Do not skip failing tests. Fix the code until tests pass before marking complete.
