Issue: SessionConfig construction repeated across run.rs, ralph.rs, and worker.rs with similar field patterns. Consider a builder or factory method to reduce duplication.
Status: draft

## Approach

There are 7 SessionConfig construction sites across 3 files. They fall into two categories:

1. **Initial session** (3 sites: run.rs x2, ralph.rs, worker.rs) — sets prompt, extra_args, append_system_prompt, working_dir
2. **Resume after interruption** (3 sites: run.rs, ralph.rs, worker.rs) — same as above plus `resume: Some(session_id)`

Add a `resume_with(&self, prompt, session_id) -> SessionConfig` method on SessionConfig that clones the config and sets the prompt and resume fields. This eliminates the 3 resume construction sites by deriving them from the initial config.

The initial construction sites stay as-is — they have enough variation (run.rs conditionally maps fork_system_prompt, worker.rs takes different input types) that abstracting them would just move complexity around without reducing it.

### Changes

**src/session/runner.rs**: Add method:
```rust
impl SessionConfig {
    pub fn resume_with(&self, prompt: String, session_id: String) -> Self {
        SessionConfig {
            prompt: Some(prompt),
            resume: Some(session_id),
            ..self.clone()
        }
    }
}
```

**src/commands/run.rs** (line ~99): Replace 6-line construction with:
```rust
let session_cfg = session_cfg.resume_with(text, session_id);
```

**src/commands/ralph.rs** (line ~135): Replace 6-line construction with:
```rust
let resume_config = session_config.resume_with(text, session_id);
```

**src/commands/worker.rs** (line ~1006): Replace 8-line construction with:
```rust
let resume_config = session_config.resume_with(text, session_id);
```

Net effect: ~20 lines of repetitive struct construction replaced with 3 one-liners + a 7-line method.

## Questions

### Should we also abstract the initial construction sites?

The 4 initial construction sites have enough variation (conditional fork_system_prompt mapping, different source types for extra_args and working_dir) that a shared constructor would need parameters for everything the struct already has. A builder pattern would add ceremony for little gain on a 6-field struct. The `resume_with` method targets the clearest duplication (resume sites are nearly identical to their corresponding initial sites).

Alternatively, we could add a `SessionConfig::new(prompt, extra_args, working_dir)` that sets the three always-present fields and returns a config with the rest defaulted, then let callers set `append_system_prompt` directly. This would save ~2 lines per site.

Answer:

## Review

