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
- [ ] Linux TUI/SSH and Terminal.app/Kitty manual acceptance (not available in this session)

## M2 — Visual media proof-of-concept on macOS (Kitty)
- [ ] Terminal capability detect; ordinary TUI fallback
- [ ] PNG/JPEG image inside right preview, resize/scroll cleanup
- [ ] PDF via external library (PDFium/Poppler evaluate license/packaging) in viewer
- [ ] `libmpv` feasibility spike: may require an OS-native media surface rather than in-terminal pixels; measure frame rate, audio sync, CPU use
- [ ] Honest report whether Kitty graphics meets video requirements; do not promise 1080p/60Hz via escape sequences

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
