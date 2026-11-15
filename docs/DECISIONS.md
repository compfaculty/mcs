# Architecture decisions (ADRs)

ADR-001 Accepted: launch `mcs` on demand, exit to invoking shell. No OS replacement.
ADR-002 Accepted: Rust workspace with cross-platform core and TUI; Linux native renderer behind explicit stub.
ADR-003 Accepted: simple MC-inspired UX, keyboard-first; visuals only for media/documents.
ADR-004 Accepted: text-only mode works over normal SSH and Linux TTY; no implicit graphic protocol.
ADR-005 Proposed: Kitty graphical protocol for macOS rapid media proof-of-concept; benchmark video before committing.
ADR-006 Proposed: local Linux DRM/KMS via seat/logind and renderer; evaluate Smithay ecosystem or smaller lower-level components after spike.
ADR-007 Proposed: PDFium/Poppler evaluation; use libmpv for video rather than writing decoder.
ADR-008 Accepted: RustRover on macOS, Codex CLI in terminal; Git-based push/pull Linux verification.
ADR-009 Accepted: do not develop a new compositor, universal GUI toolkit, or fully featured browser initially.

ADR-010 Accepted (M1): read-only navigation MVP. Keep bookmarks/marks/history in session memory; no persistent writes or file operations. Use one bounded directory worker per pane rather than spawning per request. Ignore additional navigation while scanning; commit successful navigation atomically and preserve prior state on failure. Cap listings at 100,000 entries; reject oversized scans explicitly. Standard filesystem calls cannot be cancelled; keep exit independent of joining workers. This retains the core/TUI split and adds no dependencies.

ADR-011 Accepted (approved macOS viewer plan): prioritize local Kitty F3 text, images, searchable PDF and fullscreen mpv handoff in incremental checkpoints; move Linux/SSH acceptance to BACKLOG.md. Add mcs-viewer (bounded paged text/routing, no terminal imports) and mcs-media (image-rs and optional external Poppler/mpv); prepare Kitty encodings in the TUI worker. Avoid PDF/video implementations and native FFI. Support large text files now with bounded seek-based reads and sparse indexing; PDF search navigates to matching pages/snippets without OCR. Optional tools require separate installation authorization; real media/performance acceptance cannot be inferred from fake helpers or PTYs. Rollback is to retain the text/browser frontend and report unavailable media without a GUI fallback.
