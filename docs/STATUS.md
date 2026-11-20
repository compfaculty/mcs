# Status / handoff

## 2026-11-15 — Project folder rename to mcs

The project root is now `/Users/alexg/projects/mcs` (formerly `MCS_Starter_Kit`). Preserved the existing installation-documentation change and completed local RustRover configuration repair: renamed `.idea/MCS_Starter_Kit.iml` to `.idea/mcs.iml`, updated `modules.xml`, and changed the last-opened path plus the selected/named Cargo test configuration in `workspace.xml`. Module content/source/test/exclusion paths now use `$PROJECT_DIR$`, because `$MODULE_DIR$` resolved inside `.idea`. These IDE settings are ignored by Git; no application source or dependencies changed. Reopen the `mcs` folder in RustRover to reload the settings.

The first workspace test run exposed a cached test executable containing the former absolute `CARGO_MANIFEST_DIR`; `entry_limit_rejects_partial_listing` failed against the missing old crate directory. Confirmed the old path in the executable with `strings`. Ran `cargo clean` for the six workspace packages in both debug and release profiles, then rebuilt. Final `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` passed (66 tests, two optional Poppler experiments ignored). `cargo build -p mcs` and `cargo build --release -p mcs` passed.

Parsed local IDE XML and verified every module/content/source/test/exclusion target exists. Searched tracked and ignored project files outside build artifacts/Git history; the former name remains only in this explanatory status entry. `git diff --check` passed. `python3 -u scripts/check_tui_pty.py` was attempted but stopped at its first CLI assertion because the active MCS instance causes duplicate launches to return silently with status 0. An independent nonblocking lock probe confirmed `ACTIVE_INSTANCE_LOCK`; no process was stopped. Manual terminal restoration was not rerun. Restart the running MCS process to use the rebuilt binary. No separate security scan was warranted for local path/settings changes; XML/path validation and diff review were performed. No commit/push or global IDE-preference changes.

## 2026-11-15 — MP4 startup probe timeout

User reported F3 MP4 showing `Helper timed out`. Baseline repository was clean and workspace check/format/strict Clippy/tests passed (65 tests, two optional Poppler experiments ignored). Installed `/opt/homebrew/bin/mpv` 0.42.0 took 3.952 seconds for `--no-config --vo=help`, successfully reporting Kitty support; MCS's three-second probe deadline was too short. This failure precedes decoding the selected MP4.

Changed `crates/mcs-media/src/video.rs` to allow 15 seconds for the asynchronous output-driver probe and identify mpv in probe errors. Cancellation, output limits, child reaping and local-only playback arguments remain unchanged. `src/lib.rs` adds a 3.2-second fake-startup regression plus cancellation of a stuck probe. `docs/VIEWER.md` documents the deadline and corrects the obsolete claim that mpv is absent. No dependencies, OS settings, privileges or user media files changed.

`scripts/check_media_pty.py` initially failed after successful fake playback because Ratatui emits `F4-F8` and `files` separated by cursor positioning. A first attempt to preserve output across reads did not resolve that phrase-matching assumption; the final harness preserves prior output and matches the single `F4-F8` token. Final release media checks passed, including image/PDF cleanup, playback success/failure, repaint and termios/cursor/screen restoration. A second harness invocation delegated only the driver probe to installed mpv; it passed through MCS with playback still simulated.

