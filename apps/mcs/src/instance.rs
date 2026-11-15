//! Per-user, host-wide singleton acquired before parsing CLI arguments or touching a TTY.
use std::{fs::File, io};

pub const EMBEDDED_SHELL: &str = "MCS_EMBEDDED_SHELL";

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn acquire() -> io::Result<Option<File>> {
    // Fixed host path: neither cwd, TMPDIR, HOME nor executable location can split the lock.
    let path = std::path::PathBuf::from(format!("/tmp/mcs-{}.lock", nix::unistd::geteuid()));
    lock_at(&path)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn lock_at(path: &std::path::Path) -> io::Result<Option<File>> {
    use std::{
        fs::{OpenOptions, TryLockError},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    // Never truncate, follow a symlink, or block opening an attacker-supplied special file.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.nlink() != 1
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe MCS instance lock",
        ));
    }
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
    // Keep this inode on disk: unlinking a lock permits another process to lock a new inode.
    // The owned File closes on exec and releases its lock on normal exit or process death.
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn acquire() -> io::Result<Option<File>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "MCS single-instance guard unavailable on this platform",
    ))
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::{symlink, PermissionsExt},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    fn fixture() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "mcs-instance-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn lock_is_exclusive_and_reusable_without_unlinking() {
        let path = fixture().join("instance.lock");
        let first = lock_at(&path).unwrap().unwrap();
        assert!(lock_at(&path).unwrap().is_none());
        drop(first);
        let next = lock_at(&path).unwrap().unwrap();
        assert!(lock_at(&path).unwrap().is_none());
        drop(next);
        assert!(path.is_file());
    }

    #[test]
    fn unsafe_lock_paths_are_rejected_without_truncation_or_blocking() {
        let dir = fixture();
        let target = dir.join("target");
        std::fs::write(&target, b"unchanged").unwrap();
        let link = dir.join("link");
        symlink(&target, &link).unwrap();
        assert!(lock_at(&link).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");
        assert!(lock_at(&dir).is_err());
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert!(lock_at(&target).is_err());
        let hardlink = dir.join("hardlink");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::hard_link(&target, &hardlink).unwrap();
        assert!(lock_at(&hardlink).is_err());
        let fifo = dir.join("fifo");
        nix::unistd::mkfifo(
            &fifo,
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        assert!(lock_at(&fifo).is_err());
    }
}
