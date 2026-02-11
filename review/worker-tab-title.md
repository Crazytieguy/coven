---
priority: P2
state: review
---

# Terminal tab title should reflect currently running agent

The worker currently sets the terminal tab title to the branch name. It should instead reflect the currently running agent (e.g. dispatch, plan, implement, land) so you can see at a glance what each worker is doing.

Also find something shorter than the current "coven: " prefix — it takes up too much space in narrow tab bars.

## Plan

The title is already updated at each lifecycle phase (dispatch, sleeping, agent run) — the main problems are (a) the 7-character `"coven: "` prefix wastes space, and (b) the branch name comes first, pushing the agent name off-screen in narrow tabs.

### Changes in `src/commands/worker.rs`

Shorten the prefix from `"coven: "` to `"cv "` (3 chars), and swap the order so the agent/phase comes first:

| Line | Current | New |
|------|---------|-----|
| 121 | `"coven: {branch}"` | `"cv {branch}"` |
| 193 | `"coven: {branch} — dispatch"` | `"cv dispatch — {branch}"` |
| 350 | `"coven: {branch} — sleeping"` | `"cv sleeping — {branch}"` |
| 384 | `"coven: {branch} — {title_suffix}"` | `"cv {title_suffix} — {branch}"` |

That's 4 `set_title` calls to update, all in one file. No other changes needed.

## Questions

- Is `"cv "` a good prefix, or would you prefer something else? Other options: `"cv:"`, no prefix at all, a unicode glyph.
