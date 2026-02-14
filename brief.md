# Brief

The new "Clean up stale content from board entries" section in the dispatch agent is a tad bit too agressive. Should be improved by removing content rather than adding more. The main point
Compaction session crashed due to the agent asking a question, and the worktree was removed without the work landing. Either the retry limit for tag parsing failure was reached, or something else caused coven to exit. Rather than exiting, coven should always prefer to wait for user input
Additionally, there should be a new <wait-for-user> tag option for the model, in case it's truly blocked on needing something from the user (such as needing permission for a necessary command)
Finally, the model actually tried to `git push` instead of `bash .coven/land.sh`. Its session was compacted, so maybe it lost the context. It was also confused about me jumping in for interactive discussion and then jumping back out to non-interactive. Propose some possible improvements
I want to be able to see compaction messages. Compressed to a single line by default, but viewable with :N. The worker that was researching compaction issues found the format for this but unfortunately crashed (see above)
Otherwise we didn't actually find issues with compaction. But we did (re)learn that in order to run claude within claude we need to remove an environment variable (vcr recording already does this). We should add this to CLAUDE.md in case this is needed in the future