Executed successfully: focused slow-probe/cancellation test; final `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (66 passed, two optional Poppler experiments ignored); `cargo build -p mcs`; `cargo build --release -p mcs`; release media and browser PTY suites; `git diff --check`. Manual release tool-PTY launch and q exit printed `TERMINAL_MODES_RESTORED exit=0`.

Security review: the probe remains bounded and cancellable; typed process arguments and playback restrictions are unchanged. Existing helper timeout/output/failure and safe-player-argument tests passed. Separate advisory/SAST scans were not run for this focused deadline/error-context change. No live Kitty decoding, A/V sync or performance acceptance is claimed. Next action: restart MCS from the normal outer shell with `cargo run --release -p mcs -- --backend tui` and retry the MP4. No commit/push performed.

## 2026-11-15 — Commander function-key positions

User requested conventional function-key operations rather than custom menu-letter positions. Checked the upstream Midnight Commander manual/keymap (https://source.midnight-commander.org/man/mc.html and https://github.com/MidnightCommander/mc/blob/master/misc/mc.default.keymap). Browser tiles now use F1 Help, F2 Actions, F3 View, F4 Edit, F5 Copy, F6 Move, F7 Mkdir, F8 Delete, F9 Menu, F10 Quit; F11/F12 are explicitly unassigned. F2 and F9 intentionally share a built-in session action menu in this build; there is no user-script execution/configuration. F4–F8 remain unavailable in the read-only milestone and explain that when pressed, without filesystem mutations. No copy/move/delete/editor implementation was authorized or added.

Files changed for this increment:

- `crates/mcs-tui/src/lib.rs`: scrollable F1 help, F2/F9 action menu using existing sort/hidden/filter/bookmark/refresh/icon commands, modal navigation/dismissal, static unavailable notices, function-key footer and regression tests. Existing aliases and both icon features are preserved.
- `crates/mcs-tui/src/shortcuts.rs`: ordered F1–F12 slots; dim disabled operations. Widths below 120 columns prioritize working actions; 120–143 show F1–F10; 144+ show all twelve without reassigning key numbers. Contextual filter/bookmark controls remain unchanged. Removed obsolete glyph/active-state logic for the former custom shortcut bar; status still reports sort/hidden/marks/filter values.
- `crates/mcs-tui/src/viewer.rs`: prevent F3/v interception while help/menu owns input.
- `scripts/check_tui_pty.py`: exercise F1 help/close, F2/F9 menu/close and F5 unavailable notice before existing navigation/viewer/restore checks. README documents exact mappings, unavailable/reserved keys, menu scope and macOS Fn usage.

Executed successfully: workspace check, format check, strict Clippy and workspace tests (50 passed, two optional media experiments ignored); debug/release builds; `MCS_ICONS=always python3 -u scripts/check_tui_pty.py` (all cases passed, including q/F10/Ctrl-C restoration); `git diff --check`. Final rendering cleanup also passed check/fmt/Clippy, focused TUI tests and both builds. A release tool-PTY smoke run exercised F5 notice, F1 help/close, F9 menu and its hidden-file action, then quit: `TERMINAL_MODES_RESTORED exit=0` from a shell stty comparison.

Tests cover ordered/reserved/dimmed slots, responsive/icon/text rendering, menu selection/direct shortcuts, bounded help scrolling, q dismissing help, filter isolation, F10 from overlays, preventing viewer/pane actions through overlays, and unchanged browser state after unavailable operations. Intermediate compile issues from removing the old renderer argument were fixed; the first PTY assertion expected contiguous "Unavailable", which Ratatui splits around matching cells. It now checks the emitted explanatory fragment; unit tests verify notice/state behavior.

Security review: menus dispatch a fixed list of existing read-only commands; no shell/scripts, file writes, privilege or dependency changes. Static notices/help do not introduce untrusted display content. Separate scanners/media/hardware checks were not needed for this input/UI increment. No real Kitty visual acceptance claimed. Release rebuilt; restart MCS to use the new bindings. No commit/push performed; previous UI/icon changes retained.

## 2026-11-15 — Modern icon shortcut menu

User clarified that icons were intended for the bottom menu and explicitly requested retaining the filename icons. Replaced the continuous cyan MC-style strip with separated rounded shortcut tiles, centered action icons/labels, key hints in the top border, cyan primary actions, red Quit and yellow active sort/hidden/filter/mark indicators. Status uses a restrained cyan-on-black line. Existing file icons and input bindings are preserved; `i` now toggles file and menu icons together.

- `crates/mcs-tui/src/shortcuts.rs`: presentation-only responsive tile renderer and fixed trusted glyph mapping; standard terminal colors, no graphics/font dependency. At 80 columns all eight icons fit; narrow windows prioritize a subset and drop decoration before action labels. Short windows retain a flat textual key/action fallback.
- `crates/mcs-tui/src/lib.rs`: five-row footer (two status/hint rows plus three tile rows), contextual filter/bookmark tiles, explicit active-state styling and updated buffer assertions. The footer costs two more rows than the previous strip; browser/viewer state and file behavior are unchanged.
- README: documents the shared icon toggle. Added tests for all eight menu icons at 80/100/160 columns, key visibility, text fallback and active-state colors. Existing tiny-height/width, input isolation, sanitization, filename/mark alignment and modal tests remain passing. The earlier footer geometry/color assertions were updated for the requested redesign.

Executed successfully: `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (48 passed; two optional media experiments ignored), debug/release builds, `MCS_ICONS=always python3 -u scripts/check_tui_pty.py` (q/F10/Ctrl-C, resize, text viewer return and termios/cursor/screen restoration), and `git diff --check`. A release tool-PTY run exercised active hidden/sort indicators, filtering/cancel, bookmark sidebar/close and icon toggle; exited 0 with shell `TERMINAL_MODES_RESTORED exit=0`. An intermediate caption variable-shadowing compile error was fixed before final checks.

Security review: only rendering and static glyphs changed; filename sanitization, modal input boundaries, read-only behavior and resource limits remain intact. No dependencies or privilege changes; separate security scanners/media benchmarks were not warranted. Actual Kitty appearance remains for user visual acceptance; no screenshot comparison claimed. Release binary rebuilt; restart to inspect. No commit or push performed.

## 2026-11-15 — Filename category icons

User requested emoji/category icons. Added `crates/mcs-tui/src/icons.rs` with decorative folder/link/image/video/audio/PDF/archive/code/config/text/generic-file glyphs, using existing entry metadata and filename extensions only. Links (including broken links) retain their own identity; directories take precedence over filename extensions. These categories do not imply viewer-format support.

`crates/mcs-tui/src/lib.rs` reserves a two-cell icon column while preserving marks, filename escaping, selection and modal input. `i` toggles icons for both panes for the session; the header advertises it. Startup enables icons for UTF-8 locales on ordinary terminals and defaults to ASCII on Linux virtual consoles, dumb/unknown terminals and non-UTF-8 locales. `MCS_ICONS=always|never` overrides startup selection; `i` remains available. README documents behavior and font limitations. No graphics protocol, Nerd Font, new dependencies or additional filesystem reads are required.

Verification executed: workspace check/fmt/strict Clippy; workspace tests (47 passed, two optional media experiments ignored); debug/release builds; `MCS_ICONS=always python3 -u scripts/check_tui_pty.py` (all q/F10/Ctrl-C, resize, viewer return and restoration checks passed). New tests cover category precedence, mixed-case/unknown extensions, ASCII defaults/overrides, fixed filename columns and marks in both modes, tiny rendering and toggle isolation from filter/sidebar input. Initial format and Clippy len-zero findings in test code were corrected; final format/Clippy and focused TUI tests passed. Release tool-PTY run exercised icons off/on and pane switching; exit 0 with `TERMINAL_MODES_RESTORED exit=0`. `git diff --check` passed.

Security review: icons are fixed trusted strings; untrusted filenames still pass through display sanitization, and file actions/permissions are unchanged. No parsing, subprocess or dependency boundaries changed; separate scanners and media checks were not run for this presentation change. Emoji font coverage/width cannot be reliably auto-probed; actual Kitty glyph appearance remains for user visual acceptance. Restart the rebuilt release binary to inspect it, and use `i` if the font disagrees. No commit/push performed; prior footer edits preserved.

## 2026-11-15 — Classic browser footer

User requested a Midnight Commander style bottom panel. `crates/mcs-tui/src/lib.rs` now renders a blue status strip, concise navigation hints and an edge-to-edge cyan shortcut bar with contrasting key badges. The footer uses three rows instead of four, giving the file panes one extra row. Existing bindings remain unchanged; only working actions are advertised. Filter and bookmark modes show their own controls. Narrow windows reduce the shortcut set, preserving F10 Quit at widths of at least eight cells; very short windows retain the bottom bar.

