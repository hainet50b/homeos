Read the PRD at prd.md and progress.md, then follow these instructions:

1. Read all unchecked tasks (- [ ]) in the PRD and review `progress.md` to understand what has been completed and any remarks from previous work.
2. Select the next task to work on, considering dependencies between tasks and current project state. Process Tasks before Post Tasks.
3. Implement ONLY that one task.
4. Run tests to verify all existing tests still pass.
5. If tests pass, mark the task as checked (- [x]) in the PRD.
6. Append a progress entry to `progress.md` using the following format:

```
## Task: <task name>

**Timestamp:**

<current UTC time in ISO 8601 format>

**Why this task:**

<brief reason for choosing this task — e.g., dependency order, prerequisite for other tasks, only remaining task>

**What was done:**

<summary of implementation — use multiple lines if needed>

**What was changed:**

<list of files added or modified — one per line>

**Remarks:**

<any issues encountered, workarounds applied, or lessons learned — write as much as needed>
```

7. Stage all changes and create a git commit with a descriptive message.
8. If ALL tasks in both Tasks and Post Tasks are now checked, include the exact text `<promise>COMPLETE</promise>` in your response.

IMPORTANT:
- Work on only ONE task, then stop.
- Do not proceed to the next task.
- Do not skip failing tests. Fix the code until tests pass before marking complete.
