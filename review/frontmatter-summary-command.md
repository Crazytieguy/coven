---
priority: P1
state: review
---

# Design a simple bash command to summarize issue frontmatters

The dispatch agent (and humans) need a quick way to see the state and priority of all issues across `issues/` and `review/`. Currently you have to read each file individually.

Design a simple bash command that outputs the filename, state, and priority for every issue. It should be easy to parse at a glance and work across both directories.

## Plan

The command:

```bash
head -7 issues/*.md review/*.md 2>/dev/null
```

**Why this works:**
- `head -7` on multiple files automatically prints `==> filename <==` headers, giving the filename for free
- 7 lines captures the full frontmatter (`---`, priority, state, `---`, blank line, `# Title`), so you see priority, state, and the issue title at a glance
- `2>/dev/null` suppresses errors when one directory is empty (the unexpanded glob becomes a literal path that fails to open)
- Uses only allowed bash patterns — no string interpolation, loops, or advanced xargs

**Example output:**
```
==> issues/has-flag-misses-equals-syntax.md <==
---
priority: P2
state: new
---

# `has_flag` doesn't detect `--flag=value` syntax

==> review/fork-empty-session-id.md <==
---
priority: P1
state: review
---

# Fork uses empty string when session_id is None
```

**Changes:**

1. **Update the dispatch agent template** (`.coven/agents/dispatch.md`): Replace the instruction "List the `issues/` and `review/` directories. Read each file's YAML frontmatter to check its `state` and `priority` fields." with an instruction to run `head -7 issues/*.md review/*.md 2>/dev/null` as the first step. This gives the dispatch agent all the information it needs in one tool call instead of listing directories and then reading each file individually.

2. **Document in `CLAUDE.md`**: Add to the Bash Operations section (under **Patterns:**) a line like: `Issue summary: head -7 issues/*.md review/*.md 2>/dev/null`
