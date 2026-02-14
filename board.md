# Board

---

## P1: Dynamic terminal title for coven worker

Add optional `title` field to `AgentFrontmatter` — a Handlebars template rendered with the same args map used for the prompt. Falls back to current format if absent. Keep the branch name in the title.

**Decisions:**
- Use approach A (title template in agent frontmatter)
- Keep the branch name in the title

## P1: Investigate compaction handling

Test whether coven correctly handles auto-compaction during long sessions. Trigger compaction by having haiku repeatedly read files with output each time. Document the observed behavior and any issues found. Do not implement a fix.

## P1: Improve agent prompt conciseness and stale content cleanup

Two prompt improvements:
1. Make the main agent more concise when asking questions (currently too verbose)
2. Have the dispatch agent remove stale content from board issues (old design notes, resolved alternatives, etc.) when syncing
