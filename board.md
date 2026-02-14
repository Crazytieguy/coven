# Board

## P1: Propose improvements for post-compaction context loss

Model tried `git push` instead of `bash .coven/land.sh` after session compaction. Also confused about interactive/non-interactive transitions. Propose possible improvements.

**Analysis:** Agent prompts (from `main.md`, `dispatch.md`) are sent as the initial user message — subject to compaction. The system prompt (`system.md` + transition protocol, via `--append-system-prompt`) survives compaction. Currently `system.md` has only high-level docs; critical operational rules live only in agent prompts.

**Proposals:**

1. **Move critical rules to system.md** — Add a "Rules" section to `system.md` with the key invariants (use `land.sh` not `git push`, never transition without landing, delete scratch.md on land). Survives compaction by being in the system prompt. Applies to all agents, which is fine since these are universal constraints.

2. **Inject reminders on compaction events** — When coven detects a compaction event in the stream, send a follow-up message with critical operational rules. Depends on the "Display compaction messages" issue to understand the event format. Defense-in-depth on top of proposal 1.

3. **Trim agent prompts** — Make `main.md` and `dispatch.md` more concise so they compress better and leave more room for working context. Move explanatory content to system.md, keep agent prompts to just the task-specific workflow.

**Questions:**
- Which proposals to pursue (all three, or a subset)?
- Any other failure modes observed beyond `git push` and transition confusion?

## P1: Transition parsing failure behavior overview

**Current behavior:** On parse failure, auto-retries once with a corrective prompt (resumes same session). If that also fails, blocks on user input loop. The corrective prompt shows the error and generic hardcoded examples but doesn't include the actual available agents/args from the system prompt.

**Proposed changes:**

1. **Enrich corrective prompt with available agents** — pass `&[AgentDef]` to `corrective_prompt()` so the retry includes the real agent names and args, not just `agent: main`. Low effort, likely big impact on retry success rate.

2. **Add a second auto-retry before user input** — bump from 1 to 2 auto-retries. The first retry often fails for the same reason (model doesn't have enough context to self-correct); a second attempt with the enriched prompt would catch most cases before blocking on user input.

**Questions:**
- Do these two changes sound right, or do you want something different?

---

## P1: Add wait-for-user tag

New `<wait-for-user>` tag for agents to signal they're blocked on user input (e.g. needing permission for a necessary command).

## Done

- P1: Add "Done" section to board
