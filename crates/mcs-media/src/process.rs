//! Bounded optional-helper execution. Never invokes a shell.
use std::{
    ffi::{OsStr, OsString},
    io::{self, Read},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

fn reader(
    mut pipe: impl Read + Send + 'static,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.by_ref()
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            overflow.store(true, Ordering::Relaxed);
            return Err(io::Error::other("Helper output limit exceeded"));
        }
        Ok(bytes)
    })
}

pub fn capture(
    program: &OsStr,
    args: &[OsString],
    limit: usize,
    timeout: Duration,
    token: &AtomicU64,
    generation: u64,
) -> io::Result<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "{} unavailable: {error}. Install the optional tool separately.",
                    program.to_string_lossy()
                ),
            )
        })?;
    let overflow = Arc::new(AtomicBool::new(false));
    let out = reader(
        child.stdout.take().expect("piped stdout"),
        limit,
        overflow.clone(),
    );
    let err = reader(
        child.stderr.take().expect("piped stderr"),
        64 * 1024,
        overflow.clone(),
    );
    let start = Instant::now();
    let status = loop {
        let failure = if token.load(Ordering::Relaxed) != generation {
            Some("Cancelled")
        } else if start.elapsed() >= timeout {
            Some("Helper timed out")
        } else if overflow.load(Ordering::Relaxed) {
            Some("Helper output limit exceeded")
        } else {
            None
        };
        if let Some(message) = failure {
            let _ = child.kill();
            let _ = child.wait();
            break Err(io::Error::other(message));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error);
            }
        }
    };
    let stdout = out
        .join()
        .map_err(|_| io::Error::other("Helper stdout reader stopped"))?;
    let stderr = err
        .join()
        .map_err(|_| io::Error::other("Helper stderr reader stopped"))?;
    let status = status?;
    let stdout = stdout?;
    let stderr = stderr?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "{} failed: {}",
            program.to_string_lossy(),
            String::from_utf8_lossy(&stderr)
        )));
    }
    Ok(stdout)
}
