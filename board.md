# Board

---

## P1: Add wait-for-user tag

New `<wait-for-user>` tag for agents to signal they're blocked on user input (e.g. needing permission for a necessary command).

## P1: Propose improvements for post-compaction context loss

Model tried `git push` instead of `bash .coven/land.sh` after session compaction. Also confused about interactive/non-interactive transitions. Propose possible improvements.

## P1: Transition parsing failure behavior overview

Research current behavior when transition parsing fails (retries, error handling) and propose a couple of small possible changes (retry iterations, etc). Present findings on the board.

## P1: Main agent should ask more questions

Update main agent prompt to lean more towards asking questions, even for small decisions where several approaches are viable. Overshoot slightly in that direction without being aggressive.

## P1: Add "Done" section to board

Change board format so completed issues move to a "Done" section (single line per item) instead of being removed entirely. Only clean up the Done section via explicit request from brief.md. Update dispatch and system prompts accordingly.
