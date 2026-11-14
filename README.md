# MCS — Multimedia Console System

**Status:** M1 read-only navigation MVP, verified on macOS with tool-provided PTYs. Linux/SSH and Terminal.app/Kitty acceptance remain pending. This is not a multimedia player, DRM frontend, or production file manager.

MCS is an opt-in console-first Rust application. Run `mcs` from a normal shell, use an MC-inspired two-pane file browser, then exit back to the same shell. The text frontend needs no desktop environment or Linux graphics libraries.

## Run

Install a stable Rust toolchain and open the root workspace in your editor, then:

```sh
cargo run -p mcs -- --backend tui
```

`--backend auto` also selects TUI. `--help` prints usage without entering terminal mode. Native/DRM requests explicitly report unsupported; malformed arguments exit with usage.

## Controls

- Up/Down or j/k: highlight; PageUp/PageDown: move ten rows; Home/End: first/last.
- Tab: switch independent panes. Enter: enter directory (including directory symlinks). Backspace/Left: parent.
- `[` / `]`: back/forward history, restoring highlighted paths. `r`: refresh.
- Space/Insert: mark/unmark and advance. Marks are informational; no file operations exist.
- `s`: name ascending/descending, always directories first. `.`: show/hide dotfiles (hidden initially).
- `/`: edit a case-insensitive substring filter; Enter accepts; Esc cancels editing. Esc outside editing clears the filter. Filter text is limited to 256 characters.
- `B`: add/remove the current directory bookmark. `b`: bookmark sidebar; Up/Down selects, Enter visits, Esc closes.
- `q`, F10, or Ctrl-C: exit. While filtering, q is literal text; F10 and Ctrl-C still exit.

History and bookmarks last for the session only (128 history locations per direction per pane; 32 shared bookmarks). Marks survive filtering and refresh but clear when changing directories. Filters, sort order and hidden-file settings belong to each pane and remain active across navigation.

## Behavior and limits

Directory enumeration and name sorting run in one worker per pane. Only one scan per pane is allowed at a time; navigation requests during loading are ignored, while pane switching, input and exit remain available. Failed visits preserve the prior location and history. Unavailable individual entries produce a warning; broken symlinks remain visible with an explanation. Control characters and bidi overrides are escaped for display, while filesystem operations use original paths.

Listings are capped at 100,000 entries, including hidden entries. Exceeding the cap rejects the listing, preserving the previous one. Sorting costs O(n log n); filtering/view-index updates cost O(n), with O(n) listing storage. Filesystem calls cannot be forcibly cancelled; a hung mount can keep that pane loading until exit. Bookmarks are not persisted and file contents are not previewed.

**No copy, move, delete, subprocess launching, image/PDF/video playback, or native graphics is implemented.**

## Verification

```sh
cargo check --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p mcs
python3 scripts/check_tui_pty.py
```

The optional Python standard-library PTY check runs on macOS/Linux and checks resize, input modes, q/F10/Ctrl-C exit, restored terminal settings/cursor/screen, CLI errors, and the native stub. It has been executed on macOS only. Rust tests create uniquely owned temporary fixtures under the system temp directory and leave them there; they never delete or overwrite existing user files. The large-directory test creates 20,000 empty fixture files.

See [status and evidence](docs/STATUS.md), [architecture](docs/ARCHITECTURE.md), [milestones](docs/MILESTONES.md), [test plan](docs/TEST_PLAN.md), [security](docs/SECURITY.md), and [Linux deployment](docs/LINUX_DEPLOYMENT.md). M2/media and Linux DRM require separate milestone approval and appropriate hardware validation.
