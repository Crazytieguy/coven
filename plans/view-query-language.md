Issue: [P1] VCR test view queries use fragile numeric indices that break on re-recording
Status: approved

## Design

Replace numeric view queries (e.g. `views = ["2", "3", "9", "11"]`) with label-based queries using Python-style indexing.

Syntax: `Label[index]` where index supports negative values. Plain `Label` defaults to `Label[0]`.

Examples:
- `Bash[0]` — first Bash message
- `Bash[-1]` — last Bash message
- `Edit[0]` — first Edit message
- `Read[0]` — first Read message

Matching: the query label is matched against the tool name portion of stored message labels (e.g. `[7] ▶ Bash  bash test.sh` matches "Bash"). Collect all matches, then index in.

Numeric queries (`"3"`) continue to work for backward compatibility.
