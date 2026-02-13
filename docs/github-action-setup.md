# Claude Code GitHub Action: Setup Notes & Reference

Notes from setting up and testing the `anthropics/claude-code-action` for autonomous async work. Written as a reference for MATS fellows configuring similar workflows.

## Goal

Enable researchers to hand off work to Claude asynchronously. The workflow mirrors GitHub's natural model: creating an issue (assigning work), issue comments (clarifying ambiguities), branches (autonomous execution), and PRs (human review before merging). Experiment code, documentation, and analysis all live in the repo, providing an audit trail and reproducibility.

---

## Viability Assessment

We evaluated major potential blockers for using the GitHub Action as a runtime for autonomous research work. None turned out to be hard blockers.

### 6-Hour Job Timeout (GitHub-Hosted Runners)

**Not a blocker.** Actual GPU compute happens on remote machines (runpod, vast.ai). Claude orchestrates rather than runs experiments directly. Scope to experiments that fit within the 6-hour limit, or fast-paced Python iteration. Longer experiments where Claude needs to respond to job completion are a separate topic — worst case, a human reviews the results (on wandb) and opens a new issue for Claude.

### Bash Tool Restrictions

**Not a blocker.** Bash is disabled by default in the action and must be whitelisted per-command pattern. For autonomous research, broad permissions are needed (SSH, python, pip, curl, etc.). The right permission set can be found — see the permission model section below.

### GitHub-Centric I/O Model

**Not a blocker.** The concern was that all output goes into GitHub comments (~65K char limit), which is narrow for experiment logs and analysis. However, the GitHub issues + PRs model is actually the right fit: it was independently converged on when building a custom orchestration system. Experiment code and documentation belong in the repo anyway. Claude's full execution log is available as a viewable markdown file in the GitHub Action's Summary tab.

### No Human-in-the-Loop During Execution

**Not a blocker — this is a feature, not a bug.** The whole point is autonomous work. Claude should be prompted to stop and surface ambiguities or unexpected problems as issue comments rather than continuing blindly. Researchers can always use interactive Claude sessions on their own machines when they want human-in-the-loop.

### Context Window Pressure

**Real concern, but general.** Over long sessions with many tool calls, context compaction degrades quality as Claude loses earlier reasoning. This is the general long-horizon AI capability problem, not specific to the GitHub Action. Many people are working on solutions.

### Multi-Invocation Chaining

**Open question.** The action runs one Claude invocation per step. For dynamic chaining (where Claude decides to hand off to a new Claude call with a different prompt), the action doesn't natively support this. Two potential approaches:
1. Claude triggers a new `workflow_dispatch` via GitHub API, passing the next prompt as input.
2. A shell script step that loops, calling the Claude Code CLI directly.

The second bypasses the action layer. Whether this matters depends on how much value the action's niceties provide. Forking the action is a last resort.

---

## Architecture: How the Action Works

Understanding the internals helps make informed configuration decisions.

### Token Model

When using `claude_code_oauth_token` (recommended for fellows with Claude subscriptions):

1. The workflow starts and GitHub provides its default `GITHUB_TOKEN` (scoped by the `permissions:` block in the workflow YAML).
2. The action authenticates with the **Claude GitHub App** via OAuth. The App creates its own **installation token** for the repo — a separate token from `GITHUB_TOKEN`.
3. The App token has hardcoded permissions: **contents: write, pull-requests: write, issues: write**.
4. The action sets both `GITHUB_TOKEN` and `GH_TOKEN` environment variables to the App token (source: `run.ts` lines 161-163). This means the `gh` CLI automatically uses the App token.
5. After execution, the action **revokes** the App token.

**Implication:** The `permissions:` block in the workflow YAML is mostly irrelevant — it governs the default `GITHUB_TOKEN` which Claude doesn't primarily use. The App token is what matters, and it already has write access. The `additional_permissions` input requests extra scopes on the App token beyond the default trio (e.g., `actions: read` for CI log access).

### Tag Mode vs Agent Mode

The action has two modes, auto-detected based on whether a `prompt` input is provided:

| | Tag mode (no `prompt`) | Agent mode (with `prompt`) |
|---|---|---|
| Tracking comment | Auto-created, updated with progress | None — Claude must create its own |
| Issue/PR context | Auto-fetched (body, comments, diffs, review comments) and injected into prompt | Nothing — your prompt is used as-is |
| Built-in prompt | ~870 lines of detailed instructions | Your `prompt` text, verbatim |
| Allowed tools | Auto-configured: Edit, Read, Write, git commands, MCP tools | Must be specified via `claude_args` or `--permission-mode` |
| MCP servers | Comment server always included | Only included if you list matching `mcp__github_*` tools in `--allowedTools` |
| WebSearch/WebFetch | Explicitly disallowed | Not restricted (but not allowed either unless you add them) |