Updated buffer tests cover standard sizes, widths 8–160, heights 1–5, key/label colors, contextual controls and escaped control/bidi filter text. An initial new test exposed footer loss at tiny heights; reserving the footer before laying out the browser fixed it.

Executed successfully: `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (44 passed, two optional media experiments ignored), debug and release builds. `python3 -u scripts/check_tui_pty.py` passed resize, viewer return, filter/sidebar and q/F10/Ctrl-C terminal-mode/cursor/screen restoration. A release tool-PTY run exercised pane switching and filter/cancel, then exited with `TERMINAL_MODES_RESTORED exit=0` from a shell stty comparison. Final hint wording was shortened to fit 80 columns; Rust gates and both builds were rerun.

Security review: rendering-only changes retain display sanitization and input-mode isolation; no dependency, filesystem-operation, subprocess or privilege changes. Separate security scanners and media/hardware acceptance were not needed for this footer-only change. No real Kitty screenshot comparison was performed; next step is restarting MCS in the user's terminal to inspect the new footer. No commit or push performed.

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

## Approved macOS viewer plan — checkpoint 1

Implemented `mcs-viewer` paged text state and a bounded asynchronous worker, integrated through F3/v with fullscreen return-to-browser, search, paging, wrap, line numbers and reload. No mmap or whole-file text allocation. Worker generation tokens cancel scans/search and reject stale results; the UI remains responsive. Regular files are checked before/after a nonblocking Unix open to avoid FIFO-open races. UI display escapes controls/bidi.

Executed: baseline check/fmt/Clippy/test (21 tests) passed; text checkpoint `cargo test --workspace` passed (26 tests, including 3 GiB sparse seek, bounded long lines, UTF-8, forward/reverse cross-chunk search, cancellation and file changes). Initial Clippy checked-division/collapsible-match findings fixed; strict Clippy then passed. `cargo build -p mcs` and extended `python3 -u scripts/check_tui_pty.py` passed, including F3/search/return and terminal restoration for q/F10/Ctrl-C after resize. This is tool-PTY evidence, not Kitty graphical acceptance.

Proceeding within the approved plan to image/PDF adapters and optional mpv handoff. Kitty/Poppler/mpv are not installed; no OS package installation authorized or attempted. Linux/SSH acceptance is deferred to backlog by user direction.

## Approved macOS viewer plan — final implementation evidence

Implemented all three code checkpoints; live Kitty/media acceptance remains pending optional-tool setup. This does not mark image/PDF visual or video performance acceptance complete.

Changed files/subsystems:

- `Cargo.toml` / `Cargo.lock`: register mcs-viewer and mcs-media; lock reviewed image/transport/tempfile dependencies. `crates/mcs-tui/Cargo.toml` connects them and disables ratatui-image default/system features.
- `crates/mcs-viewer/{Cargo.toml,src/lib.rs,src/text.rs}`: terminal-independent routing, bounded worker, paged text/index/search, generation cancellation/progress and focused tests. Binary/special-file rejection, UTF-8/control display, initial-EOF reads and metadata invalidation are explicit.
- `crates/mcs-media/{Cargo.toml,src/lib.rs,src/process.rs,src/video.rs}`: bounded PNG/JPEG decode, borrowed-crop viewports, optional Poppler page/text adapters, typed helper execution with cancellation/time/output limits, and prepared mpv handoff. Native parser subprocesses are not claimed sandboxed.
- `crates/mcs-tui/src/{lib.rs,viewer.rs,graphics.rs}`: explicit screen states, F3/v/search input, fit/zoom/pan/PDF pages, Kitty probe and off-thread encoding, owned-ID deletion/drop fallback, terminal handoff/resume and state preservation. Paging dimensions match actual bordered content and resize discards stale-size results.
- `scripts/check_tui_pty.py`: extend text F3/search/close checks. `scripts/check_media_pty.py`: simulate Kitty capability/DSR replies and fake Poppler/mpv to exercise transport/lifecycle, not visual quality. `.gitignore` excludes Python bytecode caches.
- README, architecture, decisions, milestones, security and test-plan docs updated; new `docs/VIEWER.md` records controls/limits/dependency review, `docs/BACKLOG.md` defers Linux/SSH and other long-term work.

Final executed checks, all successful unless explicitly noted:

- `cargo check --workspace`.
- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace`: 37 passed (3 CLI, 3 core, 10 filesystem, 5 media, 9 TUI/graphics, 7 viewer). No test failures in the final run.
- `cargo build -p mcs`.
- `python3 -u scripts/check_tui_pty.py`: passed CLI/native/error checks, F3 text/search/return, sidebar/filter/resize and q/F10/Ctrl-C termios/cursor/screen restoration.
- `python3 -u scripts/check_media_pty.py`: passed simulated Kitty image encode/zoom/owned-ID cleanup, fake Poppler page/search/resize, fake mpv success/failure handoff and terminal restoration. Final scripts ran against the final built binary.
- Manual tool-terminal `cargo run -p mcs -- --backend tui`: opened README with v, searched for viewer, closed and quit; exit 0 and shell reported `TERMINAL_MODES_RESTORED exit=0` after comparing stty state. This is a macOS tool PTY, not real Kitty visual acceptance.
- `git diff --check`: passed; source/dependency diff and runtime file-mutation/subprocess boundaries reviewed. No unsafe block, sudo, native renderer or source-file mutation added. No commit or push requested/performed for this change.

Intermediate issues resolved: style/test compile errors during interface changes; helper search-argument grouping; borrowed image crop adapter; an off-by-one text paging/layout calculation; bounded combining-character display; stale-size completion after resize. Simulated terminal tests initially expected contiguous text despite terminal diffs and omitted cursor-position replies on resume; the emulator now handles them. They do not substitute for live media checks.

Dependency evidence: initial sandbox DNS prevented fetching, then approved elevated Cargo fetch/check succeeded. Host registry manifests/build features were inspected, including permissive license alternatives and Rust requirements (VIEWER.md). `cargo metadata --format-version 1 --locked` attempted uncached cross-platform downloads and failed sandbox DNS; the host-source review was completed directly from downloaded manifests instead. `cargo audit --version` reports no installed audit command, so no advisory audit is claimed. No system packages installed.

