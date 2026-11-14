use std::io;

/// Explicit stub: no attempt to acquire seat, TTY or DRM device.
pub fn run() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "DRM/KMS backend is intentionally unimplemented; see docs/MILESTONES.md M3",
    ))
}
