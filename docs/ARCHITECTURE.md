# Architecture v0.1

Goal: user-initiated, keyboard-first, MC-inspired file/terminal/system environment; local visual media without desktop environment in later Linux phase; optional SSH text mode. Not a GNOME/KDE replacement and not a general desktop window server.

## Layers

1. `mcs-core`: navigation, file models, actions, capabilities; no TUI/DRM imports.
2. `mcs-tui`: text/ANSI renderer + keyboard via Ratatui/Crossterm. Works on macOS, Linux virtual console, SSH.
3. `mcs-viewer`: paged text, sparse line index, asynchronous routing/search state; no terminal imports. `mcs-media`: bounded PNG/JPEG decode, external Poppler adapters and prepared mpv handoff.
4. `mcs-tui` graphics worker: confirmed Kitty output using ratatui-image, independent of media decoding. No graphics protocol is assumed on Linux virtual console.
5. `mcs-native`: currently explicit Unsupported stub. Future DRM/KMS backend on Linux physical seat, independently owns display output and renders both TUI-like cell grid and media regions.
6. Future `mcs-system`: interface + Linux-specific implementations of services/network/storage, read-only first; macOS capabilities differ.

## Interfaces to design before M2/M3

- `CapabilitySet`: text colors, images, animations, direct-scanout, hardware decoding, clipboard, mouse, terminal PTY, system services.
- `UiModel`: panel tree, focused widget, navigation, actions and viewer state; no platform handles.
- `Frame`: text cells + optional image/media viewport objects (coordinates, clip, scale, z-order), timestamps for video.
- `DisplayBackend`: `probe`, `enter`, `render`, `suspend`, `resume`, `leave`; ownership and rollback explicit.
- `MediaEngine`: open/seek/pause/frame events; audio sync and decoding independent of renderer.

DRM/KMS sets modes and scans out buffers; does not decode video, manage input, draw text, or provide windows automatically. On Linux, use session/seat management (logind or seatd), libinput/evdev, rendering, audio separately. VT switch and restoration are release blockers.

## UX

MC-style dual panes + optional right preview, F3 viewer, file type routing, full-screen viewer (Esc to prior pane/selection), pane-local embedded terminal via PTY (Ctrl-T), settings accessible but not privileged by default.

## Important truth-in-advertising

- Kitty/WezTerm are desktop terminal emulators on macOS; they are for rapid prototyping, not substitutes for Linux DRM on headless GUI-free console.
- Normal SSH cannot transport DRM pixels without a separate streaming/remoting protocol; TUI can run over SSH.
- Supporting arbitrary external GUI apps is out of scope.
- Video inside same DRM surface requires compositor-like drawing within **our application**, not starting two competing direct DRM programs.
- PDF rendering and video handoff are now optional macOS viewer adapters; real Kitty media acceptance is pending.

## M1 read-only navigation implementation

`mcs-core` owns both pane models, their asynchronous directory loaders, path-based history/marks, view filters and session bookmarks. Each pane owns a single worker with bounded request/result channels and at most one pending scan. Successful results commit path/listing/history together; failed navigation never consumes history. Directory grouping and cached name keys are sorted in the worker; visible indices are rebuilt on the UI thread without re-sorting. No platform or rendering dependency enters core.

`mcs-tui` owns filter/sidebar input modes and display sanitization. It renders only visible rows, redraws on changes, and restores terminal modes on normal/error returns with a drop fallback. Filter input cannot invoke browser commands. The native backend remains an unsupported stub.

Resource limits: 100,000 entries/scan, two workers/application, 128 back and forward locations per pane, 32 shared session bookmarks and 256 filter characters through the UI. Enumeration/sort is O(n log n) time/O(n) space; view filtering is O(n). Mark reconciliation on refresh uses O(n log n) time when marks exist. Filesystem reads are not cancellable: workers are detached and not joined on exit, so a blocked mount does not block application shutdown. No background daemon or persistent state is created.

## Approved macOS viewer milestone

F3/v switches an explicit screen enum between Browser, Loading, Text, Media, Playback and Error; browser state remains owned by the unchanged core. Search input and view preferences belong to the viewer. One persistent text/routing worker and one lazily created graphics worker use bounded channels and generation IDs. Superseded/closed requests cannot apply stale results. Indexing runs in bounded idle steps; search and helper operations check cancellation between chunks/polls. Ordinary filesystem reads remain non-interruptible.

Only the main thread reads events or writes terminal output. It probes Kitty before event processing, draws pre-encoded images, deletes owned image IDs, and coordinates fullscreen playback suspend/resume. mpv alone owns the terminal during playback. No default GUI fallback, native renderer or OS installation is introduced. See VIEWER.md for exact limits, controls, dependencies and acceptance boundaries.


## Confirmed file operations

`mcs-core::operations` owns descriptor-relative captured plans and bounded recursive filesystem work. `mcs-tui::operations` owns destination/review/editor dialogs and one background worker. No rendering dependency enters core. See FILE_OPERATIONS.md for contracts, security boundaries and limits.

## Ctrl-T pane shells

`mcs-tui::shell` owns two lazy PTYs, bounded I/O channels/workers and VT screen parsers. Browser models remain unchanged while shells are visible; Ctrl-T swaps presentation without killing children. Input is routed before browser shortcuts; Ctrl-C reaches the child line discipline. Only parsed cells reach Ratatui; no raw child escapes are forwarded. See SHELL.md for lifecycle, resource budgets and limits.

## Process singleton

The executable acquires a nonblocking exclusive OS file lock before argument parsing or backend startup. One owner per effective user across terminals, working directories and executable paths; duplicates return silently. A fixed `/tmp/mcs-<euid>.lock` inode remains on disk; the lock lifetime is the owning file descriptor/process, with close-on-exec to avoid child inheritance. Embedded shells carry an explicit nested-launch ban. Guard failures fail closed before terminal setup. No PID scan, PID file, daemon or privilege escalation.