Remaining acceptance and constraints:

- Kitty, mpv and Poppler executables are absent from the inspected standard locations/PATH. Homebrew exists. Their installation was explicitly kept separate in the approved plan; live checks require setup authorization or a user-provided installation.
- Real PNG/JPEG/PDF quality, password/corrupt cases with actual Poppler and a 60-second 720p/30 mpv run (dimensions/drops/sync/CPU/memory) are unverified. No playback FPS, memory or A/V-sync claim is made.
- Filesystem reads can still block inside workers; metadata checks are best effort, not snapshots. Native helper memory is not hard-capped by the OS; output/time/pixel bounds are not a sandbox. Unexpected fatal signals/crashes and hung playback still require manual recovery testing.
- Very long lines use bounded segments; no UTF-16/hex/live-follow, PDF OCR/graphical search boxes, extra formats, multiplexer graphics, GUI fallback or file editing/mutations.
- Linux/SSH and DRM/system work stay in the long-term backlog, per user direction.

## 2026-11-15 — Retina viewport regression fix

User reported an F3 PDF failure, "Viewport exceeds 4 megapixels", in Kitty. Root cause: the encoded-raster budget was incorrectly applied to the full terminal pixel area, so normal Retina windows could be rejected before rendering. No personal document was inspected to reproduce this geometry bug.

- `crates/mcs-media/src/lib.rs`: compute crop/fit in actual terminal coordinates, then uniformly downsample the raster within four megapixels / 4,096 per dimension. Preserve physical display dimensions separately; existing viewport() callers remain compatible. Guard zero source dimensions and exact integer budgets.
- `crates/mcs-tui/src/graphics.rs`: use physical display dimensions for cell placement and supply explicit c/r on the initial Kitty virtual-placement header. This small adapter extends ratatui-image 11.1's encoded transmission, rather than writing an image codec/transport. Prevents the bounded raster from appearing smaller than the fitted window. A status note indicates budget scaling.
- `crates/mcs-tui/src/viewer.rs`: render the placement-aware widget. `docs/VIEWER.md`: document downsampling instead of rejecting large terminal areas.
- Added regression cases for Retina/5K/ultrawide/portrait/extreme dimensions and a 4,000x3,200 physical viewport that emits a bounded 2,048x2,048 raster into an explicit 160x80-cell Kitty placement. Repaints do not retransmit the image.

Executed: cargo check --workspace, cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace (39 passed), cargo build -p mcs, git diff --check; all passed. No dependency changes, user-file mutations or security-gate weakening. The raster cap remains enforced; no memory/performance improvement is claimed.

Kitty, mpv and Poppler are now installed by the user. Actual Kitty UI inspection through the computer-use tool is unavailable (app access denied); visual correctness is not inferred from buffer/protocol tests. Restart MCS to load the rebuilt binary, then repeat F3 in the user's Kitty session. Prior statements about missing tools are historical and superseded.

Follow-up verification exposed a pre-existing intermittent text PTY failure: F3 could be swallowed after an unsupported-terminal probe timed out. Inspection of ratatui-image 11.1's query_with_timeout showed its detached blocking stdin reader could outlive the timeout. Replaced startup probing in `crates/mcs-tui/src/capabilities.rs` / `src/lib.rs` with main-thread Mio readiness polling, a 300 ms overall deadline and a 4 KiB reply cap, before Crossterm starts event reading. No reader survives timeout. Positive Kitty acknowledgement and nonzero cell metrics are still mandatory; multiplexers remain unsupported. Added fragmented/reordered/negative/invalid-metric reply tests.

`crates/mcs-tui/Cargo.toml` and Cargo.lock add a direct Unix Mio dependency, reusing existing locked mio 1.2.5 (MIT, tokio-rs/mio). Source/portability/license reviewed; no new resolved package version, unsafe code or system library was introduced. This dependency change supersedes the earlier no-dependency-change note for the initial Retina-only fix.

Final follow-up checks passed: cargo check, format check, strict Clippy, workspace tests (41 passed), build, both complete Python PTY suites, git diff --check. A fresh tool-terminal run with the final probe exited 0 with `TERMINAL_MODES_RESTORED exit=0`. No real Kitty visual acceptance is claimed; the user should restart the running app to load the rebuilt binary and retry F3.

## 2026-11-15 — Measured viewer-opening performance fix

User reported very slow F3 opening. Profiled generated fixed-content PDF fixtures rather than user documents, before changing the pipeline. Environment: arm64 macOS 26.6.2, rustc 1.99.1, Poppler 26.11.0. Three sequential samples per profile, 4,000x3,000 physical viewport; no competing build during timed experiments. Numbers below are medians, not guarantees or real Kitty display latency.

Baseline findings:

- Default debug pipeline preparation: 2,491.01 ms median (metadata + render/decode + resize + Kitty encode + Ratatui buffer preparation). Software resize alone took roughly 1,650–1,670 ms.
- Release pipeline preparation: 482.53 ms median. The native Poppler PNG render/output stage remained expensive even with optimized Rust.
- Separate actual-helper comparison: PNG output took 394–398 ms and produced 534,931 bytes; binary PPM output took 27–28 ms and produced 8,896,529 bytes. Both used the same generated PDF/scale.
- Uncompressed terminal encoding was 22,473,593 bytes for a needlessly upscaled 1,722x2,435 raster.

Changes by file:

