# Brief

## Comments on the recent agent restructuring

### dispatch.md

- Can clarify a bit more that preserving human input and decisions faithfully is important (so implementation doesn't diverge from the plan)

### plan.md

- "## Plan" section: not accurate. I think it's more about understanding the requirements and findings ambiguity or inconsistency. Please re-draft. The main goal for the plan agent is to elicit the human's preferences for implementation via bringing up the right questions
- "Recording Issues" section: since this applies to all agents, it should be in system.md instead (though the review agent should have emphasis)

### implement.md

- I've dropped the "If you hit ambiguity" paragraph: rely on review instead
- Can overall be simplified - too many small sections. What's important is that it transitions to review when it's done

### review.md

- Rather than framing as gating the implementer's work, I think it would be healthier as framing it as evaluating the changes. It's not about the implementer doing a good/poor job, it's about whether the changes match the criteria
- Maybe "push back" has a somewhat negative valence?
- Should have a bit of emphasis but with different wording on top of the system.md on noticing issues and adding them to the board: this is part of the review task (for issues that shouldn't block landing or are unrelated)

General note: I've updated the agents without re-recording vcr, we'll have to do that again after the changes (tests currently fail, that's ok)
