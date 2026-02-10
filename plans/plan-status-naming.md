Issue: [P2] Brainstorm alternatives to "approved" for plan status naming. "Approved" is awkward — consider: ready, accepted, go, etc.
Status: draft

## Approach

This is a convention-only change — no Rust code parses plan statuses. The change affects `workflow.md` and any existing plan files that use the old status name.

Replace `approved` with the chosen alternative in:

1. `workflow.md` — the status documentation and implementation instructions
2. Any in-flight plan files still using the old name

## Questions

### Which name should replace "approved"?

Options with tradeoffs:

| Name | Pro | Con |
|------|-----|-----|
| `ready` | Clear intent ("ready to implement"), neutral tone | Could be confused with "ready for review" |
| `accepted` | Standard term, clear meaning | Still sounds formal/bureaucratic |
| `go` | Short, punchy, unambiguous action signal | Unusual for a status field, might read oddly |
| `do` | Even shorter, imperative | Very terse, could be confusing |
| `confirmed` | Clear that human has verified | Verbose, still formal |

My recommendation: **`go`** — it's the most distinct from "draft" and "rejected", reads naturally ("Status: go"), and its brevity matches the lightweight feel of the workflow.

Answer:

## Review

