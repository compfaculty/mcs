# Status / handoff

2026-11-14: Initial scaffold authored. Rust toolchain was not available in packaging environment, therefore **no compile or run verification has been performed**. M0 source exists, M0 acceptance pending. The native DRM backend is a deliberate unsupported stub. No image/PDF/video playback exists yet.

First agent action: run checks described in `prompts/01-start.md`, fix concrete incompatibilities, record exact results here. Do not state M0 complete before execution in a real terminal.

## 2026-11-14 — Initial review and M0 baseline verification

- Read repository instructions, kickoff prompt, architecture, milestones, test plan, security model, decisions and deployment docs.
- `git status --short`: unavailable; this directory has no Git repository metadata. A committed baseline/diff cannot be established.
- Initial `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` passed (two unit tests; no integration or rendering tests).
- Initial `cargo fmt --all -- --check` failed. Applied `cargo fmt --all` to the four Rust source files; no intentional behavior changes or dependency changes.
- Re-ran all four required checks after formatting: all passed.
- `cargo run -p mcs -- --backend tui`: launched in the tool-provided 80x24 PTY. Exercised Tab, Enter, Backspace, refresh and q; exit code 0. Captured alternate-screen exit and cursor-show sequences. This is a PTY smoke check, not full visual acceptance in Terminal.app/Kitty or a verification of restored termios state.
- `cargo run -p mcs -- --backend native`: expected unsupported message and exit code 1; no native graphics implementation.

Review findings to address within the agreed MVP scope:

- Directory symlinks are classified as non-directories by `DirEntry::file_type`, so Enter does not follow them.
- Directory enumeration and sorting run synchronously on the input/render thread; responsiveness for massive or slow directories is unverified.
- CLI parsing silently falls back to TUI for a missing backend value or unknown options.
- Only two narrow unit tests exist; navigation failure paths, rendering, resize and terminal restoration need acceptance coverage.

MVP feature scope, target platform acceptance and read-only versus confirmed file operations are awaiting user clarification. No M1 features started. Linux, SSH, real-terminal resize/restoration, hostile filename rendering and security scanners have not been tested. Media, DRM and system configuration remain roadmap features.

## 2026-11-14 — Recommended M1 read-only MVP implemented

User approved the recommended M1 scope: macOS local verification, Linux TUI/SSH compatibility retained, read-only operations. This supersedes the pending-scope note above. No M2 or Linux native work started.

Changes by file:

- `crates/mcs-core/src/lib.rs`: bounded asynchronous directory scans (one worker/pane), atomic navigation/history, path-based highlight restoration, marking, name sort/filter, hidden-file toggle, directory/broken symlink handling, session bookmarks and resource limits. Oversized listings fail explicitly rather than silently truncating.
- `crates/mcs-tui/src/lib.rs`: modal filter/bookmark input, sidebar and status/help, paging and mark controls, sanitized untrusted display strings, change-driven rendering and fallible terminal setup/restore with drop fallback. No graphics/native imports.
- `apps/mcs/src/main.rs`: strict CLI parsing, help and rejection of missing/unknown/duplicate/non-UTF-8 options before terminal initialization.
- `crates/mcs-core/tests/navigation.rs`: ten macOS filesystem integration tests plus a Linux-only non-UTF-8 filename test. Fixtures are uniquely created and retained in the system temp directory; tests do not delete or overwrite existing user files.
- `scripts/check_tui_pty.py`: repeatable Python standard-library executable/PTY checks; no runtime dependency for MCS.
- `README.md`, `docs/ARCHITECTURE.md`, `docs/DECISIONS.md`, `docs/MILESTONES.md`, `docs/SECURITY.md`, `docs/TEST_PLAN.md`, `docs/STATUS.md`: controls, decisions, bounds, evidence and outstanding acceptance.

Final local Rust gates:

- `cargo check --workspace`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, 21 tests on macOS (3 CLI, 3 core, 10 filesystem integration, 5 TUI); no failures. TUI assertions cover 80x24, 100x30 and tiny terminal buffers, not screenshot comparisons.
- `cargo build -p mcs`: passed.
- `python3 -u scripts/check_tui_pty.py` (final harness): passed CLI/help/native checks and all three q/F10/Ctrl-C exit cases. Each case exercised input/sidebar/filter and 80x24 -> 100x30 resize, then verified termios equality, alternate-screen exit and cursor restoration.
- `cargo test -p mcs-core --test navigation large_directory -- --nocapture`: passed. One local debug-build sample measured 20,000-entry scan/commit at 17.088125 ms, excluding fixture creation. This is an observation, not a before/after benchmark or latency guarantee.
- `cargo run -p mcs -- --backend tui` in a tool-provided macOS PTY: exercised pane switching, filtering and bookmarks; q returned exit 0, with `TERMINAL_MODES_RESTORED exit=0` from comparing shell `stty -g` before/after.

Intermediate checks found and fixed a Clippy octal-permission literal and test compilation errors. A non-UTF-8 filesystem fixture failed on macOS; it is now Linux-only, while Unix CLI non-UTF-8 rejection passes locally. The first PTY harness expected contiguous text despite cursor-position escapes; corrected. An early controlling-terminal harness attempt stalled and was stopped; the harness now uses `start_new_session` without a Python pre-exec hook and bounds cleanup waits.

Security/source review: runtime changes use read-only filesystem APIs, no shell/subprocess execution, new dependencies, unsafe code or privilege changes. Control/bidi display injection, permission failures, stale history, broken symlinks, request/history/bookmark/filter bounds and listing-limit rejection have focused tests. Diff review used pre-edit source copies because `git status --short` still reports no Git metadata. Separate advisory/SAST scanners were not run; no dependency changes were made.

Limitations and next acceptance: Linux/SSH, Terminal.app/Kitty visual acceptance, SSH disconnect, fatal-signal/crash restoration and actual hung-mount behavior remain unverified. Ordinary filesystem reads cannot be cancelled; a hung scan leaves its pane loading until exit, and navigation while loading is ignored. Bookmarks/history are session-only; the listing cap is 100,000 entries. No file operations, media, PTY child shell, system configuration or DRM were added. Next action is manual terminal-emulator and Linux/SSH acceptance of M1 before approving another milestone.

## 2026-11-14 — Version control

User accepted the first MVP and requested a commit. Initialized a local Git repository on `main` because the supplied directory had no Git metadata. The initial commit includes the scaffold and verified M1 implementation; build output, IDE settings and `.env` remain ignored. No remote or push requested. Additional feature scope awaits the user's next instructions.