**Mode detection logic** (source: `src/modes/detector.ts`):
- If `prompt` is provided on a comment/issue/PR event → **agent mode**
- If no `prompt` and `@claude` trigger found → **tag mode**
- `track_progress: true` forces **tag mode** even with a custom `prompt`, but the custom prompt is appended to (not replacing) the built-in prompt

**We chose agent mode** because the built-in tag mode prompt is ~870 lines of opinionated instructions that can't be replaced, only appended to. Agent mode gives full control over the prompt.

### MCP Servers

The action provides several MCP (Model Context Protocol) servers. In agent mode, they are **conditionally included** — listing the corresponding tools in `--allowedTools` both enables the tool AND triggers the server setup.

| Server | Tools | Purpose |
|---|---|---|
| `github_comment` | `update_claude_comment` | Updates Claude's tracking comment. **Requires a pre-created comment ID that only tag mode provides.** Does not work in agent mode. |
| `github_ci` | `get_ci_status`, `get_workflow_run_details`, `download_job_log` | Reads CI workflow results and logs |
| `github_file_ops` | `commit_files`, `delete_files` | API-based commits (only for commit signing mode) |
| `github_inline_comment` | (inline review comments) | Posts inline comments on PR diffs. Only for PRs |
| `github` | (full GitHub MCP server) | Official `github/github-mcp-server` Docker image. Only included if you list `mcp__github__*` tools |

**Key finding:** The `update_claude_comment` MCP tool does not work in agent mode because it requires a `CLAUDE_COMMENT_ID` that is only set during tag mode's `createInitialComment` step. In agent mode, use `gh issue comment` with `--edit-last` for progress tracking instead.

### @claude Filtering

The `if` condition in the workflow YAML checks for `@claude` mentions **before the job even starts**. This is a GitHub Actions feature, not the Claude Code action. In agent mode, the action runs whenever `prompt` is provided — the workflow-level `if` is what prevents it from running on every comment.

### Execution Log

Both modes generate a step summary viewable in the Actions run's **Summary tab**. The log includes all tool calls and responses in a formatted markdown view. Tag mode additionally updates a tracking comment on the issue with a link to the run, making it more visible.

### Branch Handling

- Tag mode: Automatically creates a `claude/` branch via `setupBranch`.
- Agent mode: No automatic branch creation. We add a workflow step to create the branch before Claude runs, so Claude doesn't need checkout permissions.

---

## Permission Model (Deep Dive)

### Bash Permissions

The action **disables Bash by default**. Tools must be explicitly allowed via `--allowedTools` in `claude_args`.

**Colon syntax for multiline commands:** Use `Bash(command:*)` (colon before `*`) instead of `Bash(command *)` (space before `*`). The colon syntax correctly matches commands with multiline arguments (e.g., `gh issue comment` with a multi-paragraph body). Without the colon, permission matching fails on newlines.

**Observed:** `Bash(gh issue comment *)` was denied when Claude tried to post a multiline comment body. `Bash(gh issue comment:*)` works correctly.

### File Operation Permissions

The action's default permission mode does not include file editing permissions (Edit, Write, etc.). Read is available by default. The repo's `.claude/settings.json` controls Bash permissions but does not grant file edit permissions — those come from the permission mode.

**Observed:** Without `--permission-mode acceptEdits`, Claude was denied file edits. Read operations worked fine.

**Solution:** Use `--permission-mode acceptEdits` in `claude_args`. This grants Edit, Write, and other file modification tools.

### Git Push Safety

To prevent Claude from pushing to `main` or other protected branches:

- `Bash(git push origin HEAD)` (exact match, no wildcard) only allows pushing the current branch to its same-named remote ref.
- `Bash(git push origin claude/*)` is **NOT safe** — the refspec syntax `git push origin claude/issue-1:main` matches this pattern and pushes to `main`.
- `Bash(git push *)` allows `git push origin HEAD:main` which pushes to main without switching branches.
- **Branch protection rules on the repo are the strongest safeguard.** Permission patterns are a soft guardrail.

### Interaction Between Settings Layers

- `.claude/settings.json` (checked into repo) provides project-level Bash permissions
- `--allowedTools` in `claude_args` adds additional allowed tools (additive)
- `--permission-mode` controls file operation permissions (Edit, Write, etc.) independently of the allow list
- `--disallowedTools` can deny specific tools. Deny takes precedence over allow, but cannot selectively narrow a broad pattern (denying `Bash(git push *)` also blocks `Bash(git push origin HEAD)`)