- `crates/mcs-media/src/lib.rs`: request bounded binary RGB (P6) from Poppler and decode using the mature image-rs PNM decoder, eliminating PNG compression/decompression on the internal pipe. Keep public image input restricted to PNG/JPEG, with identical dimension/decoded-byte/output/time checks. Preserve native source-crop resolution whenever the terminal can scale it; perform software resize only for actual downsampling.
- `crates/mcs-media/Cargo.toml`: enable image's PNM feature solely for internal Poppler output; no new package version/dependency. Added native-size/public-format rejection coverage.
- `crates/mcs-tui/src/{capabilities.rs,lib.rs,viewer.rs,graphics.rs}`: separately query Kitty zlib support; only a positive reply enables ratatui-image's lossless compression. Retain raw transport without acknowledgement. Preserve explicit cell placement independently of the smaller raster. Added compression-confirmation/fallback coverage and explicit ignored profiling/pixel-equivalence experiments.
- `scripts/check_media_pty.py`: fake Poppler emits P6 and simulated Kitty acknowledges compression; assert compressed output and lifecycle cleanup. `scripts/check_tui_pty.py`: optional MCS_TEST_BINARY selects the release binary for testing.
- README and VIEWER docs recommend `cargo run --release -p mcs -- --backend tui` for normal viewing; security/test docs explain internal-format and profiling boundaries. Cargo.lock remains consistent; no new resolved package version.

After the changes:

- Debug preparation: 318.39 ms median versus 2,491.01 ms baseline, using confirmed compression.
- Release preparation: 59.09 ms median versus 482.53 ms baseline, using confirmed compression.
- Terminal encoding: 826,417 bytes versus 22,473,593 bytes baseline; raster now stays at the PDF source's 1,448x2,048 pixels and Kitty preserves its fitted display size. The terminal write/compositor cost was not included in the timing; actual documents, photos, cold caches and load vary.
- Compression trades CPU for transfer size and benefits text/flat-color content most. No claim of a universal photo-heavy speedup, hardware FPS, or general security certification.

Commands executed:

- `cargo test -p mcs-tui benchmark_pdf_open -- --ignored --nocapture` and the `--release` equivalent, both before and after implementation: passed; three samples each, both raw/compressed encoding variants reported.
- `cargo test -p mcs-tui pdf_ppm_matches_png_pixels -- --ignored`: passed with actual Poppler; pixel buffers from P6 and the prior PNG transfer were identical for the generated fixture.
- Workspace check, format-check, strict Clippy and workspace tests passed. Default tests: 43 passed; two optional Poppler experiments intentionally ignored by default and explicitly run as recorded above.
- Debug and release `cargo build -p mcs`: passed.
- Both debug PTY suites passed; release simulated-media PTY suite also passed (`MCS_TEST_BINARY=target/release/mcs python3 -u scripts/check_media_pty.py`), including confirmed compression, PDF search/resize, owned-ID cleanup, fake video failure recovery and terminal restoration.

Security/resource review: raw helper output is larger than PNG, but remains within the existing 64 MiB pipe limit, parser dimensions/decoded-byte bounds and 10-second timeout. Public PNM input is rejected. No input validation, cancellation, source-file safety, authorization or memory limit was removed. No OS packages installed, no user document inspected, no commit/push performed. Advisory scanning remains unavailable as previously recorded.

Final release tool-terminal smoke run exited 0 with `TERMINAL_MODES_RESTORED exit=0` from shell stty comparison. No real Kitty display-latency or private-document benchmark was performed.

## 2026-11-15 — User-authorized F4–F8 file operations

Implemented the explicitly requested file operations over the existing uncommitted icons/help/menu work, preserving that work. No commit or push performed.

Changed files for this milestone:

- `crates/mcs-core/src/operations.rs`: captured descriptor-relative plans; exclusive recursive copy, atomic no-replace move, mkdir, recursive deletion, bounded UTF-8 editor with staged saves and source revalidation. Core state exposes marked paths with selection fallback in `src/lib.rs`.
- `crates/mcs-core/Cargo.toml` and Cargo.lock: direct macOS/Linux rustix 1.1.6 fs dependency, reusing the existing locked version. Cached source/manifest/platform implementation reviewed (Bytecode Alliance, MIT/Apache alternatives, Rust 1.65 minimum); no newly resolved package version.
- `crates/mcs-tui/src/operations.rs`: one asynchronous worker, modal destination/review/editor/result screens, explicit y confirmation, failed-save buffer retention, pane refresh. Browser lib/viewer input and shortcut tiles integrate F4–F8 while retaining F11/F12 reservations and narrow-terminal Quit visibility.
- `scripts/check_operations_pty.py`: real-binary F4–F8 mutations confined to a new private temporary fixture; verifies no changes before y. Existing TUI/media harness assertions updated for the active operations/header.
- README, FILE_OPERATIONS, architecture, security, test-plan and milestone docs record controls, boundaries and limitations. Existing unrelated changes in icons and prior status entries were preserved.

Executed checks:

