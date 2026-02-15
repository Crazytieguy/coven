# Brief

I think if several coven workers are woken up from sleep by commits on main, as soon as one of them decides to sleep, the others should go back to sleep as well. So for instance if there are 4 workers (a b c d), this might be a valid sequence:

- a entry agent decides to transition
- b entry agent decides to transition
- c entry agent decides to sleep
- d entry agent doesn't run

The workers start from the same state, so there's no reason to run dispatch again if the result is sleeping.

If you can think of an even better way to handle this: propose an approach. If implementation is tricky: lmk what you're unsure about. Otherwise implement without asking for confirmation.
