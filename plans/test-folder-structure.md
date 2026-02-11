Issue: [P1] The test cases folder is currently a flat list of files, and it's hard for me to navigate. I want to organize all tests into a neat folder structure
Status: draft

## Approach

Move test case files from the flat `tests/cases/` directory into command-type subdirectories:

```
tests/cases/
  run/
    simple_qa.toml, simple_qa.vcr, simple_qa.snap, simple_qa__views.snap
    tool_use.toml, tool_use.vcr, tool_use.snap, tool_use__views.snap
    grep_glob.toml, grep_glob.vcr, grep_glob.snap
    mcp_tool.toml, mcp_tool.vcr, mcp_tool.snap, mcp_tool__views.snap
    error_handling.toml, error_handling.vcr, error_handling.snap, error_handling__views.snap
    multi_turn.toml, multi_turn.vcr, multi_turn.snap
    steering.toml, steering.vcr, steering.snap
    write_single_line.toml, write_single_line.vcr, write_single_line.snap
    edit_tool.toml, edit_tool.vcr, edit_tool.snap, edit_tool__views.snap
    show_thinking.toml, show_thinking.vcr, show_thinking.snap
    subagent.toml, subagent.vcr, subagent.snap
    subagent_error.toml, subagent_error.vcr, subagent_error.snap
    parallel_subagent.toml, parallel_subagent.vcr, parallel_subagent.snap
    interrupt_resume.toml, interrupt_resume.vcr, interrupt_resume.snap
  ralph/
    ralph_break.toml, ralph_break.vcr, ralph_break.snap
  worker/
    worker_basic.toml, worker_basic.vcr, worker_basic.snap
    concurrent_workers.toml, concurrent_workers.snap, concurrent_workers__*.vcr
  fork/
    fork_basic.toml, fork_basic.vcr, fork_basic.snap
    fork_buffered.toml, fork_buffered.vcr, fork_buffered.snap
    fork_single.toml, fork_single.vcr, fork_single.snap
  init/
    init_fresh.toml, init_fresh.vcr, init_fresh.snap
  status/
    status_no_workers.toml, status_no_workers.vcr, status_no_workers.snap
  gc/
    gc_no_orphans.toml, gc_no_orphans.vcr, gc_no_orphans.snap
```

### Code changes

1. **Test macros in `tests/vcr_test.rs`**: Add a `category` parameter to the macros so the test can find its files in the right subdirectory.

   ```rust
   macro_rules! vcr_test {
       ($category:ident / $name:ident) => {
           #[tokio::test]
           async fn $name() {
               let result = run_vcr_test(stringify!($category), stringify!($name)).await;
               // snapshot_path points to the subdirectory
               insta::with_settings!({
                   snapshot_path => concat!("../tests/cases/", stringify!($category)),
                   prepend_module_to_snapshot => false,
               }, {
                   insta::assert_snapshot!(stringify!($name), result.display);
               });
               // ... views snapshot similarly
           }
       };
   }
   ```

   Invoked as: `vcr_test!(run / simple_qa);`

2. **`run_vcr_test` and `run_multi_vcr_test`**: Accept a `category` parameter and construct paths as `tests/cases/{category}/{name}.toml` etc.

3. **`record_case` in `src/bin/record_vcr.rs`**: The recorder needs to discover test cases across subdirectories. Change discovery to walk subdirectories of `tests/cases/`, collecting `(category, name)` pairs. The VCR path becomes `tests/cases/{category}/{name}.vcr`. Accept CLI arg as either `name` (search all subdirs) or `category/name`.

4. **Move the files**: Move each group of files into its subdirectory. No renames needed — filenames stay the same.

### Migration

This is a one-shot migration: move files, update code, re-run tests. No VCR re-recording needed since the files are just moved, not modified.

## Questions

### Should `init`, `status`, and `gc` each get their own subdirectory, or share an `auxiliary` folder?

They each have only one test case currently. Separate directories match the command structure but may feel sparse. A shared `auxiliary/` folder is more compact but less discoverable.

Answer:

### Should fork tests live under `run/` or in their own `fork/` directory?

Fork is a flag on run mode (`--fork`), so these could reasonably go under `run/`. But they test distinct fork-specific behavior, and having a `fork/` category makes them easy to find.

Answer:

## Review

