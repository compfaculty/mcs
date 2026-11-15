# MCS — Multimedia Console System

**Status:** macOS browser with confirmed F4–F8 file operations plus F3 text/image/PDF viewer and optional terminal mpv handoff. Rust and simulated PTY checks are available; real Kitty media and performance acceptance remain pending. Linux/SSH work is deferred. No native DRM backend exists.

MCS is an opt-in console-first Rust application. Run `mcs` from a normal shell, use an MC-inspired two-pane file browser, then exit back to the same shell. The text frontend needs no desktop environment or Linux graphics libraries.

## Run

Install a stable Rust toolchain and open the root workspace in your editor, then:

```sh
cargo run --release -p mcs -- --backend tui
```

Use the optimized release build for normal media viewing; the default debug build prioritizes debugging. `--backend auto` also selects TUI. `--help` prints usage without entering terminal mode. Native/DRM requests explicitly report unsupported; malformed arguments exit with usage.

## Controls

The bottom menu follows commander function-key positions: **F1 Help, F2 Actions, F3 View, F4 Edit, F5 Copy, F6 Move, F7 Mkdir, F8 Delete, F9 Menu, F10 Quit**. F2 and F9 currently open the same built-in read-only action menu (sort, hidden files, filter, bookmarks, refresh and icons); no user scripts are executed. Up/Down and Enter select an action, or press its displayed letter shortcut. Esc/q closes help or the menu. Help scrolls with Up/Down. F11/F12 remain unassigned. Wide windows show dimmed reserved tiles; narrow windows prioritize working actions without renumbering them. macOS may require Fn with function keys. These keys apply in the browser; filter/bookmark/viewer modes keep their contextual controls, with F10/Ctrl-C always available to exit.

- Up/Down or j/k: highlight; PageUp/PageDown: move ten rows; Home/End: first/last.
- Tab: switch independent panes. Enter: enter directory (including directory symlinks). Backspace/Left: parent.
- `[` / `]`: back/forward history, restoring highlighted paths. `r`: refresh.
- Space/Insert: mark/unmark and advance. F5/F6/F8 use marks, falling back to the highlighted entry.
- `s`: name ascending/descending, always directories first. `.`: show/hide dotfiles (hidden initially).
- `i`: toggle icons beside filenames and in the bottom shortcut tiles. Emoji default on for UTF-8 terminal locales, except Linux virtual consoles and dumb/unknown terminals. Set `MCS_ICONS=always` or `MCS_ICONS=never` to override startup detection. No special icon font or graphics protocol is required; font coverage and emoji width vary by terminal. ASCII file markers and text menu labels remain available. Categories are filename hints, not a guarantee that the viewer supports that format.
- `/`: edit a case-insensitive substring filter; Enter accepts; Esc cancels editing. Esc outside editing clears the filter. Filter text is limited to 256 characters.
- `B`: add/remove the current directory bookmark. `b`: bookmark sidebar; Up/Down selects, Enter visits, Esc closes.
- F4: UTF-8 editor (1 MiB); Ctrl-S reviews save, `y` confirms, Esc discards.
- F5/F6: copy/move; F7: mkdir; F8: permanent recursive delete. Enter confirms copy/move/mkdir in a compact centered dialog; delete requires `y`. Esc cancels. No overwrite; cross-filesystem moves are rejected. See [file operations and limits](docs/FILE_OPERATIONS.md).
- F3 or `v`: fullscreen viewer; Esc/q returns to the same browser state.
- `q`, F10, or Ctrl-C in the browser: exit. While filtering, q is literal text; F10 and Ctrl-C still exit.

History and bookmarks last for the session only (128 history locations per direction per pane; 32 shared bookmarks). Marks survive filtering and refresh but clear when changing directories. Filters, sort order and hidden-file settings belong to each pane and remain active across navigation.

## Behavior and limits

Directory enumeration and name sorting run in one worker per pane. Only one scan per pane is allowed at a time; navigation requests during loading are ignored, while pane switching, input and exit remain available. Failed visits preserve the prior location and history. Unavailable individual entries produce a warning; broken symlinks remain visible with an explanation. Control characters and bidi overrides are escaped for display, while filesystem operations use original paths.

Listings are capped at 100,000 entries, including hidden entries. Exceeding the cap rejects the listing, preserving the previous one. Sorting costs O(n log n); filtering/view-index updates cost O(n), with O(n) listing storage. Filesystem calls cannot be forcibly cancelled; a hung mount can keep that pane loading until exit. Bookmarks are not persisted. F3 opens the selected file in the viewer.

## F3 viewer

Text: arrows/PgUp/PgDn/Home/End navigate; `/` searches, n/N repeats; `w` wrap, `l` line numbers, `r` reload. Large files use bounded seek-based reads and background indexing/search. Invalid UTF-8 is displayed with a warning; binary/UTF-16 viewing is unsupported.

Kitty images/PDF: `+`/`-` zoom, `0` fit, arrows pan; PgUp/PgDn change PDF pages. PDF search navigates to matching pages with a snippet; no OCR. PNG/JPEG support is built in. PDF needs optional Poppler tools on PATH; video needs optional mpv with Kitty output. Tools are not auto-installed. Other terminals retain text viewing and report media unavailable.

Video hands the terminal to mpv: Space pause, arrows seek/volume, m mute, Esc/q return. F10/Ctrl-C exits MCS. Native decoding/player libraries are not implemented from scratch. Live media acceptance and 720p/30 measurements are pending.

See [viewer controls, limits and dependency review](docs/VIEWER.md). File mutations use separate confirmation dialogs. No external GUI fallback or native graphics is implemented.

## Verification

```sh
cargo check --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p mcs
python3 scripts/check_tui_pty.py
python3 scripts/check_media_pty.py
```

The optional Python standard-library PTY check runs on macOS/Linux and checks resize, input modes, q/F10/Ctrl-C exit, restored terminal settings/cursor/screen, CLI errors, and the native stub. It has been executed on macOS only. Rust tests create uniquely owned temporary fixtures under the system temp directory and leave them there; they never delete or overwrite existing user files. The large-directory test creates 20,000 empty fixture files.

See [status and evidence](docs/STATUS.md), [architecture](docs/ARCHITECTURE.md), [milestones](docs/MILESTONES.md), [test plan](docs/TEST_PLAN.md), [security](docs/SECURITY.md), and [Linux deployment](docs/LINUX_DEPLOYMENT.md). The approved macOS viewer work is tracked in status; [long-term backlog](docs/BACKLOG.md) includes Linux/SSH and DRM. Real media requires Kitty and optional external tools; installation remains separately authorized.
