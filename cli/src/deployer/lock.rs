use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};

#[derive(Debug)]
pub(crate) struct DeployLockGuard {
    path: PathBuf,
}

impl DeployLockGuard {
    pub(crate) fn acquire(root: &Path, color: ColorMode) -> Result<Self> {
        let path = root.join(".evm-cloud-deploy.lock");
        let created = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);

        match created {
            Ok(mut file) => {
                let _ = write!(
                    file,
                    "{{\"pid\":{},\"started_at\":{}}}",
                    std::process::id(),
                    lock_timestamp(),
                );
                Ok(Self { path })
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                // Check if the lock holder is still alive.
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Some(stale_pid) = parse_lock_pid(&contents) {
                        if !is_process_alive(stale_pid) {
                            output::warn(
                                &format!(
                                    "Stale lock from PID {} (not running). Auto-recovering.",
                                    stale_pid
                                ),
                                color,
                            );
                            let _ = fs::remove_file(&path);
                            // Retry once.
                            let mut file = std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&path)
                                .map_err(|source| CliError::Io {
                                    source,
                                    path: path.clone(),
                                })?;
                            let _ = write!(
                                file,
                                "{{\"pid\":{},\"started_at\":{}}}",
                                std::process::id(),
                                lock_timestamp(),
                            );
                            return Ok(Self { path });
                        }
                    }
                }
                Err(CliError::DeployLockBusy { path })
            }
            Err(source) => Err(CliError::Io {
                source,
                path,
            }),
        }
    }
}

impl Drop for DeployLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_lock_pid(contents: &str) -> Option<u32> {
    // Parse {"pid":12345,...} — simple extraction without full JSON parse.
    let start = contents.find("\"pid\":")?;
    let after_key = &contents[start + 6..];
    let trimmed = after_key.trim_start();
    let end = trimmed.find(|c: char| !c.is_ascii_digit())?;
    trimmed[..end].parse().ok()
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    // kill(pid, 0) checks existence without sending a signal.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    // On non-Unix, assume alive (conservative — won't auto-recover).
    true
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{DeployLockGuard, parse_lock_pid, is_process_alive};
    use crate::error::CliError;
    use crate::output::ColorMode;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-cli-tests-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    #[test]
    fn lock_guard_blocks_concurrent_acquisition() {
        let dir = temp_dir("deploy-lock");
        let first = DeployLockGuard::acquire(&dir, ColorMode::Never).expect("first lock must succeed");
        let second = DeployLockGuard::acquire(&dir, ColorMode::Never).expect_err("second lock must fail");

        match second {
            CliError::DeployLockBusy { .. } => {}
            other => panic!("unexpected error: {other}"),
        }

        drop(first);
        let third = DeployLockGuard::acquire(&dir, ColorMode::Never);
        assert!(third.is_ok());
    }

    #[test]
    fn lock_writes_pid_and_recovers_stale() {
        let dir = temp_dir("deploy-lock-pid");
        let guard = DeployLockGuard::acquire(&dir, ColorMode::Never).expect("acquire lock");
        let lock_path = dir.join(".evm-cloud-deploy.lock");
        let contents = fs::read_to_string(&lock_path).expect("read lock");
        let pid = parse_lock_pid(&contents).expect("parse pid from lock");
        assert_eq!(pid, std::process::id());
        drop(guard);
    }

    #[test]
    fn lock_recovers_dead_pid() {
        let dir = temp_dir("deploy-lock-stale");
        let lock_path = dir.join(".evm-cloud-deploy.lock");
        // Write a lock with a PID that almost certainly doesn't exist.
        fs::write(&lock_path, "{\"pid\":999999999,\"started_at\":0}").expect("write stale lock");
        // Should auto-recover (PID 999999999 is not alive).
        let guard = DeployLockGuard::acquire(&dir, ColorMode::Never);
        assert!(guard.is_ok(), "should recover stale lock from dead PID");
    }

    #[test]
    fn parse_lock_pid_extracts_correctly() {
        assert_eq!(parse_lock_pid("{\"pid\":12345,\"started_at\":0}"), Some(12345));
        assert_eq!(parse_lock_pid("{\"pid\":1,\"started_at\":0}"), Some(1));
        assert_eq!(parse_lock_pid("garbage"), None);
        assert_eq!(parse_lock_pid(""), None);
    }

    #[test]
    fn current_process_is_alive() {
        assert!(is_process_alive(std::process::id()));
    }
}
