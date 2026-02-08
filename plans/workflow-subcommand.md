Issue: [P2] Big issue: integrate our workflow directly into a coven subcommand, based on workflow.md
Status: draft

## Approach

### Overview

Currently, the autonomous workflow (issue tracking, plan-based development, priorities) is implemented entirely as a prompt that Claude reads from `workflow.md` each iteration. This works but has downsides:

- Every iteration re-reads and re-interprets the full workflow instructions, wasting context
- Claude must parse `issues.md` and plan file statuses itself, which is error-prone
- No structured state tracking between iterations — everything is inferred from filesystem
- The workflow prompt is large and crowds out the actual task context

A `coven workflow` subcommand would move orchestration logic into coven itself. Coven parses the workflow state (issues, plans, git status) and constructs a focused, minimal prompt for each iteration based on what needs doing.

### Architecture

```
coven workflow [--issues issues.md] [--plans-dir plans/] [-- extra claude args]
```

Each iteration:
1. **Coven** reads `issues.md`, parses issues with priorities and plan references
2. **Coven** checks plan file statuses (draft/approved/rejected) and git status for modifications
3. **Coven** determines the single highest-priority action using the priority rules
4. **Coven** constructs a focused prompt describing just that action
5. **Claude** executes in a fresh ralph-style session
6. **Coven** validates the result (clippy, tests, etc.) — or instructs Claude to do so
7. Loop continues

### Phase 1: Issue/plan parsing (`src/workflow/`)

New module `src/workflow/` with:

```rust
// src/workflow/issues.rs
pub struct Issue {
    pub text: String,
    pub priority: Priority,       // P0, P1, P2 (default P1)
    pub plan_path: Option<PathBuf>,
}

pub enum Priority { P0, P1, P2 }

pub fn parse_issues(content: &str) -> Vec<Issue>
// Parse markdown list items, extract [P0]/[P1]/[P2] tags, extract (plan: ...) references

// src/workflow/plan.rs
pub struct Plan {
    pub path: PathBuf,
    pub issue_text: String,
    pub status: PlanStatus,
}

pub enum PlanStatus { Draft, Approved, Rejected }

pub fn parse_plan(content: &str) -> Plan
// Parse plan file frontmatter for Status: field
```

### Phase 2: Action selection (`src/workflow/scheduler.rs`)

```rust
pub enum Action {
    RunClippy,
    PlanIssue(Issue),
    ImplementPlan(Plan),
    RevisePlan(Plan),
    ReviewTestCases,
    AddTestCoverage,
    Refactor,
}

pub fn next_action(issues: &[Issue], plans: &[Plan], modified_plans: &[PathBuf]) -> Action
```

Implements the priority rules from workflow.md:
1. Lint first (check if clippy is clean — could skip if last iteration passed)
2. Modified plan files → check status, implement/revise as appropriate
3. Highest-priority issue without plan → create plan
4. Within same priority: planning > implementing
5. Across levels: implementing higher > planning lower
6. Fallback: review tests, add coverage, refactor

### Phase 3: Prompt construction (`src/workflow/prompt.rs`)

Each action maps to a focused prompt. Examples:

- **PlanIssue**: "Create a plan for this issue: {issue text}. Write it to plans/{name}.md using this format: {template}. Update issues.md to link the plan."
- **ImplementPlan**: "Implement this approved plan: {plan content}. Run clippy and tests before committing. When done, remove the issue from issues.md and delete the plan file."
- **RevisePlan**: "Revise this plan based on review comments: {plan content with review}. Clear the Review section after revising."
- **RunClippy**: "Run `cargo clippy` and fix any warnings."

The prompt also includes a condensed version of the coding conventions from CLAUDE.md (commit discipline, testing, etc.) — but NOT the full workflow rules, since coven is now handling orchestration.

### Phase 4: Command implementation (`src/commands/workflow.rs`)

Built on the existing ralph infrastructure:

```rust
pub struct WorkflowConfig {
    pub issues_path: PathBuf,      // default: issues.md
    pub plans_dir: PathBuf,        // default: plans/
    pub extra_args: Vec<String>,
    pub show_thinking: bool,
}

pub async fn workflow(config: WorkflowConfig) -> Result<()>
```

Loop structure similar to ralph:
1. Parse current state (issues, plans, git status)
2. Determine next action
3. Build focused prompt
4. Spawn session, run to completion
5. Post-session validation (did clippy pass? did tests pass?)
6. If validation fails, next iteration gets a "fix the failures" prompt
7. Continue loop