- Initial workspace check briefly reported a shortcut-draw argument mismatch while the pre-existing uncommitted UI files were changing; a fresh baseline check/fmt/strict Clippy/test then passed before operation implementation.
- Final `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: passed; 57 tests passed, two pre-existing optional Poppler experiments ignored.
- `cargo build -p mcs`: passed.
- `python3 -u scripts/check_operations_pty.py`: passed F4 edit/save, F5 copy, F6 move, F7 mkdir, F8 permanent deletion, no writes before confirmation, termios/cursor/alternate-screen restoration. Generated fixture retained for inspection; no existing user file was mutated by verification.
- `python3 -u scripts/check_tui_pty.py`: passed CLI, help/menus, cancelled copy prompt, viewer/search, resize and q/F10/Ctrl-C restoration.
- `python3 -u scripts/check_media_pty.py`: passed simulated Kitty, fake Poppler and fake mpv lifecycle/restoration checks.
- Manual tool PTY `cargo run -p mcs -- --backend tui`: opened F7, cancelled without writing, quit; shell stty comparison printed `TERMINAL_MODES_RESTORED exit=0`.
- `git diff --check`: passed. Reviewed mutation boundaries, dependency lock diff and modal input isolation. `cargo audit --version` confirms cargo-audit remains unavailable; no advisory scan or separate SAST run is claimed.

Intermediate fixes: moved a helper before the test module for strict Clippy; changed a sandbox-denied Unix socket fixture to a newly owned FIFO; fixed tiny-terminal shortcut clipping. The operation PTY harness initially sent y while plan preparation was still pending (correctly ignored); it now waits for the actual confirmation dialog. The media harness still expected an old browser header; assertions updated to the current header and the complete suite rerun successfully.

Security and limitations: each write requires y after review; child symlinks are never followed in recursive work, regular-file opens use NOFOLLOW/nonblocking plus fstat, destinations are exclusive, and source metadata changes invalidate plans. Tests cover ordinary changed-source and destination races, symlinks/broken links, special files, self-descendant/root/parent rejection and modal save recovery. These checks are not a snapshot or hostile-writer sandbox; check-to-unlink/rename races remain documented. Copy/delete have entry/depth limits and copy a 16 GiB limit; batches/failed copies can leave partial output. Delete has no undo; moves across filesystems are rejected. Editor supports regular UTF-8 files up to 1 MiB, rejects symlinks/multiple hardlinks, and saves by replacing the file; ACLs/xattrs and other extended metadata are not preserved. Exit during ongoing I/O can leave partial output. Linux, live Kitty visuals, disk-full recovery and adversarial writer stress remain unverified. See FILE_OPERATIONS.md for exact contracts. Next acceptance is restarting MCS and inspecting these dialogs in the user's Kitty session.

Final delivery rebuild: `cargo build -p mcs` and `cargo build --release -p mcs` both passed after the final cleanup/error-reporting review. `MCS_TEST_BINARY=target/release/mcs python3 -u scripts/check_operations_pty.py` also passed all F4–F8 mutations, confirmation and terminal-restoration checks. Final workspace check/format/Clippy/tests and diff whitespace checks passed. Restart with `cargo run --release -p mcs -- --backend tui` to load these changes.

## 2026-11-15 — Compact operation dialogs and direct F6 confirmation

User reported F6 appearing ineffective after Enter and provided a screenshot of the oversized destination modal. Root cause of the interaction confusion: Enter only prepared a second review, then y was required to execute. Replaced this flow for Copy/Move/Mkdir: the destination dialog shows source/name or marked count, source directory, destination and explicit `[ Enter: Move/Copy/Mkdir ] [ Esc: Cancel ]` hints. Enter now runs path validation and executes the displayed action in the existing worker; no hidden second confirmation. Delete and editor Save still require y. No overwrite/path-safety protections changed.

`crates/mcs-tui/src/operations.rs`: destination, confirmation, working and result modals are content-sized (up to 80 columns) and centered, retaining the browser background. The editor keeps its large viewport. Added F6-to-other-pane execution/collision regression tests, Esc/no-write checks, centered-buffer/background assertions and tiny-terminal coverage. `src/lib.rs` help, README, FILE_OPERATIONS, SECURITY and TEST_PLAN updated to describe the exact confirmation keys. `scripts/check_operations_pty.py` now verifies destination typing causes no writes and Enter directly executes copy/move/mkdir; save/delete remain y-confirmed. Existing uncommitted work preserved.

Executed successfully: cargo check --workspace; cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace (59 passed, two optional Poppler experiments ignored); cargo build -p mcs; cargo build --release -p mcs; MCS_TEST_BINARY=target/release/mcs python3 -u scripts/check_operations_pty.py; python3 -u scripts/check_tui_pty.py; git diff --check. Real-binary tests verified F4–F8, explicit confirmation/no-write-before-confirmation, q/F10/Ctrl-C and terminal restoration. Filesystem tests affected only newly owned fixtures, not user Documents. Intermediate compile/Clippy errors from moving the area calculation and using a manual clamp were fixed before final checks.

Security review: confirmation remains explicit, with visible source/destination/action; unchanged core preflight, metadata rechecks, exclusive creation and atomic no-replace moves continue to pass regression tests. No dependency or privilege changes. Separate advisory/SAST scanners and media suites were not needed for this isolated dialog/input change; live Kitty visual acceptance and previously documented filesystem-race/partial-output limits remain unchanged. Restart the running app to load the rebuilt release binary.

Manual final release tool-PTY check: opened the centered F6 dialog, verified the visible `[ Enter: Move ] [ Esc: Cancel ]` hints, cancelled without mutation and quit. Shell comparison printed `TERMINAL_MODES_RESTORED exit=0`. This is a tool-terminal check, not real Kitty screenshot acceptance.

## 2026-11-15 — Silent success for file operations

Removed the success acknowledgement modal at the user's request. `crates/mcs-tui/src/operations.rs` now refreshes both panes and clears the operation dialog immediately on success, including copy/move/mkdir/delete and editor save. Errors still remain visible, failed saves keep their buffers, and pre-action confirmations/path-safety behavior are unchanged. Updated existing state tests to assert inactive dialogs after success and retained collision/save-failure checks. `scripts/check_operations_pty.py` now chains real F4–F8 actions without extra result-dismissal keys and quits with q immediately after deletion, proving browser input is restored. FILE_OPERATIONS.md documents this behavior. Existing uncommitted work preserved; no commit/push.

Executed successfully: baseline and final cargo check --workspace, cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace (59 passed; two optional Poppler experiments ignored); cargo build -p mcs; cargo build --release -p mcs; MCS_TEST_BINARY=target/release/mcs python3 -u scripts/check_operations_pty.py; git diff --check. PTY checks verified no post-success acknowledgement, actual filesystem results in a newly owned fixture, and termios/cursor/alternate-screen restoration. No user files were mutated by verification. The existing failure-path tests still pass. No dependencies, mutation permissions or core filesystem code changed; separate media/security scanners were not warranted for this small success-presentation change. Live Kitty appearance remains unverified; restart the app to use the rebuilt release binary.

## 2026-11-15 — File-operation milestone commit preparation

User requested committing the completed work. Reviewed pending source/documentation/dependency changes and new files, checked whitespace and common credential patterns in new files (no matches), and removed a stale README sentence that still described F4–F8 as unavailable. Commit scope includes the accumulated commander menus/icons, file operations, compact direct-confirmation dialogs, silent success behavior, regression harnesses and documentation. Latest executed Rust/build/PTY evidence is recorded above; no runtime code changed during commit preparation, so tests were not repeated. No push requested.

## 2026-11-15 — User-authorized F11 persistent pane shells

Implemented the requested pane terminal toggle independently of the remaining M5 system-panel work. Repository was clean at start; no commit or push performed.

Changed files:

- `crates/mcs-tui/src/shell.rs`: lazy per-pane interactive `$SHELL` PTYs started in the browser directory; persistent state/screen while hidden; bounded, cancellable nonblocking I/O workers; VT cell rendering, color/Unicode, resize, foreground interrupts, shell exit/restart and owned shell/foreground-group teardown with asynchronous reaping. No application unsafe code.
- `src/lib.rs`, `src/viewer.rs`, `src/shortcuts.rs`: F11 shell/files toggle, shell input before browser commands, Ctrl-] pane switching (including Crossterm's legacy Ctrl-5 decoding), contextual hints and Shell shortcut. Existing modal input stays isolated; browser location/state retained, listing refreshed when toggled back.
- `crates/mcs-tui/Cargo.toml`, Cargo.lock: portable-pty 0.9.0, vt100 0.16.2 and nix 0.28.0 (reuses portable-pty's version) gated to macOS/Linux. Source, MIT licenses, Unix PTY/session/descriptor behavior, parser allocation and portability reviewed; no graphics/system library or OS installation. Network-restricted dependency fetch first failed DNS, then succeeded through the approved Cargo escalation. No dependency advisory scan passed: `cargo audit --version` reports unavailable.
- `scripts/check_shell_pty.py`: real-binary shell/session/resize/failure/restore checks in new retained temporary fixtures. README, SHELL, ARCHITECTURE, MILESTONES, SECURITY and TEST_PLAN document behavior, budgets and limitations.

Executed successfully:

- Initial and final `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Final tests: 63 passed, two pre-existing optional Poppler experiments ignored.
- `cargo build -p mcs` and `cargo build --release -p mcs`.
- `MCS_TEST_BINARY=target/release/mcs python3 -u scripts/check_shell_pty.py`: variable/cwd persistence, hidden-command continuation, Ctrl-C interruption, independent pane PIDs/state, resize (`24x68` interior), shell exit/restart, unavailable-shell recovery, shell cleanup and termios/cursor/alternate-screen restoration passed.
- `python3 -u scripts/check_tui_pty.py`: CLI, help/menu, browser/viewer/filter input, resize and q/F10/Ctrl-C restoration passed.
- `python3 -u scripts/check_operations_pty.py`: actual F4–F8 operations, confirmations and restore passed in new private fixtures. `python3 -u scripts/check_media_pty.py`: simulated Kitty, fake Poppler/mpv and restoration passed.
- Manual tool PTY `cargo run -p mcs -- --backend tui`: F11 shell, set/print session variable, F11 files, F11 restore, print retained variable, F10 exit. Shell comparison printed `TERMINAL_MODES_RESTORED exit=0`.
- `git diff --check`: passed.

