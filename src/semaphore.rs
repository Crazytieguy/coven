//! Counted file-lock semaphores for per-agent concurrency control.
//!
//! Semaphore files live in `<git-common-dir>/coven/semaphores/`.
//! For an agent with `max_concurrency: N`, files `<agent>.0.lock`
//! through `<agent>.<N-1>.lock` are used as exclusive locks.

use std::fs::{self, File, OpenOptions};
use std::path::Path;

use anyhow::{Context, Result};
use fs2::FileExt;

use crate::worker_state;

/// A held semaphore permit. Released when dropped (the `File` lock is
/// released on drop by fs2).
pub struct SemaphorePermit {
    _file: File,
}

impl crate::vcr::Recordable for SemaphorePermit {
    type Recorded = ();

    fn to_recorded(&self) -> Result<()> {
        Ok(())
    }

    fn from_recorded((): ()) -> Result<Self> {
        let file = File::open("/dev/null")?;
        Ok(SemaphorePermit { _file: file })
    }
}

/// Acquire a semaphore permit for the given agent.
///
/// Tries `try_lock_exclusive` on each slot `0..max_concurrency` in sequence.
/// If all slots are locked, sleeps and retries. Retries forever — an automatic
/// timeout could cause two workers to run the same exclusive agent simultaneously.
pub async fn acquire(
    repo_path: &Path,
    agent_name: &str,
    max_concurrency: u32,
) -> Result<SemaphorePermit> {
    let sem_dir = worker_state::coven_dir(repo_path)?.join("semaphores");
    fs::create_dir_all(&sem_dir)
        .with_context(|| format!("failed to create {}", sem_dir.display()))?;

    loop {
        for i in 0..max_concurrency {
            let lock_path = sem_dir.join(format!("{agent_name}.{i}.lock"));
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&lock_path)
                .with_context(|| format!("failed to open {}", lock_path.display()))?;

            match file.try_lock_exclusive() {
                Ok(()) => return Ok(SemaphorePermit { _file: file }),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    return Err(anyhow::anyhow!(e)
                        .context(format!("failed to lock {}", lock_path.display())));
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_repo(dir: &Path) {
        let run = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .output()
                .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"));
        };
        run(&["init"]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);
        fs::write(dir.join("README.md"), "# test\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "init"]);
    }

    #[tokio::test]
    async fn acquire_creates_lock_file() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        let _permit = acquire(repo.path(), "dispatch", 1).await.unwrap();
        let sem_dir = worker_state::coven_dir(repo.path())
            .unwrap()
            .join("semaphores");
        assert!(sem_dir.join("dispatch.0.lock").exists());
    }

    #[tokio::test]
    async fn acquire_two_permits() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        let _permit1 = acquire(repo.path(), "implement", 2).await.unwrap();
        let _permit2 = acquire(repo.path(), "implement", 2).await.unwrap();

        let sem_dir = worker_state::coven_dir(repo.path())
            .unwrap()
            .join("semaphores");
        assert!(sem_dir.join("implement.0.lock").exists());
        assert!(sem_dir.join("implement.1.lock").exists());
    }

    #[tokio::test]
    async fn acquire_blocks_when_full() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        let _permit = acquire(repo.path(), "dispatch", 1).await.unwrap();

        // Try to acquire with a short timeout — should not succeed
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            acquire(repo.path(), "dispatch", 1),
        )
        .await;

        assert!(result.is_err(), "acquire should have timed out");
    }

    #[tokio::test]
    async fn acquire_succeeds_after_drop() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        let permit = acquire(repo.path(), "dispatch", 1).await.unwrap();
        drop(permit);

        // Should succeed immediately
        let _permit2 = acquire(repo.path(), "dispatch", 1).await.unwrap();
    }
}