Uses `session_loop::run_session()` for each iteration, same as ralph.

### Phase 5: CLI integration

Add `Workflow` variant to `Command` enum in `src/cli.rs`:

```rust
Workflow {
    #[arg(long, default_value = "issues.md")]
    issues: PathBuf,
    #[arg(long, default_value = "plans/")]
    plans_dir: PathBuf,
}
```

### What stays in the prompt vs. moves to coven

| Currently in workflow.md | Moves to coven | Stays in prompt |
|---|---|---|
| Priority rules | Yes (scheduler) | No |
| Issue/plan parsing | Yes (parser) | No |
| Action selection | Yes (scheduler) | No |
| Plan file template | Yes (prompt builder) | Template injected per-action |
| Coding conventions | No | Yes (condensed) |
| "One action then end" | Partially — coven runs one action per session | Brief reminder |
| Review before commit | Stays in prompt | Yes |
| Break tag for completion | Replaced — coven decides when to stop | No |

### Files to create/modify

| File | Change |
|------|--------|
| **NEW** `src/workflow/mod.rs` | Module exports |
| **NEW** `src/workflow/issues.rs` | Issue parsing |
| **NEW** `src/workflow/plan.rs` | Plan file parsing |
| **NEW** `src/workflow/scheduler.rs` | Action selection logic |
| **NEW** `src/workflow/prompt.rs` | Per-action prompt construction |
| **NEW** `src/commands/workflow.rs` | Command implementation |
| `src/commands/mod.rs` | Export workflow module |
| `src/cli.rs` | Add Workflow subcommand |
| `src/main.rs` | Dispatch to workflow command |

## Questions

### How much orchestration should move to coven vs. stay in Claude's prompt?

The plan proposes a "thick coven" approach where coven parses state, selects actions, and gives Claude focused single-task prompts. The alternative:

A. **Thick coven (proposed):** Coven handles orchestration. Claude gets single-task prompts. Pro: less context waste, more reliable action selection, structured state. Con: more code to maintain, less flexible (adding a new action type requires code changes).

B. **Thin coven:** Coven just manages the loop and provides parsed state as context (e.g. "Here are the current issues: ... Here are the plan statuses: ... Follow the workflow rules to pick your action."). Claude still decides what to do. Pro: flexible, easy to change rules. Con: still wastes context on rules, Claude may misinterpret priorities.

C. **Hybrid:** Coven picks the action, but the prompt includes enough context that Claude could override if the situation doesn't match (e.g. discovers a blocking bug while implementing). Pro: best of both worlds. Con: slightly more complex prompting.

Answer:

### Should the workflow subcommand support user interaction between iterations?

A. **Fully autonomous (proposed):** Like ralph mode but smarter — no user input between iterations. The human reviews asynchronously by editing plan files. Ctrl+C to stop.

B. **Interactive checkpoints:** After each iteration, show a summary and prompt "Continue? [y/n/edit]". Allows mid-run course correction.

C. **Mixed:** Autonomous by default, but pause for confirmation on certain actions (e.g. before implementing an approved plan, or after a test failure).

Answer:

### Should we keep ralph mode separate or merge it into workflow?

Ralph mode is a general-purpose loop. The workflow subcommand is an opinionated workflow built on the same loop infrastructure. Options:

A. **Keep separate (proposed):** `coven ralph` remains the general loop, `coven workflow` is the opinionated workflow runner. They share `session_loop` infrastructure but are independent commands.

B. **Merge:** Make `coven ralph` accept a `--workflow` flag that enables the structured workflow behavior. Simpler CLI surface but muddies ralph's generality.

C. **Replace:** Remove ralph, make workflow the only loop command, with a "freeform" mode that behaves like current ralph. Cleaner but breaking change.

Answer:

### How should coven detect iteration success/failure?

Currently, Claude's output is just text. For coven to validate results, it needs to know if the iteration succeeded. Options:

A. **Post-iteration checks (proposed):** After each session, coven runs `cargo clippy` and `cargo test` itself (as subprocesses). If they fail, the next iteration's prompt says "fix these failures: {output}".

B. **Structured output:** Require Claude to emit structured status (e.g. `<status>success</status>` or `<status>failed: clippy warnings</status>`). Coven parses this.

C. **Git-based:** Check if there are new commits after the iteration. No commits = nothing happened = maybe stuck.

D. **Trust Claude:** Just let Claude manage validation in-prompt as it does now. Coven only handles orchestration.

Answer:

## Review

