# Brief

## Agent restructuring comments

This overall looks good. I think the dispatch agent should also prioritize delegating a planning task over an implementation task (but throttle depending on priority and how many blocked tasks there are). So priority should be something like:

Plan P0
Implement P0
Plan P1 (throttled)
Implement P1 (not throttled)
Plan P2 (throttled)
Implement P2 (not throttled)

"post questions to the board" for the implement agent: should be clear that this means putting it into Blocked. Though honestly this can be quite light, we even have the review gate (review can still evaluate whether new meaningful decisions were made that weren't in the plan)

Answers:
- Sections sound good
- Plan agent needs write permission to the board, I wouldn't worry about making it explicitly read-only right now (just via prompting)
- Planning over implementation: yes, see above
- Prompt drafts seem like a good start

Last comment: we should make sure that everything is coherent, including system.md. Prefer explaining the why to explaining the what

This is ready to implement! Make sure to re-record vcr
