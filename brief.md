# Brief

Make sure the spurious wake up fix doesn't cause a race condition. Also check if there might be other spurious wakeups: For instance I think maybe workers are woken up when another worktree is removed