---

## Our Configuration

### Decisions and Reasoning

| Decision | Reasoning |
|---|---|
| **Agent mode** over tag mode | Full control over the prompt. Tag mode's ~870-line built-in prompt is opinionated and can't be replaced, only appended to. |
| **`gh` CLI** over MCP tools for GitHub interaction | The `update_claude_comment` MCP tool doesn't work in agent mode (no pre-created comment ID). `gh issue comment --edit-last` provides equivalent functionality for progress tracking. |
| **Separate issue and PR workflows** | Different contexts (branch handling, prompt, triggers) make two focused files cleaner than one file full of conditionals. Issue workflow: `claude-issue.yml`. PR workflow: `claude-pr.yml`. |
| **Branch created by workflow step**, not Claude | Claude doesn't need general checkout permissions. The branch is ready before Claude starts. |
| **`--permission-mode acceptEdits`** | Grants file operation tools (Edit, Write, Read, etc.) without listing each one in `--allowedTools`. |
| **`Bash(git push origin HEAD)`** exact match | Prevents pushing to arbitrary branches. Combined with branch protection rules for defense in depth. |
| **`--json` flag for `gh issue view`** | In CI (no TTY), `gh issue view --comments` may not reliably show the title and body. `--json title,body,comments,labels` gives structured, reliable output. |
| **`--disallowedTools TodoWrite`** | Forces Claude to use the issue comment for progress tracking instead of an internal todo list that isn't visible to the user. |
| **`fetch-depth: 0`** (full checkout) | Shallow checkouts miss git history needed for understanding the codebase. |
| **Opus 4.6 model** | Sonnet 4.5 (the default) is not desired. Specify explicitly via `--model claude-opus-4-6`. |
| **Two equal paths in prompt** (ask questions vs implement) | Ambiguity should trigger questions, not guesses. Both outcomes are equally valid — the prompt emphasizes this rather than treating implementation as the default. |

### Current Workflow Files

#### Issue Workflow (`claude-issue.yml`)

```yaml
name: Claude Code (Issues)

on:
  issue_comment:
    types: [created]
  issues:
    types: [opened, assigned]

concurrency:
  group: claude-issue-${{ github.event.issue.number }}
  cancel-in-progress: true

jobs:
  claude:
    if: |
      (github.event_name == 'issue_comment' && contains(github.event.comment.body, '@claude') && !github.event.issue.pull_request) ||
      (github.event_name == 'issues' && (contains(github.event.issue.body, '@claude') || contains(github.event.issue.title, '@claude')))
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: read
      issues: read
      id-token: write
      actions: read
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Create working branch
        run: git checkout -b claude/issue-${{ github.event.issue.number }}

      - name: Run Claude Code
        id: claude
        uses: anthropics/claude-code-action@v1
        with:
          claude_code_oauth_token: ${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}
          additional_permissions: |
            actions: read
          prompt: |
            You are running autonomously via github action. You were triggered by
            an @claude mention on issue #${{ github.event.issue.number }}
            in ${{ github.repository }}.

            ## Getting started

            Read the full issue:
              gh issue view ${{ github.event.issue.number }} --json title,body,comments,labels

            Always post a new tracking comment (never reuse one from a previous run):
              gh issue comment ${{ github.event.issue.number }} --body "Starting work..."

            Update this comment as you work using:
              gh issue comment ${{ github.event.issue.number }} --edit-last --body "<updated content>"

            Use checklist format (- [ ] / - [x]) in your tracking comment to show progress.
            Update after each significant step — reading the issue, making each change,
            running tests, iterating on failures, pushing. The comment is the only way humans can see your progress.

            ## Choose one of two paths

            **Path A — Ask questions:** If the issue is ambiguous, underspecified, or you
            hit blockers during implementation, update your tracking comment with your
            questions and stop. Do not guess or make assumptions about unclear requirements.

            **Path B — Implement:** If the issue is clear, implement the changes on the
            current branch. When done:
              1. Push with: git push origin HEAD
              2. Create a PR: gh pr create --title "<title>" --body "<summary referencing #N>"
              3. Update your tracking comment with a summary and link to the PR.
          claude_args: |
            --model claude-opus-4-6
            --permission-mode acceptEdits
            --allowedTools "Bash(gh issue view:*),Bash(gh issue comment:*),Bash(gh pr create:*),Bash(git push origin HEAD)"
            --disallowedTools TodoWrite
```

