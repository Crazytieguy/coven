---
priority: P2
state: approved
---

# `resolve_ref_paths` runs git commands outside VCR

`src/commands/worker.rs:1067-1093` calls `worktree::main_branch_name()` and `std::process::Command::new("git")` directly, bypassing VCR recording:

```rust
fn resolve_ref_paths(worktree_path: &Path) -> Option<RefPaths> {
    let main_branch = worktree::main_branch_name(worktree_path).ok()?;
    let output = std::process::Command::new("git")
        .arg("-C").arg(worktree_path)
        .args(["rev-parse", "--git-common-dir"])
        .output().ok()?;
    // ...
}
```

This violates the project convention that all external I/O must go through `vcr.call()`.

## Impact

Low — the function is best-effort with `Option<RefPaths>` return and graceful fallback. During VCR replay, the git commands fail (worktree doesn't exist), `resolve_ref_paths` returns `None`, the watcher watches nothing, and the loop relies entirely on VCR-replayed `next_event` calls. Behavior is correct in all modes.

## Fix

Wrap the path resolution in a `vcr.call()`. This would require either:
1. Making `setup_ref_watcher` async and accepting `&VcrContext`, or
2. Pre-resolving the paths via VCR before calling `setup_ref_watcher`

Option 2 is simpler: resolve the ref paths through VCR at the start of `wait_for_new_commits` and pass them into `setup_ref_watcher`.

## Plan

Use option 2: pre-resolve ref paths via VCR in `wait_for_new_commits`, then pass the result into `setup_ref_watcher`.

### 1. Make `RefPaths` serializable

Add `Serialize, Deserialize` derives to the `RefPaths` struct (line 1060) so it can go through `vcr.call()`:

```rust
#[derive(Serialize, Deserialize)]
struct RefPaths {
    refs_heads_dir: PathBuf,
    loose_ref: PathBuf,
    packed_refs: PathBuf,
}
```

`PathBuf` already implements `Serialize`/`Deserialize` via serde, so this just works.

### 2. Create `vcr_resolve_ref_paths` wrapper

Add an async VCR wrapper following the `vcr_main_head_sha` pattern (lines 903-908):

```rust
/// VCR-wrapped `resolve_ref_paths`.
async fn vcr_resolve_ref_paths(vcr: &VcrContext, wt_str: String) -> Result<Option<RefPaths>> {
    vcr.call("resolve_ref_paths", wt_str, async |p: &String| {
        Ok(resolve_ref_paths(Path::new(p)))
    })
    .await
}
```

The return type `Result<Option<RefPaths>>` works because both `Option` and `RefPaths` (after step 1) implement `Serialize + DeserializeOwned`, satisfying the `Recordable` bound.

### 3. Change `setup_ref_watcher` to accept `Option<RefPaths>`

Change the signature from `fn setup_ref_watcher(worktree_path: &Path)` to `fn setup_ref_watcher(ref_paths: Option<RefPaths>)`. Remove the internal `resolve_ref_paths` call — it receives the already-resolved paths:

```rust
fn setup_ref_watcher(
    ref_paths: Option<RefPaths>,
) -> Result<(notify::RecommendedWatcher, tokio::sync::mpsc::Receiver<()>)> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    let mut watcher = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
        let _ = tx.try_send(());
    })
    .context("failed to create filesystem watcher")?;

    if let Some(paths) = ref_paths {
        // ... existing watch logic unchanged ...
    }

    Ok((watcher, rx))
}
```

### 4. Update call site in `wait_for_new_commits`

In `wait_for_new_commits` (line 1108), resolve paths via VCR before creating the watcher:

```rust
let ref_paths = vcr_resolve_ref_paths(vcr, wt_str.clone()).await?;
let (_watcher, mut rx) = setup_ref_watcher(ref_paths)?;
```

### 5. Re-record VCR fixtures and run tests

```
cargo run --bin record-vcr
cargo test
cargo insta accept   # if needed
```

The VCR recordings will now include a `resolve_ref_paths` entry, and replay will use the recorded paths instead of running git commands.
