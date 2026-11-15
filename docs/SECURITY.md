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

## Approved macOS F3 viewer

Files cross an untrusted parser/display boundary. Text opens are nonblocking on Unix and recheck the opened handle as a regular file. Text reads, index storage, pages and search queries are bounded; generation IDs cancel superseded work. No mmap, dynamic execution or custom parser for PDF/video was introduced. Display controls/bidi overrides are escaped, including helper errors and extracted PDF snippets. Binary classification is explicitly heuristic.

PNG/JPEG decode uses image-rs with codec features restricted, dimension/pixel/decoded-byte checks and bounded encoded-file reads. Native PDF/video parsing is delegated to separately installed tools. Poppler helpers have wall-time/output limits, typed arguments and cancellation/reaping; this is not an OS sandbox or hard memory bound. Source file mutations are detected with best-effort metadata checks, not a transactional filesystem snapshot. Same-size/mtime-preserving changes and races during external helper reads remain possible.

Video is local-file-only, forces lavf with a file protocol whitelist, disables automatic config/scripts/reference/sidecar loading and default key bindings, and uses a fixed private input configuration without screenshot/write actions. Quiet logging avoids forwarding raw malicious filenames to the terminal. MCS stops input/rendering while mpv owns the terminal, then restores the browser through reinitialization. Native codec vulnerabilities and stuck external playback remain residual risks; do not claim general sandbox isolation.

Only generated private playback configuration is written/removed; browsed source files are read-only. No user mpv configuration is changed. Owned Kitty IDs are individually deleted; no global delete-all operation is used. See VIEWER.md for resource budgets and dependency/license review. Cargo audit was checked for availability but is not installed, so no advisory audit passed. Rust check/format/Clippy/tests and simulated process/protocol checks provide local evidence only.


The opening-performance update enables image-rs PNM only for the bounded binary-RGB output of Poppler. Public image decode still rejects formats other than PNG/JPEG; internal transfer applies the same dimension/decoded-byte limits and helper-output caps. Kitty zlib transport is used only after its own positive capability acknowledgement; unconfirmed terminals keep raw transport. No size/time/authorization check was relaxed for performance. Pixel-equivalence with the prior Poppler PNG path is an explicit local check, not a general proof for every document.
