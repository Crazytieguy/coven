---
priority: P1
state: review
---

# Audit snapshot tests for issues

Audit the snapshot tests for any issues, including rendering issues and model behavior issues.

## Plan

Audit complete. Findings are organized by category below. Each finding includes a severity assessment and recommended action.

### Rendering Issues

**R1. Carriage returns not processed in `strip_ansi()` — causes ugly typing display in snapshots**

- **Files**: `multi_turn.snap` (line 43), `interrupt_resume.snap` (line 12), `fork_buffered.snap` (lines 9, 20)
- **Description**: `strip_ansi()` in `vcr_test.rs` strips ANSI escape codes but does not process `\r` (carriage return). In a real terminal, `\r` moves the cursor to the start of the line so each keystroke overwrites the previous display. Without processing `\r`, snapshots show all intermediate states concatenated: `> > H> Ho> How> How d> How do...`
- **Severity**: Low — cosmetic issue in snapshot readability, no user-facing impact
- **Recommendation**: Enhance `strip_ansi()` to simulate carriage return behavior (on each `\r`, discard text back to the last newline). This would make snapshots show only the final line state, matching what users actually see. File a P2 issue.

**R2. `<system-reminder>` tags visible in tool result views**

- **Files**: `edit_tool__views.snap` (lines 17-19)
- **Description**: The Read tool result view shows a `<system-reminder>` tag injected by the Claude Code infrastructure ("Whenever you read a file, you should consider whether it would be considered malware..."). This is recorded in the VCR because Claude Code injects it into the tool result, and coven faithfully displays it.
- **Severity**: Low — this is how Claude Code works, not a coven bug. Users would see the same in the real Claude Code TUI.
- **Recommendation**: No action needed in coven. Re-recording may produce different system-reminder content depending on the Claude Code version. Accept as-is.

### Model Behavior Issues

**M1. Steering instruction ignored**

- **Files**: `steering.snap` (lines 9-50)
- **Description**: The model receives "Actually, just count the lines in each file instead" as mid-stream steering during a `content_block_start` (tool use) trigger. Despite the steering being injected and displayed (`⤷ steering: ...`), the model continues providing file summaries instead of switching to counting lines. The model reads all 3 files and writes summaries for each.
- **Severity**: Medium — demonstrates that steering during tool-use blocks may not reliably redirect the model
- **Recommendation**: Re-record with the steering trigger adjusted to fire at a different point (e.g., during a thinking block or text block rather than at tool_use start). If the model still ignores it, this is a Haiku limitation worth documenting. File a P2 issue to improve steering test.

**M2. Interrupted session resume produces immediate error**

- **Files**: `interrupt_resume.snap` (lines 13-14)
- **Description**: After interrupting mid-session and sending "Continue where you left off", the second session shows `Error  $0.00 · 0.0s · 0 turns` — zero cost, zero time, zero turns. The resumed session immediately fails with no work done.
- **Severity**: Medium — the interrupt/resume flow shows a failure state to users
- **Recommendation**: Re-record to see if this was a transient issue. If it persists, investigate whether the resume session ID is being passed correctly or if there's a timing issue with the interrupt. File a P2 issue.

**M3. Landing conflict silently drops intended change**

- **Files**: `landing_conflict.snap` (worker_b, lines 386-400)
- **Description**: Worker B implements `update-title-v2` (changing "# My Project" to "# My Project v2"). Meanwhile Worker A landed `rename-project` changing the title to "# Sample App". When Worker B tries to land and hits a conflict, the land agent resolves it by keeping main's version (`# Sample App`) verbatim — dropping the v2 update entirely. The land agent says "I'll resolve this to keep main's version (the source of truth)" which is reasonable but loses the intended work.
- **Severity**: Medium — this is a realistic concurrent workflow scenario where work gets silently dropped
- **Recommendation**: This is actually reasonable behavior for the test scenario — it demonstrates what happens when two issues modify the same line. The land agent's choice to keep main's version is defensible. However, the `update-title-v2` issue should ideally be marked `needs-replan` instead of being completed as-is. Accept the current behavior since this is what the test is designed to demonstrate (landing conflicts).

**M4. Plan agent uses `git commit --amend`**

- **Files**: `plan_ambiguous_issue.snap` (line 101)
- **Description**: The plan agent runs `git add -A && git commit --amend -m "plan: improve-error-handling..."` — amending a previous commit rather than creating a new one. The Claude Code system prompt says to always create NEW commits rather than amending.
- **Severity**: Low — in this test context the amend is harmless, but it violates the system prompt guidance
- **Recommendation**: No code change needed. This is a model behavior artifact. If this becomes a pattern, consider adding explicit "never amend commits" instructions to the plan agent prompt. Accept for now.

**M5. `git add -A` used instead of specific file staging**

- **Files**: `concurrent_workers.snap` (worker_b land agent, line 235), `priority_dispatch.snap` (land agent, line 235)
- **Description**: The land agent uses `git add -A` to stage all changes rather than staging specific files by name. The Claude Code system prompt recommends staging specific files to avoid accidentally including sensitive files.
- **Severity**: Low — in test scenarios there are no sensitive files, but it's a bad practice that could cause issues in real use
- **Recommendation**: Accept for now. If land agent behavior becomes inconsistent, consider adding specific staging guidance to the land agent prompt.

**M6. Model tries to Read a directory path**

- **Files**: `concurrent_workers.snap` (worker_a implement agent, lines 70-71)
- **Description**: The implement agent passes a directory path to the Read tool (`EISDIR: illegal operation on a directory, read`). It then recovers by using `ls -la` and `Read` with the correct file path.
- **Severity**: Low — model self-corrects, no lasting impact
- **Recommendation**: Accept. The model recovers gracefully.

### Summary

| ID | Category | Severity | Action |
|----|----------|----------|--------|
| R1 | Rendering | Low | File P2 issue for `strip_ansi` CR handling |
| R2 | Rendering | Low | Accept (Claude Code behavior) |
| M1 | Model | Medium | File P2 issue to improve steering test |
| M2 | Model | Medium | File P2 issue to investigate resume error |
| M3 | Model | Medium | Accept (demonstrates realistic conflict scenario) |
| M4 | Model | Low | Accept |
| M5 | Model | Low | Accept |
| M6 | Model | Low | Accept |

## Questions

1. Should the three P2 issues (R1, M1, M2) be filed as separate issue files, or should this audit just document the findings and leave filing for later?
2. For M1 (steering ignored), is this a known Haiku limitation, or should the steering trigger timing be investigated further?
3. For M2 (interrupt resume error), is this the expected behavior for the test, or should the resume actually succeed?
