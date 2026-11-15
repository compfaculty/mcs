# Milestones and acceptance criteria

Each milestone requires evidence in STATUS.md. Proceed only on user approval.

## M0 — Project setup / running TUI (macOS tool-PTY verification recorded)
- [x] Build/check/clippy/test on macOS
- [x] Launch from macOS tool-provided terminal with `cargo run -p mcs -- --backend tui`
- [x] Two read-only panes, navigation, clear error messages, clean exit to original tool terminal
- [x] Explicit native backend unavailable error; no hidden native behavior

## M1 — Reliable MC-like file navigation (macOS/Linux, no native graphics)
- [x] Sidebar/history, selection, sorting/filtering, hidden files, session bookmarks
- [x] Bounded asynchronous scans; 20,000-entry macOS fixture, symlinks and permission failure tests passed
- [x] Read-only MVP approved; copy/move/delete excluded until a separate confirmation specification and tests
- [x] Unit + integration + terminal UI snapshot assertions; macOS PTY resize/restore checks
- Linux/SSH acceptance moved to the long-term backlog; macOS viewer acceptance below.

## M2 — Approved macOS/Kitty F3 viewer (incremental checkpoints)

### A — Large-file text
- [x] F3/v fullscreen viewer, return to browser state, paging, wrap/numbers, reload
- [x] Bounded seek-based multi-gigabyte reads, sparse indexing and cancellable literal search
- [x] UTF-8/control handling, mutation/cancellation/resource tests, text PTY restoration

### B — PNG/JPEG and searchable PDF
- [x] Bounded Kitty capability query with text-only fallback; image decode/encoding off UI thread
- [x] Fit/zoom/pan and resize, owned-ID cleanup, bounded image inputs/viewports
- [x] Optional Poppler page rendering/search, output limits, timeout and cancellation
- [x] Simulated Kitty and fake Poppler lifecycle/cleanup checks
- [ ] Real Kitty image/PDF visual acceptance with installed Poppler

### C — Terminal mpv video handoff
- [x] Verify Kitty driver; controlled local-only arguments/input config and suspend/resume
- [x] Fake-player success/failure and terminal restoration checks
- [ ] Real Kitty/mpv playback and 60-second 720p/30 measurements (dimensions, drops, sync, CPU/memory)

No automatic OS package installation. No media-performance claim before live acceptance. See VIEWER.md and STATUS.md.

## M3 — Ubuntu Server local DRM/KMS research spike (physical Linux laptop)
- [ ] Confirm hardware/seat, active VT, `/dev/dri`, logind/seatd, evdev/libinput, permissions
- [ ] Acquire/release display safely; 1 basic drawn frame; VT suspend/resume; crash restoration
- [ ] Image + text in same DRM-owned output; optional video via libmpv render API or decode/output bridge
- [ ] No root-required normal user workflow; fail gracefully; no display manager
- [ ] Multi-monitor and GPU compatibility documented; safe rollback to TTY

## M4 — Native media viewer
- [ ] Embedded image/PDF/video viewports; fullscreen, zoom, seeking, subtitle and audio controls
- [ ] Deterministic input/focus and repaint; 60Hz UI target when supported; measure rather than assume
- [ ] Clean resource release on exit, ctrl-c and VT switch; bug regression suite

## M5 — Embedded PTY and system center
- [ ] PTY child lifecycle, resize, Ctrl-C/signals, tab/pane focus
- [ ] Read-only service/network/storage panels; Linux implementations via stable APIs
- [ ] Privileged changes mediated via explicit authorization; no blanket sudo

## M6 — Packaging, Linux installer, remote operation
- [ ] Reproducible Linux build for target architectures; dependency inventory
- [ ] Optional .deb and signed release artifacts; no automatic login shell override
- [ ] SSH TUI still usable without GPU; local DRM enabled only when supported
- [ ] Crash recovery, support matrix, manual, release tests

## Out of scope for initial releases
- Entire GUI application compatibility, browsers with full JS, Office editor, system-wide window manager, kernel/driver rewriting.

## Approved F4–F8 file operations — 2026-11-15

User explicitly requested implementation, superseding the earlier M1 exclusion for this scope.

- [x] F4 bounded UTF-8 editor and confirmed save, with conflict detection and buffer recovery.
- [x] F5/F6 marked-entry or selection copy/move; exclusive destinations, recursive copy, atomic same-filesystem move.
- [x] F7 mkdir and F8 confirmed permanent recursive deletion.
- [x] Descriptor-relative path safety, child symlink non-traversal, resource limits and failure reporting.
- [x] Focused Rust regression tests, real-binary F4–F8 PTY fixture and browser/media PTY regression checks on macOS.
- [ ] Linux execution and live Kitty visual acceptance; adversarial concurrent-writer and disk-full recovery checks.

See FILE_OPERATIONS.md and STATUS.md for explicit limits and actual evidence. No expansion into DRM, system configuration or privileged services.

## Approved Ctrl-T pane-shell increment — 2026-11-15

User authorized the embedded-terminal portion of M5 independently of system panels. Ctrl-T toggles independent persistent pane sessions; PTY resize, child input/interrupt/exit and browser restoration are implemented. Read-only system panels and privileged operations remain unimplemented. Acceptance evidence is recorded in STATUS.md; Linux runtime acceptance remains pending.
