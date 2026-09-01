//! Cross-process serialization of HID transactions.
//!
//! Several lamzuctl processes (CLI invocations, the GUI, the Stream Deck
//! plugin, status-bar widgets polling the battery) may talk to the same
//! device at once. The device keeps a single feature-report response buffer,
//! so a read that lands between another process's send and receive picks up
//! that process's response — or zeroed garbage while a write is being
//! applied. An advisory file lock held around each send/receive round trip
//! serializes the transactions across processes.
//!
//! The lock is best-effort: if the lock file cannot be created (unwritable
//! directory, another user's file in a shared temp dir) we degrade to the
//! old unsynchronized behavior rather than refusing to run.

use std::fs::{File, OpenOptions, TryLockError};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;

/// How long to wait for another process to release the device.
///
/// A transaction holds the lock for tens of milliseconds, so hitting this
/// timeout means the holder is wedged (the OS releases the lock automatically
/// if the holder merely dies).
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// How often to re-attempt a contended lock.
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// A cross-process lock for one physical device.
///
/// Created once per connection; locked once per HID transaction via
/// [`DeviceLock::transaction`].
pub(crate) struct DeviceLock {
    file: Option<File>,
}

impl DeviceLock {
    /// Create the lock for the device at `device_path`.
    ///
    /// Never fails: when the lock file cannot be opened the lock degrades to
    /// a no-op, since it is only advisory.
    pub(crate) fn for_device(device_path: &str) -> Self {
        Self {
            file: open_lock_file(device_path).ok(),
        }
    }

    /// Take the lock for the duration of the returned guard.
    ///
    /// Fails only when another process holds the lock for longer than
    /// [`LOCK_TIMEOUT`].
    pub(crate) fn transaction(&self) -> Result<TransactionGuard<'_>> {
        let Some(file) = &self.file else {
            return Ok(TransactionGuard { file: None });
        };

        let deadline = Instant::now() + LOCK_TIMEOUT;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(TransactionGuard { file: Some(file) }),
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        anyhow::bail!(
                            "timed out waiting for another process to release the device \
                             (lock held for more than {LOCK_TIMEOUT:?})"
                        );
                    }
                    std::thread::sleep(LOCK_RETRY_INTERVAL);
                }
                // Locking is not supported here (unusual filesystem):
                // degrade to unsynchronized access.
                Err(TryLockError::Error(_)) => return Ok(TransactionGuard { file: None }),
            }
        }
    }
}

/// Releases the lock when dropped.
pub(crate) struct TransactionGuard<'a> {
    file: Option<&'a File>,
}

impl Drop for TransactionGuard<'_> {
    fn drop(&mut self) {
        if let Some(file) = self.file {
            let _ = file.unlock();
        }
    }
}

/// Directory for lock files: the user's runtime dir where available,
/// otherwise the system temp dir.
fn lock_dir() -> PathBuf {
    #[cfg(unix)]
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return path;
        }
    }
    std::env::temp_dir()
}

fn open_lock_file(device_path: &str) -> std::io::Result<File> {
    // DefaultHasher::new() uses fixed keys, so the name is stable across
    // processes for the same device path.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    device_path.hash(&mut hasher);
    let name = format!("lamzuctl-{:016x}.lock", hasher.finish());

    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_dir().join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_and_release() {
        let lock = DeviceLock::for_device("/dev/test-lamzuctl-lock");
        {
            let _guard = lock.transaction().unwrap();
            // A second lock in the same process on the same File handle
            // would succeed anyway (file locks are per-process), so this
            // only exercises the lock/unlock path.
        }
        let _guard = lock.transaction().unwrap();
    }
}
