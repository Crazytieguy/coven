# Brief

## Agent restructuring

I think we still have too many cases of issues being implemented without asking questions. Probably we can simplify and make it easier for the agents to follow. We should split the main agent into two: plan, and implement. Issues are explicitly marked as requiring planning by default, but the human can say via brief that an issue can go straight to implementation. The new lifecycle is:
- Issue added via brief, marked as needs planning (via dispatch)
- Planning -> results in questions for the human (via plan agent)
- Human answers questions, dispatch needs to infer whether the issue still needs planning or if it can transition to implementation ready
- Either re-plan or implement

This more robustly ensures that things don't just make it into the codebase before the human is happy with the approach.

The implementation agent should still have an escape hatch to add questions if needed, but less critical.

"Plans" should be kept concise: the human doesn't want to read through a bunch of irrelevant implementation details. Just the key decisions and open questions.

Unsure what to do about agent-added issues: probably best to have them as requiring planning by default (so a plan agent can pick them up, and later the human can approve the plan)

It's actually very important to me that all agents are empowered to add issues and do so liberally

Alright let's figure out the specifics: I want to be involved in the prompting decisions
