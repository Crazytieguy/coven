Issue: `coven init` should ask the user if they'd like to update CLAUDE.md to reference the workflow.
Status: draft

## Approach

Currently `coven init` prints a passive tip when `workflow.md` is created:

```
Tip: Add this to your CLAUDE.md so interactive sessions understand the workflow:
  See .coven/workflow.md for the issue-based development workflow.
```

Change this to an interactive prompt that offers to append the reference automatically.

### Implementation

In `src/commands/init.rs`:

1. After the created/skipped summary, if `workflow.md` was created (or exists), check whether `CLAUDE.md` already contains a reference to `.coven/workflow.md`.

2. If no reference exists, prompt the user with something like:
   ```
   Add a reference to .coven/workflow.md in CLAUDE.md? [Y/n]
   ```

3. On confirmation (or default yes):
   - If `CLAUDE.md` doesn't exist, create it with just the reference line.
   - If `CLAUDE.md` exists, append the reference line (with a blank line separator).
   - The appended text: `See .coven/workflow.md for the issue-based development workflow.\n`
   - Report what was done.

4. On decline, print the current tip as a fallback so the user can do it manually.

### Details

- Use `std::io::stdin().read_line()` for the prompt (simple y/n, no extra dependencies).
- Check for existing reference by reading `CLAUDE.md` and searching for `.coven/workflow.md` substring.
- Always prompt — even if `workflow.md` already existed (was skipped) — because the user might have run `init` previously without adding the CLAUDE.md reference.

## Questions

### Should this also prompt when workflow.md was skipped (already existed)?

The CLAUDE.md reference is useful regardless of whether this specific `init` call created `workflow.md`. If CLAUDE.md lacks the reference, prompting makes sense either way.

Proposed: always prompt if CLAUDE.md lacks the reference, regardless of whether `workflow.md` was just created or already existed.

Answer:

## Review

