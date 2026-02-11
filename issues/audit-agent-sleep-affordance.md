---
priority: P1
state: new
---

# Audit agent should be allowed to find nothing and sleep

The audit agent currently always proposes issues. If none of its findings are genuine improvements, it creates low-value issues anyway, which can lead to an infinite audit loop — audit finds nothing real, creates weak issues, those get rejected or resolved, audit runs again, repeat.

The audit agent should have the affordance to sleep (like the dispatch agent) when it doesn't find any issues worth adding. It's fine to audit and conclude "nothing to report."
