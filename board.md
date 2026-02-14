# Board

---

## P1: Prefer waiting for user input over exiting

Compaction session crashed because the agent asked a question and coven exited. Either the retry limit for tag parsing failure was reached, or something else. Coven should always prefer to wait for user input rather than exiting.

## P1: Add wait-for-user tag

New `<wait-for-user>` tag for agents to signal they're blocked on user input (e.g. needing permission for a necessary command).

## P1: Propose improvements for post-compaction context loss

Model tried `git push` instead of `bash .coven/land.sh` after session compaction. Also confused about interactive/non-interactive transitions. Propose possible improvements.

## P1: Display compaction messages

Show compaction messages compressed to a single line by default, viewable with `:N`. The previous worker found the format but crashed before landing.
