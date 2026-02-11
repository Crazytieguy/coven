Issue: [P2] Windows support: uses Unix-specific APIs (libc::tcflush, /dev/null, kill command, rsync). Need platform abstractions or #[cfg] guards to support x86_64-pc-windows-msvc target.
Status: draft

## Approach

Drop Windows support for now and document it explicitly.

### Changes

1. **README.md** — Add a "Platform support" section noting that coven currently only supports macOS and Linux. Mention the Unix-specific dependencies (libc::tcflush, /dev/null, kill, rsync) as the reason.

2. **Cargo.toml** — No changes needed. The existing `libc` dependency already implicitly limits to Unix. Optionally, could add a build-time check, but documentation is sufficient.

### Rationale

The effort to abstract four Unix-specific call sites isn't justified by current demand. Documenting the limitation is honest and low-cost. Windows support can be revisited if there's user interest.

## Questions

## Review


