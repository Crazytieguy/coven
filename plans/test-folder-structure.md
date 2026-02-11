Issue: [P1] The test cases folder is currently a flat list of files, and it's hard for me to navigate. I want to organize all tests into a neat folder structure
Status: done

## Approach

Reorganize `tests/cases/` into themed top-level folders, with a dedicated subfolder per test case. Each test case folder contains its `.toml`, `.vcr`, and `.snap` files.

### Target structure

```
tests/cases/
  session/
    simple_qa/
      simple_qa.toml, simple_qa.vcr, simple_qa.snap, simple_qa__views.snap
    multi_turn/
      multi_turn.toml, multi_turn.vcr, multi_turn.snap
    steering/
      steering.toml, steering.vcr, steering.snap
    interrupt_resume/
      interrupt_resume.toml, interrupt_resume.vcr, interrupt_resume.snap
    show_thinking/
      show_thinking.toml, show_thinking.vcr, show_thinking.snap
    error_handling/
      error_handling.toml, error_handling.vcr, error_handling.snap, error_handling__views.snap
  rendering/
    tool_use/
      tool_use.toml, tool_use.vcr, tool_use.snap, tool_use__views.snap
    grep_glob/
      grep_glob.toml, grep_glob.vcr, grep_glob.snap
    mcp_tool/
      mcp_tool.toml, mcp_tool.vcr, mcp_tool.snap, mcp_tool__views.snap
    edit_tool/
      edit_tool.toml, edit_tool.vcr, edit_tool.snap, edit_tool__views.snap
    write_single_line/
      write_single_line.toml, write_single_line.vcr, write_single_line.snap
  subagent/
    subagent/
      subagent.toml, subagent.vcr, subagent.snap
    parallel_subagent/
      parallel_subagent.toml, parallel_subagent.vcr, parallel_subagent.snap
    subagent_error/
      subagent_error.toml, subagent_error.vcr, subagent_error.snap
  fork/
    fork_basic/
      fork_basic.toml, fork_basic.vcr, fork_basic.snap
    fork_buffered/
      fork_buffered.toml, fork_buffered.vcr, fork_buffered.snap
    fork_single/
      fork_single.toml, fork_single.vcr, fork_single.snap
  ralph/
    ralph_break/
      ralph_break.toml, ralph_break.vcr, ralph_break.snap
  orchestration/
    worker_basic/
      worker_basic.toml, worker_basic.vcr, worker_basic.snap
    concurrent_workers/
      concurrent_workers.toml, concurrent_workers.snap, concurrent_workers__init.vcr, concurrent_workers__worker_a.vcr, concurrent_workers__worker_b.vcr
    init_fresh/
      init_fresh.toml, init_fresh.vcr, init_fresh.snap
    status_no_workers/
      status_no_workers.toml, status_no_workers.vcr, status_no_workers.snap
    gc_no_orphans/
      gc_no_orphans.toml, gc_no_orphans.vcr, gc_no_orphans.snap
```

**Theme rationale:**
- **session** — core session lifecycle: starting, multi-turn, interrupting, steering, errors, thinking display
- **rendering** — tool output display: how specific tools look when rendered
- **subagent** — subagent spawning and error handling
- **fork** — fork mode (parallel sub-sessions)
- **ralph** — ralph loop mode
- **orchestration** — worker, init, status, gc (the multi-agent workflow)

### Code changes

1. **Test macros in `tests/vcr_test.rs`**: Add `theme` and `name` parameters. The path becomes `tests/cases/{theme}/{name}/{name}.toml` etc.

   ```rust
   macro_rules! vcr_test {
       ($theme:ident / $name:ident) => {
           #[tokio::test]
           async fn $name() {
               let result = run_vcr_test(stringify!($theme), stringify!($name)).await;
               insta::with_settings!({
                   snapshot_path => concat!("../tests/cases/", stringify!($theme), "/", stringify!($name)),
                   prepend_module_to_snapshot => false,
               }, {
                   insta::assert_snapshot!(stringify!($name), result.display);
               });
               // ... views snapshot similarly
           }
       };
   }
   ```

   Invoked as: `vcr_test!(session / simple_qa);`

2. **`run_vcr_test` and `run_multi_vcr_test`**: Accept `theme` and `name` parameters. Construct paths as `tests/cases/{theme}/{name}/{name}.toml`, `tests/cases/{theme}/{name}/{name}.vcr`, etc.

3. **`record_case` in `src/bin/record_vcr.rs`**: Walk `tests/cases/{theme}/{name}/` structure to discover cases. Accept CLI arg as `name` (search all themes) or `theme/name`. The VCR path becomes `tests/cases/{theme}/{name}/{name}.vcr`.

4. **Move the files**: Create the directory structure and move each test case's files into its own folder within the appropriate theme directory.

### Migration

One-shot migration: create directories, move files, update code, run tests. No VCR re-recording needed since the files are just relocated.

## Questions

## Review

