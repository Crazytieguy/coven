# Blocked

## P1: Agent restructuring — split main into plan + implement

### Proposal

**New board sections** — add `# Plan` to distinguish planning-needed from implementation-ready:

```
# Blocked     ← needs human input (unchanged)
# Plan        ← needs planning (default state for new issues)
# Ready       ← implementation ready (human approved plan)
# Done        ← completed (unchanged)
```

**New lifecycle:**

```
dispatch → plan → dispatch → [human answers] → dispatch → implement × N → review → dispatch
```

**Changes to each agent:**

**dispatch** — Route by section. `# Plan` issues → plan agent. `# Ready` issues → implement agent. New brief items default to `# Plan` unless the human says otherwise. When brief answers questions on a blocked issue, dispatch infers: still unclear → `# Plan`, clear enough → `# Ready`. Same priority/throttling logic applies across both sections.

**plan** (new, replaces main's "post to board" path) — Read-only exploration. Reads the issue, explores the codebase, produces a concise plan: key decisions made + open questions. Moves issue to `# Blocked`, lands, transitions to dispatch. No code modifications. Prompt core:

> Read the board issue. Explore the codebase to understand the problem. Post a concise plan: key decisions and open questions only — no implementation details the human doesn't need. Move the issue under `# Blocked`, commit, land, transition to dispatch.

**implement** (new, replaces main's "implement" path) — Focused execution. Only picks up `# Ready` issues where a plan has been approved. Same permissions and flow as current main's implementation mode. Escape hatch: if implementation reveals ambiguity, discard work and post to board. Prompt core:

> Implement the board issue. The plan has been approved — follow the decisions. If you hit ambiguity, stop, discard uncommitted changes, post questions to the board, and transition to dispatch. Otherwise, commit your work and continue or transition to review.

**review** — Mostly unchanged. Continues to review implementation quality and can push back.

**Agent-added issues** — All agents can add issues to `# Plan` (default) liberally. This ensures agent-spotted problems get human review before implementation.

**Decisions:**
- Plan agent is read-only (no code modifications, just exploration + git for board updates)
- Implement agent retains escape hatch for posting questions

**Questions:**
- Does the `# Blocked` / `# Plan` / `# Ready` / `# Done` section approach work, or prefer a different mechanism?
- Plan agent permissions: read-only + git, or does it need anything else?
- Should dispatch prioritize planning over implementation at the same priority level (so plans get reviewed faster)?
- Prompt drafts above capture the right tone and constraints? Anything to add or remove?

# Ready

## P1: Investigate follow-up messages vs. next tag — what wins?

When a coven session outputs a `<next>` tag but also has follow-up messages queued, what wins? Is the continuation agent spawned?

# Done

- P2: Fix parent auto-continue during fork (kill parent CLI before fork children run, respawn with reintegration message after)
- P1: Fix invisible claude sessions (kill CLI after Result in worker/ralph to prevent async task continuations)
- P1: Coordinate worker sleep — if one dispatch sleeps, others should too

- P1: Review: is `git reset --hard main` correct in the review agent?
- P1: Implement new board format (replace divider with Blocked/Ready sections)
- P2: Capture stderr from claude process
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
- P1: Simplify status line after exiting embedded interactive session
- P1: wait-for-user prompt final revision
- P2: scratch.md: should clarify that it's gitignored
- P1: Mark a session to wait for user input when it finishes
