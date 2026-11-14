# Architecture v0.1

Goal: user-initiated, keyboard-first, MC-inspired file/terminal/system environment; local visual media without desktop environment in later Linux phase; optional SSH text mode. Not a GNOME/KDE replacement and not a general desktop window server.

## Layers

1. `mcs-core`: navigation, file models, actions, capabilities; no TUI/DRM imports.
2. `mcs-tui`: text/ANSI renderer + keyboard via Ratatui/Crossterm. Works on macOS, Linux virtual console, SSH.
3. Future `mcs-media`: image/PDF/video decoders and frame/time model (decoders independent of output).
4. Future `mcs-terminal-graphics`: Kitty graphics / Sixel adapters where supported, not assumed in Linux virtual console.
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

MC-style dual panes + optional right preview, F3 viewer, file type routing, full-screen viewer (Esc to prior pane/selection), embedded terminal eventually as PTY, settings accessible but not privileged by default.

## Important truth-in-advertising

- Kitty/WezTerm are desktop terminal emulators on macOS; they are for rapid prototyping, not substitutes for Linux DRM on headless GUI-free console.
- Normal SSH cannot transport DRM pixels without a separate streaming/remoting protocol; TUI can run over SSH.
- Supporting arbitrary external GUI apps is out of scope.
- Video inside same DRM surface requires compositor-like drawing within **our application**, not starting two competing direct DRM programs.
- PDF rendering and video playback are not part of this starter code.

## M1 read-only navigation implementation

`mcs-core` owns both pane models, their asynchronous directory loaders, path-based history/marks, view filters and session bookmarks. Each pane owns a single worker with bounded request/result channels and at most one pending scan. Successful results commit path/listing/history together; failed navigation never consumes history. Directory grouping and cached name keys are sorted in the worker; visible indices are rebuilt on the UI thread without re-sorting. No platform or rendering dependency enters core.

`mcs-tui` owns filter/sidebar input modes and display sanitization. It renders only visible rows, redraws on changes, and restores terminal modes on normal/error returns with a drop fallback. Filter input cannot invoke browser commands. The native backend remains an unsupported stub.

Resource limits: 100,000 entries/scan, two workers/application, 128 back and forward locations per pane, 32 shared session bookmarks and 256 filter characters through the UI. Enumeration/sort is O(n log n) time/O(n) space; view filtering is O(n). Mark reconciliation on refresh uses O(n log n) time when marks exist. Filesystem reads are not cancellable: workers are detached and not joined on exit, so a blocked mount does not block application shutdown. No background daemon or persistent state is created.
