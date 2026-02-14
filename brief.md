# Brief

Sometimes the agent tries to transitions with invalid yaml and it fails. Example:
```
<next>
agent: main
task: Refine post-compaction context: system.md scope and dispatch faithfulness
</next>
```
I think it's not receiving a very good error message, because it tried the same thing again (only succeeding on the third try). Propose either an improvement to the error message the model gets, or a better format (yaml kinda sucks doesn't it).
