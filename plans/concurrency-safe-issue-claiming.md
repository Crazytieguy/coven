Issue: It would be nice if issues could be "claimed" in a concurrency-safe way, so that multiple agents can work with the same issue list
Status: draft

## Approach

Use explicit numeric IDs on issues in `issues.md` to make identification deterministic. Then use file-based claiming keyed by those IDs.

### Issue IDs

Each issue in `issues.md` gets a short numeric ID prefix:

```
- #1 When there are queued messages, we should display them somehow...
- #2 I think token count is over-counting...
```

Workflow rules:
- New issues get the next available number (max existing ID + 1).
- When an issue is resolved and removed, its number is retired (not reused).
- The ID is the source of truth for identity — two agents always agree on which issue is `#3`.

### Claiming mechanism

1. Add a `claims/` directory (gitignored — claims are ephemeral local state).
2. Before working on an issue, create a claim file `claims/<id>.<type>` where `<type>` is `plan` or `impl`. Use `set -C` (noclobber) + redirect for atomic creation:
   ```
   set -C && echo "claimed" > claims/3.plan
   ```
   If the file exists, the shell fails atomically — pick a different issue.
3. Planning and implementation are separate claims (`claims/3.plan` vs `claims/3.impl`), so one agent can plan while another implements a different issue.
4. When done (plan committed, or issue resolved), delete the claim file.

### Changes

- **`.gitignore`**: Add `claims/` entry.
- **`workflow.md`**: Add a "Claiming issues" section documenting the ID and claiming protocol. Update issue format instructions.
- **`issues.md`**: Add numeric IDs to existing issues.
- **No code changes** — purely workflow/convention.

## Questions

### Should IDs be strictly sequential?

Strictly sequential (no gaps) is simplest for "next available = max + 1". Gaps from removed issues are fine — we don't reuse numbers, so `#1, #3, #5` is valid after `#2` and `#4` are resolved.

Answer:

## Review

