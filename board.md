# Board

---

## P1: Dynamic terminal title for coven worker

Add optional `title` field to `AgentFrontmatter` — a Handlebars template rendered with the same args map used for the prompt. Falls back to current format if absent. Keep the branch name in the title.

**Decisions:**
- Use approach A (title template in agent frontmatter)
- Keep the branch name in the title

## P1: Investigate compaction handling

Test whether coven correctly handles auto-compaction during long sessions. Trigger compaction by having haiku repeatedly read files with output each time. Document the observed behavior and any issues found. Do not implement a fix.