Intermediate verification caught and fixed legacy Ctrl-] decoding. Harness assumptions about contiguous long paths, blank cells in Ratatui diffs and startup timing were corrected; final tests use short labeled outputs and wait for startup. Strict Clippy caught a single-element loop in the former reserved-F11/F12 test; fixed before final checks.

Security review found vt100's std-enabled vte OSC payload buffer is unbounded. Unsupported OSC strings are now discarded incrementally before parsing, with cancellation of partial parser sequences to prevent escape splicing. A regression feeds an 8 MiB unterminated OSC across chunks without accumulating its payload. Parsed controls never reach host output; no clipboard/title/graphics passthrough. Screen size is capped at 200x500, no scrollback; each shell has eight 8 KiB output slots and 32 input slots, with per-tick output work bounded. I/O workers cancel even if detached children retain descriptors.

Shell commands run as the user, with inherited environment and normal shell startup configuration. They can modify files directly without browser confirmation dialogs; no sandbox or privilege elevation is claimed. Sessions continue while hidden and last only for this MCS process. F10 kills owned shells/current foreground groups; detached background groups may survive. Mouse, graphics, terminal query replies, extended keyboard protocols and scrollback UI are not implemented. Linux/SSH execution, real Kitty visuals and interactive editor compatibility remain unverified. No performance claims or separate SAST/advisory audit. Next acceptance: restart MCS using the rebuilt release binary and exercise F11 in the user's terminal.

## 2026-11-15 — Global single-instance guard and nested-launch ban

User requested that MCS launches from its embedded terminal be ignored and that only one instance run globally. Implemented host-wide coordination per effective OS user, across terminals, cwd, HOME/TMPDIR and executable paths. Prior uncommitted F11 work preserved; no commit/push.

Changes:

- `apps/mcs/src/instance.rs`: atomic nonblocking exclusive OS file lock at fixed `/tmp/mcs-<euid>.lock`. Validate the opened descriptor's regular-file type, ownership, private permissions and single link; NOFOLLOW/nonblocking opens, no truncation, no unlink. Persistent inode avoids unlink/recreate races; lock releases on descriptor/process death and is close-on-exec. Other platforms fail explicitly before terminal initialization.
- `apps/mcs/src/main.rs`: embedded-shell ban and instance acquisition before CLI parsing/backend startup. Existing-owner/nested calls return silently with exit 0 and no terminal I/O; unsafe/inaccessible lock state fails closed with an error.
- `crates/mcs-tui/src/shell.rs`: set `MCS_EMBEDDED_SHELL=1` on every child shell; global locking also prevents recursion if the marker is removed.
- `apps/mcs/Cargo.toml`, Cargo.lock: direct cfg-gated nix 0.28 dependency reuses the prior F11 version, adding user-ID access. No new resolved package/version for this increment; MIT source/license/portability reviewed. Locking uses stable std File APIs; no unsafe added.
- `scripts/check_instance_pty.py`, `scripts/check_shell_pty.py`: duplicate/concurrency/crash/path independence, nested marker and unmarked recursion regression coverage. README, SHELL, ARCHITECTURE, SECURITY and TEST_PLAN document behavior and sequential PTY-suite execution.

Executed successfully:

- Baseline and final `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: 65 tests passed, two optional Poppler experiments ignored. New tests cover lock exclusivity/reuse and symlink/hardlink/directory/FIFO/insecure-permission rejection, preserving existing target bytes.
- `cargo build -p mcs`; `cargo build --release -p mcs`.
- Sequential release `scripts/check_instance_pty.py` and `scripts/check_shell_pty.py`: silent status-0 duplicates including help/invalid args, differing cwd/HOME/TMPDIR, eight concurrent contenders, exactly one winner of simultaneous starts, normal/crash release, marker ban without a parent, actual nested shell launches with/without marker, shell lifecycle and termios/cursor/screen restoration all passed.
- `python3 -u scripts/check_tui_pty.py`: CLI, help/menus, browser/viewer/filter, resize and q/F10/Ctrl-C restoration passed.
- Manual tool-PTY release launch: while it owned the terminal, separate debug and release executable calls both returned 0 with empty stdout/stderr. Quit restored terminal settings; shell comparison printed `TERMINAL_MODES_RESTORED exit=0`.
- `git diff --check`: passed. An initial Python test-command quoting error was corrected before successful harness execution.

Limits: applies to cooperating updated binaries for the same OS user; separate users/containers, older unguarded binaries and deliberate lock-inode removal are outside this coordination contract. Restart the currently running older MCS binary to enable protection. Lock files intentionally remain; do not remove an active lock file. Invalid lock ownership/type/permissions fail closed rather than launch unguarded. macOS verified; Linux code is cfg-gated but runtime remains unverified. Separate advisory/SAST scans and unrelated media/operation PTY suites were not rerun for this startup-only increment; prior evidence remains above. No user file operations, OS installations or privilege changes.

## 2026-11-15 — Physical Fn+F11 troubleshooting and alternate toggle

User reports Fn+F11 leaves MCS visible with no apparent change. Reproduced F11 only via synthetic PTY input: release shell/session tests passed; physical key delivery remains unverified. User confirmed no visible Show Desktop transition. No root cause is asserted. Apple documents Fn+F11 as a possible desktop shortcut, but this does not establish the user's event path. Requested `kitten show-key` capture in the normal Kitty shell. No macOS/Kitty settings changed.

`crates/mcs-tui/src/viewer.rs` now accepts Ctrl-T as the equivalent F11 toggle in the browser and pane shell, retaining modal/filter/sidebar/viewer isolation. `src/lib.rs` help/hints document it and show shell-exit notices. `src/shell.rs` retains exit status in Info rather than silently returning after a quick shell termination; startup errors render as wrapped pane text instead of only a clipped border title. Existing Ctrl-T transposition in child shells is now reserved for MCS toggle, as documented. The session-preserving F11 path and singleton protection remain intact.

`scripts/check_shell_pty.py` adds Ctrl-T opening a real `/bin/zsh` with private ZDOTDIR/HOME, verifies F11 restore/resume, and checks immediate shell exit with `/usr/bin/false`. Existing sh/nested-guard/invalid-shell checks remain. README, SHELL and TEST_PLAN explain the fallback and physical-key verification boundary.

Executed: baseline and final workspace check, format-check, strict Clippy and workspace tests (65 passed; two optional Poppler experiments ignored); debug/release builds; release shell PTY checks for sh/zsh/invalid-shell; browser PTY and release singleton PTY checks, all passed. Source/diff review confirms no host escape forwarding, privileges, lock changes, new dependencies or user-configuration writes. No separate advisory/SAST scan or unrelated media/operation suite rerun for this input/presentation change. Physical Fn+F11 remains unresolved pending key capture; Ctrl-T is the verified alternative. Restart the app from a normal outer shell to load the rebuilt binary.

Follow-up checks: immediate `/usr/bin/false` shell exit-status test passed. Manual release tool PTY confirmed Ctrl-T opens the shell and Ctrl-T restores files; q exited with `TERMINAL_MODES_RESTORED exit=0`. This validates the alternate binding, not physical Fn+F11 delivery.

## 2026-11-15 — Ctrl-T only; F11 unassigned by user request

User confirmed Ctrl-T works as expected and explicitly ended F11 troubleshooting. Ctrl-T is now the sole shell toggle. F11/F12 are again reserved/unassigned; F11 never opens/hides a pane shell. No further physical-key capture or OS/terminal configuration investigation is needed; earlier requests for such capture are withdrawn.

`crates/mcs-tui/src/lib.rs` handles Ctrl-T directly, advertises it in browser/shell hints and restores reserved-key regression coverage for F11/F12. `src/viewer.rs` no longer translates Ctrl-T to F11; active shells ignore F11. `src/shortcuts.rs` restores the dimmed F11 Unused tile, and `src/shell.rs` messages name Ctrl-T only. Updated `scripts/check_shell_pty.py` to use Ctrl-T throughout and assert F11 cannot open/hide shells. README, SHELL, ARCHITECTURE, MILESTONES, SECURITY and TEST_PLAN reflect the final binding; prior STATUS entries remain historical. Session persistence and the global single-instance guard are unchanged. Existing uncommitted work preserved; no commit/push.

Executed successfully: baseline/final workspace check, format-check, strict Clippy and workspace tests (65 passed; two optional Poppler experiments ignored), focused mcs-tui tests after the final footer-text change (28 passed, two ignored), debug/release builds and git diff --check. The attempted release shell PTY harness received no startup output because another instance owns the global lock; a nonblocking lock probe confirmed ACTIVE_INSTANCE_LOCK. The chained browser harness therefore did not run. Live/manual terminal-restoration reruns are deferred rather than stopping the user's active MCS instance or bypassing the guard. Prior terminal-restoration evidence remains above. No dependency, privilege, filesystem-operation or security-boundary changes; separate advisory/media/operations checks were unnecessary for this binding-only change. Restart the running instance to load the rebuilt release binary.

## 2026-11-15 — Pane-shell milestone commit preparation

User requested committing the accumulated Ctrl-T shell and singleton changes. Reviewed tracked diff, new-file inventory and whitespace; checked new files for common private-key/token patterns with no matches. Commit includes pane-local PTY sessions, Ctrl-T-only controls with F11/F12 reserved, safe single-instance startup, regression harnesses, dependencies and documentation. Validation evidence and active-instance PTY limitations are recorded above; no runtime code changed during commit preparation, so checks were not repeated. No push requested.