**Key change from v1:** Added `!github.event.issue.pull_request` guard so PR comments don't trigger this workflow. PR comments are handled by the PR workflow instead.

#### PR Review Workflow (`claude-pr.yml`)

```yaml
name: Claude Code (PR Review)

on:
  pull_request_review:
    types: [submitted]
  issue_comment:
    types: [created]

concurrency:
  group: claude-pr-${{ github.event.pull_request.number || github.event.issue.number }}
  cancel-in-progress: true

jobs:
  claude:
    if: |
      (github.event_name == 'pull_request_review' &&
        (github.event.pull_request.user.login == 'claude[bot]' ||
         contains(github.event.review.body, '@claude'))) ||
      (github.event_name == 'issue_comment' &&
        github.event.issue.pull_request &&
        contains(github.event.comment.body, '@claude'))
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: read
      issues: read
      id-token: write
      actions: read
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Checkout PR branch
        run: gh pr checkout ${{ github.event.pull_request.number || github.event.issue.number }}
        env:
          GH_TOKEN: ${{ github.token }}

      - name: Run Claude Code
        id: claude
        uses: anthropics/claude-code-action@v1
        with:
          claude_code_oauth_token: ${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}
          additional_permissions: |
            actions: read
          prompt: |
            You are running autonomously via github action. You were triggered by
            a review or comment on PR #${{ github.event.pull_request.number || github.event.issue.number }}
            in ${{ github.repository }}.

            ## Getting started

            Read the PR with its reviews and comments:
              gh pr view ${{ github.event.pull_request.number || github.event.issue.number }} --json title,body,comments,reviews,labels

            Read inline review comments (these are not included in gh pr view):
              gh api repos/${{ github.repository }}/pulls/${{ github.event.pull_request.number || github.event.issue.number }}/comments --jq '.[] | {path, line, original_line, side, body, user: .user.login, in_reply_to_id}'

            Use git to understand what the PR changed (e.g. git diff, git log).

            Read any linked issues referenced in the PR body (look for #N references):
              gh issue view <number> --json title,body,comments,labels

            Always post a new tracking comment (never reuse one from a previous run):
              gh pr comment ${{ github.event.pull_request.number || github.event.issue.number }} --body "Starting work..."

            Update this comment as you work using:
              gh pr comment ${{ github.event.pull_request.number || github.event.issue.number }} --edit-last --body "<updated content>"

            Use checklist format (- [ ] / - [x]) in your tracking comment to show progress.
            Update after each significant step — reading the PR, making each change,
            running tests, iterating on failures, pushing. The comment is the only way humans can see your progress.

            ## Your task

            Address the review feedback. Read the review comments carefully, make the
            requested changes, and push to the PR branch.

            When done:
              1. Push with: git push origin HEAD
              2. Update your tracking comment with a summary of changes made.
          claude_args: |
            --model claude-opus-4-6
            --permission-mode acceptEdits
            --allowedTools "Bash(gh pr view:*),Bash(gh pr comment:*),Bash(gh issue view:*),Bash(gh api repos/<owner>/<repo>/pulls/*/comments *),Bash(git push origin HEAD)"
            --disallowedTools TodoWrite
```

**Design decisions for the PR workflow:**

| Decision | Reasoning |
|---|---|
| **Auto-trigger on Claude's PRs** | Any review on a PR authored by `claude[bot]` triggers Claude — reviewers don't need to remember `@claude`. |
| **`pull_request_review` only** (not `pull_request_review_comment`) | The review event fires once per submission. Claude reads all inline comments via `gh api`. Triggering on both would cause duplicate runs. |
| **`issue_comment` for top-level PR comments** | People leave general requests as top-level comments (not reviews). These still need `@claude` since they're not formal reviews. |
| **`gh pr checkout` in workflow step** | The PR branch already exists. A workflow step handles checkout so Claude starts on the right branch without needing extra permissions. |
| **No "ask questions" path** | PR reviews are concrete feedback. If a completely different approach is needed, the reviewer should close the PR and comment on the issue instead. |
| **Read linked issues** | Claude's PRs reference the original issue. Reading it gives Claude the full context for why the changes were made. |
| **Per-PR/issue concurrency groups** | Each inline review comment fires a separate event. Without concurrency limits, multiple runs race on the same branch. `cancel-in-progress: true` ensures the latest trigger wins. |
| **`gh api` for inline review comments** | `gh pr view --json reviews` doesn't include inline comments. The `gh api` endpoint with a `--jq` filter gives compact output (path, line, body, user). Permission scoped to `repos/<owner>/<repo>/pulls/*/comments` only. |

