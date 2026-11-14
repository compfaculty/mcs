# Security model

- Normal user process; do not run whole application as root.
- M0 and M1 read-only by default. File destructive actions require separate confirmations and tests.
- Avoid `sh -c` with user-supplied path; pass typed process arguments. Handle symlinks and path changes safely.
- Media/PDF parsing crosses an untrusted-data boundary. Review native dependencies, memory limits, codecs and FFI.
- If adding a helper for network/system configuration, design a narrowly scoped privileged API + auth (polkit/sudo where appropriate) rather than broad root permissions.
- DRM session/input device access should use logind/seatd or documented permissions; never broad /dev permission changes.
- Keep automated agents within repository and sandbox; request approval before OS package installs, driver updates, destructive file operations, or credential access.

## M1 verification boundaries

Runtime navigation uses directory enumeration and metadata APIs only. It does not mutate files, execute subprocesses, request privileges or persist bookmarks. Directory symlinks may point anywhere the invoking user can read; this browser is not a filesystem sandbox. Original `PathBuf`s carry filesystem identity, including non-UTF-8 names where the filesystem supports them; lossy strings are for display/filtering only.

The TUI escapes C0/C1 controls and bidi direction overrides in filenames, paths, filters and filesystem errors. Tests exercise escape sequences, newline/tab and bidi display cases. Scans, channels, workers, history, bookmarks and filter input have explicit bounds; blocked filesystem calls cannot be cancelled and may leave a pane loading until exit. No media parsers, FFI or unsafe code were added. No dependency changes were made. A dependency-advisory audit and separate SAST scan were not run; format/Clippy/tests and focused source review are the local checks, not a general security certification.
