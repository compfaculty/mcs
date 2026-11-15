# Test plan

## M0/M1
- `cargo check --workspace`; format, clippy, unit tests.
- 24x80 and 100x30 terminals; resize while running; exit restores cursor/raw mode.
- Empty, unreadable and huge directories; symlink traversal; weird filenames; Unicode/RTL; file name > display width.
- Navigation through directories with permission failures, mounted volumes and changing contents.
- SSH session disconnected mid-run; no stray terminal modes on normal exit.
- Tests must not delete or overwrite files.

## M2
- Compare Terminal.app/Kitty on macOS and SSH from Kitty.
- Image resize and scrollback; inline graphics cleanup; invalid/corrupt media; filesize limits and timeouts.
- PDF fuzz fixtures, memory bound, password-protected files.
- libmpv perf and A/V sync for local and network sources.

## M3/M4
- Hardware matrix Intel/AMD/NVIDIA as available; document untested configurations.
- Seat/VT release, Ctrl-C, SIGTERM, suspend/resume, hotplug, EDID changes, compositor collision, crash restoration.
- No blind root execution; device permission errors handled without destructive modifications.
- Frame timing/missed frames metrics, memory/CPU usage, video 1080p on representative hardware.

## Security
- Treat files and media as untrusted; no unguarded subprocess shell interpolation.
- Limit viewer memory/CPU and privileges; sandbox risky decoders when feasible.
- Privileged system changes separated from UI with user authorization.

## Implemented M1 local checks

- `cargo test --workspace`: CLI rejection before terminal initialization; bounded loading/history; atomic navigation success/failure; path-based highlight/mark retention; filtering/sorting/hidden files; session bookmarks; symlink/broken link; permission failure; empty/missing paths; 20,000-entry directory.
- TUI tests assert rendering at 80x24 and 100x30, tolerate 1x1/20x5 resize, isolate filter/sidebar input, bound filter text and escape untrusted control/bidi characters. These are buffer assertions, not screenshot comparisons.
- `cargo build -p mcs` then `python3 scripts/check_tui_pty.py`: actual executable, CLI errors/native unavailable, sidebar/filter input, 80x24 -> 100x30 resize, q/F10/Ctrl-C exits, termios equality, alternate-screen exit and cursor-show sequences. Python standard library only; macOS execution recorded, Linux execution pending.
- For a terminal-emulator manual check: save `stty -g`, launch TUI, navigate/filter/bookmark/resize, exit, compare `stty -g` and inspect cursor/shell. A tool-provided macOS PTY check is recorded in STATUS; Terminal.app/Kitty/SSH checks remain pending.
- Fixtures are exclusively created temporary directories/files, retained under the system temp directory; no test deletes or overwrites existing user files. The non-UTF-8 filesystem test is Linux-only because the local macOS filesystem rejected that fixture. Unix CLI non-UTF-8 rejection is separately tested on macOS. Permission assertions are explicitly skipped if the test account can bypass permissions.

## Approved macOS viewer gates

- Rust tests cover text paging/wrap/UTF-8/CRLF/BOM, literal cross-chunk search in both directions, cancellation, changed files, a 3 GiB sparse seek fixture, long-line bounds and 16 MiB index compaction. The sparse fixture establishes seeking/bounds, not representative log throughput.
- Image tests cover a real small PNG decode, malformed input, a valid CRC oversized IHDR rejected before allocation, fit/zoom/pan and viewport limits. Fake Poppler tests cover page count/render/text search/wrap/change detection, and helper timeout/cancel/output/failure.
- Viewer/client tests cover stale generations, input isolation, preserving browser state, tiny rendering and text highlighting. Fake mpv tests inspect typed/safe arguments, unavailable driver/program, normal return, exit request and child failure.
- `cargo build -p mcs`; `python3 scripts/check_tui_pty.py`: F3 text/search/close, sidebar/filter/resize, q/F10/Ctrl-C and terminal restoration.
- `python3 scripts/check_media_pty.py`: simulated positive Kitty capability/cell-size/DSR replies; real PNG protocol encoding/owned-ID deletion; fake Poppler search/resize; fake mpv handoff/failure/repaint; final termios/cursor/alternate-screen restoration. These are protocol/lifecycle tests, not visual comparisons or decoder/playback acceptance.
- Live Kitty acceptance still required: PNG/JPEG/PDF with resize/zoom/pan/repeated open-close; PDF text search and corrupt/password cases; 60-second 720p/30 source playback, pause/seek/volume/close, child failure and normal shell restoration. Record actual viewport dimensions, versions/hardware, drop/sync statistics and CPU/memory. Install Kitty/Poppler/mpv only after explicit setup approval.
- Linux/SSH acceptance is deferred in BACKLOG.md. No DRM tests or screenshots were performed for this milestone.

### Retina viewport regression

Large terminal pixel areas must uniformly downsample the encoded raster rather than fail. Tests cover 5K/ultrawide/portrait/extreme dimensions, exact four-megapixel/4,096-dimension raster bounds, and the Kitty transmission's explicit cell-sized virtual placement. A bounded raster must retain the fitted on-screen size, without retransmission on repaint. User visual confirmation in Kitty remains necessary.


### Opening-performance experiments

The ignored `benchmark_pdf_open` experiment generates a fixed 48-line PDF, measures three sequential samples per profile (metadata, Poppler transfer/decode, crop/resize, Kitty encoding and Ratatui buffer preparation), and reports encoded bytes. Run debug and release separately without competing builds. It excludes actual terminal writes/compositing; do not interpret it as real-file open latency or a universal speedup. The ignored `pdf_ppm_matches_png_pixels` check compares actual Poppler P6 and PNG pixel buffers at equal raster size. Default tests additionally check no software upscaling, public PNM rejection, and explicit compression acknowledgement/fallback; the simulated media PTY checks compressed rendering and cleanup.


## Confirmed file-operation gates

Rust tests cover recursive copy/move/delete, symlink and broken-link preservation, no traversal during deletion, destination collisions including post-plan races, changed sources, root/parent/self-descendant rejection, FIFO rejection, bounded editor inputs, mode preservation, hardlink/symlink rejection, batch preflight, modal confirmation and save-conflict buffer retention. `scripts/check_operations_pty.py` verifies real F4–F8 execution only in a new temporary fixture, no writes before the applicable Enter/y confirmation, and terminal restoration. This explicitly permits mutation of these uniquely owned fixtures; existing user files are untouched. Linux execution, live Kitty inspection, disk-full recovery and adversarial concurrent-writer stress remain pending.
