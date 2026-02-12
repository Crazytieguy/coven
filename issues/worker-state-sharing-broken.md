---
priority: P0
state: new
---

# Worker state sharing broken after agent overhaul

The recent agent overhaul broke worker state sharing. When a plan agent is working on an issue, another worker's dispatch agent sees all other workers as running dispatch agents — which isn't even possible due to the semaphore. The shared state between workers is not correctly reflecting which agent each worker is currently running.
