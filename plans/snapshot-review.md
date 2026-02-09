Issue: [P2] review all snapshots for correctness and UI quality
Status: draft

## Approach

Review all 13 test snapshots for correctness and UI quality, fix any issues found.

### Snapshot inventory

| Snapshot | What it tests | Status |
|---|---|---|
| simple_qa | Basic Q&A, no tools | OK |
| tool_use | Single Bash tool call | OK |
| multi_tool | Write + thinking blocks | OK |
| multi_turn | Follow-up message (after-result) | See issue 1 |
| error_handling | Tool error display | OK |
| show_thinking | Extended thinking text | OK |
| grep_glob | Grep tool detail format | OK |
| mcp_tool | MCP plugin tool name decoding | OK |
| steering | Mid-stream steering message | See issue 2 |
| ralph_break | Ralph mode loop + break | See issue 3 |
| subagent | Task tool with indented child tools | OK |
| write_single_line | Write tool line count | OK |
| edit_tool | Edit tool diff indicator | OK |

### Issues found

#### 1. multi_turn: no follow-up indicator visible

The second turn in `multi_turn.snap` appears after a `---` separator but there's no indication that it was triggered by a follow-up message. The user sees the response but not what prompted it. Compare with the `steering` snapshot where the steering message is similarly invisible.

This overlaps with the existing P1 follow-up display cleanup issue — once that's implemented, this snapshot should be re-recorded and re-reviewed.

**Action:** No change now. Will be addressed by the follow-up display cleanup issue.

#### 2. steering: no steering message shown

The steering message that was injected after tool 1 is not shown to the user. They see the `---` separator and then Claude's changed behavior, but not the message that caused it. This may be intentional (the user typed it so they know what it said) or a display gap.

**Action:** Verify whether this is intentional. If the user typed a steering message, they already know what it says, so the `---` separator alone might be sufficient. No change unless the user requests it.

#### 3. ralph_break: `<break>` tag shown raw

In `ralph_break.snap`, the break tag appears as literal `<break>All tasks in TODO.md are completed.</break>` in the output. This is Claude's output text — it's supposed to contain the break tag for coven to detect and stop the loop. But displaying the raw XML tag to the user is ugly.

**Action:** Strip or reformat the break tag in the display. Options:
- Strip the `<break>...</break>` tags and show the inner text with a visual indicator (e.g., dimmed "Loop ended: All tasks in TODO.md are completed.")
- Strip the entire break line from display (coven already knows to stop)
- Leave as-is if the raw tag is considered acceptable

#### 4. Tool counter continuity across ralph sessions

In `ralph_break.snap`, the tool counter does NOT reset between sessions — it shows [1]-[4] in the first session, then [5]-[9] in the second. But wait, looking more carefully, the counter DOES show [1]-[4] then [5]-[9]. Actually let me re-check — it might be resetting since each session is independent.

**Action:** Verify the actual counter behavior. If the counter resets per session, that's correct (each session is independent). If it doesn't reset, it should — ralph sessions are separate Claude invocations.

### Summary of proposed changes

1. **Fix break tag display** (issue 3) — strip `<break>` tags from displayed text, show a clean "Loop ended" message instead.
2. **Verify tool counter reset** (issue 4) — confirm behavior, fix if wrong.
3. Issues 1 and 2 are covered by the existing follow-up display cleanup plan.

After fixing, re-record affected VCR fixtures and update snapshots.

## Questions

### Should the `<break>` tag content be shown at all?

When Claude outputs `<break>reason</break>`, coven stops the loop. Options for display:
- **Option A:** Show "Loop ended: reason" in dim text (keeps the reason visible)
- **Option B:** Strip entirely — the "Done" line after it is sufficient
- **Option C:** Leave as-is (raw XML tags visible)

I'd lean toward Option A — the reason is useful context.

Answer:

## Review

