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
