---
priority: P1
state: new
---

# Replace polling with instant notifications

The worker's `wait_for_new_commits` polls `git rev-parse` every 10 seconds. Replace this with a filesystem watcher (e.g. `notify` crate) on the git refs to get instant wake-up on new commits.

Also audit the codebase for other polling patterns and convert them where possible.
