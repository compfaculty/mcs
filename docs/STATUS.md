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
