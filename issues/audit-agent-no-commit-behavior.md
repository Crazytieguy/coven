---
priority: P1
state: new
---

# Audit agent creates low-value issues instead of finding nothing

The audit agent currently always proposes issues. If none of its findings are genuine improvements, it creates low-value issues anyway, which can lead to an infinite audit loop.

## Changes requested

Two changes — one to the prompt, one to the worker's handling of no-commit agents.

### 1. Update audit agent prompt

Keep the planned prompt addition telling audit it's okay to find nothing and not commit. Don't manufacture low-value issues just to have output.

### 2. Configurable no-commit behavior via agent frontmatter

Don't encode agent-specific logic in the CLI. Instead, add a frontmatter field to agent configs (e.g. `on_no_commits: sleep`) that tells the worker what to do when an agent finishes without committing.

After an agent produces no commits, the worker should check the field and:

- **`sleep`** (for audit): Check the working directory state:
  - **Clean** → agent genuinely found nothing → sleep until new commits
  - **Dirty** → agent did work but didn't commit → send a follow-up telling it to commit
- **Default / unset** (for other agents): Current behavior — other agents should always commit

This keeps agent-specific policy in agent config rather than hardcoded in the worker loop.