### Authentication

Fellows use `claude_code_oauth_token` via their Claude subscriptions (reimbursed by MATS). The built-in `/install-github-app` command in Claude Code handles setup, creating a PR with the workflow file. Most fellows aren't hitting subscription limits, and subscriptions are more cost-effective than API usage.

---

## Observations from Testing

### First Run (tag mode defaults → agent mode, sonnet 4.5)

- `Bash(gh issue comment *)` (space syntax) was denied when the command body contained newlines. **Fix:** Use `Bash(gh issue comment:*)` (colon syntax).
- `Edit` tool was denied because the default permission mode doesn't include file editing. **Fix:** Use `--permission-mode acceptEdits`.
- `gh issue view N --comments` in non-TTY mode didn't reliably include the title. Claude improvised with `--json` which worked. **Fix:** Use `--json title,body,comments,labels` in the prompt.
- Claude tried to use backticks in `gh issue comment` body, which was rejected by Claude Code's shell safety checks. Claude worked around this on its own.
- Default model was sonnet 4.5. **Fix:** Specify `--model claude-opus-4-6` explicitly.
- Shallow checkout (`fetch-depth: 1`) limits git history access. **Fix:** Use `fetch-depth: 0`.
- The execution log was generated and visible in the Actions Summary tab, but less prominent than tag mode's tracking comment.

### Second Run (agent mode, opus 4.6, all fixes applied)

- Ran smoothly end-to-end: read issue, posted tracking comment, implemented changes, pushed branch, created PR, updated tracking comment with summary.
- Significant time spent on `cargo clippy` and `cargo test` due to compilation. **Open item:** Add cargo build caching to the workflow.

### Third Run (PR review workflow testing)

- **`app/claude` vs `claude[bot]`:** The `gh` CLI reports PR author as `app/claude`, but GitHub Actions event payloads use `claude[bot]`. The auto-trigger filter failed silently (job skipped) until this was corrected.
- **Multiple reviews = multiple runs:** Each inline review comment submitted separately fires a `pull_request_review: submitted` event. Three reviews on the same PR spawned three concurrent runs, all racing to push to the same branch. **Fix:** Added `concurrency` groups keyed to the PR/issue number with `cancel-in-progress: true`.
- **`cancel-in-progress: true`:** Chosen over queuing because new review feedback likely supersedes what Claude was already working on. The latest trigger should win.

---

## Open Questions & Future Work

1. **Cargo caching:** Add a caching step (e.g., `actions/cache` or `Swatinem/rust-cache`) to avoid recompiling on every run.
2. ~~**PR trigger support:**~~ **Resolved.** Added `claude-pr.yml` — triggers on `pull_request_review` (auto for Claude's PRs) and `@claude` mentions in PR comments.
3. **Async notification instead of cancel-in-progress:** Currently, a new trigger on the same PR cancels the running job and starts fresh. An alternative: notify the running Claude session of the new feedback mid-stream (similar to steering) so it can incorporate it without restarting. The action doesn't support this natively.
4. **Multi-invocation chaining:** How to support Claude handing off to a new Claude call with a different prompt. Fork vs. `workflow_dispatch` vs. shell loop.
5. **Runpod interaction model:** How to give Claude access to remote GPU compute without risking data loss. Needs a safe abstraction (e.g., MCP tool for spinning up environments and running code, rather than raw SSH).
6. **Long-running experiment completion:** When a remote job finishes after Claude's session has ended, how should results flow back? Current answer: human opens a new issue. Future: remote machine triggers `workflow_dispatch`.
7. **Prompt design iteration:** What the actual base prompt should contain for research tasks. When to stop and surface ambiguities vs. continue autonomously.
8. **Per-task prompt variation:** Whether to use skills/plugins for different task types or a single flexible prompt.
9. **Setup skill for MATS distribution:** A Claude Code skill (e.g., `/setup-async-workflow`) that walks fellows through the setup interactively.

### Per-Fellow Integration Needs

Researchers configure Claude Code tools and settings via `.claude/settings.json` checked into their repos. The workflow YAML should only contain things specific to the GitHub Action context, not general Claude settings.

Known integration needs (configured outside the workflow YAML):
- **wandb** — experiment tracking
- **runpod** — GPU compute (needs careful permission design)
- **HuggingFace** — model access
- **Various LLM APIs** — research-specific

The system should accommodate unknown future needs. Setup steps in the workflow (e.g., `uv` installation, API key secrets) can be added per-project.
