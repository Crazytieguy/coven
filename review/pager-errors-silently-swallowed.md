---
priority: P2
state: review
---

# `view_message` silently swallows all pager errors

`src/commands/session_loop.rs:520-542` — the `view_message()` function has multiple silent failure points:

1. **Line 526:** If the pager fails to spawn (e.g., `$PAGER` is set to a nonexistent binary), `.spawn()` returns `Err` but the entire `if let Ok(ref mut child)` block is skipped — the user sees nothing and gets no error message.

2. **Line 530:** `stdin.write_all(...).ok()` — if writing to the pager fails (broken pipe), the error is silently discarded.

3. **Line 534:** `child.wait().ok()` — pager exit status is ignored.

4. **Line 542:** `terminal::enable_raw_mode().ok()` — if re-enabling raw mode fails after the pager exits, the terminal is left in cooked mode. Subsequent input handling will break silently.

**Impact:** Users with a broken `$PAGER` configuration get no feedback when trying to inspect messages with `:N`. Terminal state corruption after pager failure is hard to diagnose.

**Fix:** At minimum, print an error line when the pager fails to spawn. For `enable_raw_mode`, consider logging the error or panicking since the session can't continue correctly without raw mode.

## Plan

All changes in `src/commands/session_loop.rs`, in `view_message()` (currently lines 502–543).

### 1. Report pager spawn failure

Replace the silent `if let Ok(ref mut child) = child` with a match:

```rust
let mut child = match child {
    Ok(child) => child,
    Err(e) => {
        // Re-enable raw mode before writing the error, since write_raw expects raw mode
        terminal::enable_raw_mode().ok();
        renderer.write_raw(&format!("Failed to open pager '{pager}': {e}\r\n"));
        return;
    }
};
```

This shows the user a clear error when `$PAGER` doesn't exist or can't be executed.

### 2. Report pager stdin write failure

Replace `stdin.write_all(content.as_bytes()).ok();` with:

```rust
if let Err(e) = stdin.write_all(content.as_bytes()) {
    // Not fatal — pager may have quit early (broken pipe). Log and continue
    // so we still wait on the child and restore terminal state.
    eprintln!("pager write error: {e}");
}
```

Use `eprintln!` here since we're in cooked mode (raw mode was disabled before spawning the pager), so stderr goes straight to the terminal.

### 3. Leave `child.wait().ok()` as-is

The pager's exit code doesn't matter — `less` returns non-zero on `q` in some cases, and a failed pager doesn't affect our state. The important thing is that we wait for it to exit. No change needed.

### 4. Panic on `enable_raw_mode` failure

Replace `terminal::enable_raw_mode().ok();` with:

```rust
terminal::enable_raw_mode().expect("failed to re-enable raw mode after pager");
```

If raw mode can't be re-enabled, the session is broken — input handling relies on it. Panicking is appropriate here since `RawModeGuard::acquire` also propagates `enable_raw_mode` failures (via `?`), establishing that the session can't function without it. The panic will be caught by the cleanup in `main.rs` which disables raw mode on exit.

### 5. Leave `disable_raw_mode().ok()` as-is (line 520)

Failing to disable raw mode before spawning the pager is a cosmetic issue (pager may not work perfectly) but not worth crashing over — the pager spawn itself will likely fail or the user can quit. Consistent with `RawModeGuard::drop` which also uses `.ok()`.
