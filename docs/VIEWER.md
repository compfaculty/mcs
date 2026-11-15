# macOS F3 viewer

F3 or `v` opens the highlighted local file fullscreen. Browser pane paths, selection, marks, filters and history remain in place. Esc/q/F3 closes; F10/Ctrl-C exits the application. During a running operation, Esc cancels first; press again to close. While entering a search, Esc cancels input and q is literal text. No file editing, copy/move/delete or GUI fallback exists.

## Text

- Up/Down or j/k scroll; PageUp/PageDown move one viewport; Home/End seek to the beginning/end.
- Left/Right scroll horizontally, `w` toggles wrapping, `l` toggles line numbers, `r` reloads.
- `/` enters a case-sensitive literal UTF-8 query (256 characters maximum); Enter searches, n/N finds next/previous, wrapping once. Search is asynchronous with progress and cancellation. Matches in visible rows are highlighted.
- Seek-based buffered reads support multi-gigabyte files without mmap or whole-file allocation. Individual long-line segments are at most 64 KiB; `+` beside a line number indicates a continuing segment. A giant physical line can therefore occupy multiple navigation segments even when wrapping is off. Horizontal scrolling applies within a segment.
- Visible text is bounded to 256 rows and 1,024 cells per row; rendered combining-character sequences also have a byte cap. Bounded pages are well within a 64 MiB text-cache budget. The sparse line index has a separate 16 MiB cap and doubles its checkpoint stride when compacted. Unknown line numbers show `?` while indexing catches up.
- UTF-8 BOM and CRLF are supported; invalid bytes are shown as replacement characters with a warning. Binary/UTF-16 detection examines the first 8 KiB for NUL; it is a heuristic, not encoding detection. Later controls are escaped. UTF-16 conversion, syntax highlighting, hex and live following remain deferred.
- Reads use the length observed at open. Detected size/mtime changes invalidate the view and require reload; this is not an immutable snapshot, and same-size changes with preserved timestamps may escape detection. Reload incorporates appended content.

## PNG/JPEG and PDF

Kitty support is actively queried once at startup (300 ms overall deadline, 4 KiB reply cap) before event processing. Main-thread Mio readiness polling leaves no detached stdin reader after timeout. Graphics require a positive Kitty response and usable cell metrics; terminal multiplexers are deliberately unsupported for media. Other terminals keep all text/navigation functionality and show a media-unavailable error. Kitty is not installed automatically.

- Images: `+`/`-` zoom; `0` resets fit/pan; arrows pan. Resize recomputes a fit-based viewport in the worker.
- PDF: PageUp/PageDown or `[`/`]` change pages; Home/End first/last. Image zoom/pan controls also apply. `/` searches extracted text; n/N wraps through matching pages. Results navigate to the page and display a text snippet, without graphical match boxes. No OCR or password UI.
- PDF requires separately installed Poppler tools `pdfinfo`, `pdftoppm`, `pdftotext` on PATH. Missing tools show a setup error. Helpers receive distinct typed arguments and an absolute path; no shell is invoked.
- Image input limit: 64 MiB encoded, 16 megapixels, dimensions no larger than 16,384, decoded source at most 64 MiB. Only PNG/JPEG are accepted as browser image inputs. The PNM codec is enabled exclusively for bounded internal Poppler RGB transfer. Encoded reads remain bounded if a source grows after stat.
- PDF input: 128 MiB and 100,000 pages maximum. Each page is rendered at up to 2,048 pixels on its longest edge; zoom magnifies that raster. One decoded page is cached (within the planned maximum of three). Each helper has a 10-second timeout, 64 KiB stderr cap, 64 MiB render-output cap, and 1 MiB extracted-text cap per page. Exceeding a text cap is an error, never a silent no-match.
- Encoded graphics rasters are capped at four megapixels and 4,096 pixels per dimension. Larger/Retina terminal areas are downsampled uniformly within that budget, while explicit Kitty placement dimensions preserve the fitted on-screen size and aspect ratio. The status line indicates budget scaling. Source/crop/viewport ownership is bounded, with a 128 MiB decoded/cache budget; protocol encodings and codec/resize scratch allocations are additional bounded work. This is not a process-RSS or native-parser memory guarantee.
- Decoding, crop/resize and Kitty encoding run off the input/render thread. Only owned image IDs are deleted on replacement, close, suspend or cleanup. Untrusted paths/errors/snippets are escaped before display.

## Video handoff

Supported routed extensions: MP4, MKV, MOV, WebM, M4V, AVI. F3 prepares optional `mpv` asynchronously, verifies Kitty output availability, and writes a fixed input configuration inside a private temporary directory. MCS restores terminal modes and stops reading/rendering before spawning mpv. On return it reinitializes and fully repaints, including after child failure. Source files and user mpv configuration are never changed.

The output-driver probe allows up to 15 seconds for cold mpv startup, with cancellation and bounded output. Probe errors identify mpv explicitly; this startup deadline does not limit playback duration.

mpv owns fullscreen Kitty playback: Space pauses, Left/Right seeks, Up/Down changes volume, m mutes, Esc/q returns. F10/Ctrl-C requests application exit through cleanup. No MCS chrome is drawn during playback. A child failure leaves a closable error view with browser state preserved.

Configuration/scripts, automatic file loading, playlist/reference expansion, default/builtin key bindings and resume writes are disabled. Only the fixed playback controls are bound, so screenshot/write commands are absent. Logging is quiet to avoid raw filename escape injection. FFmpeg/lavf is selected with a file-only protocol whitelist. These settings are defense in depth, not an OS sandbox; native media parsers remain an untrusted-data boundary. No network URLs or user-supplied player arguments are accepted.

The target is a measured 60-second 720p/30 proof of concept, not a guarantee. Record Kitty/mpv versions, hardware, actual displayed dimensions, elapsed duration, dropped frames, available A/V-sync counters, CPU and peak memory. Pixel fit can reduce the displayed resolution. Real playback/performance acceptance remains pending; installed mpv 0.42.0 reports Kitty output support, while fake-player tests verify only lifecycle and arguments.

## Dependencies and verification

- `ratatui-image` 11.1.0 (MIT), official ratatui/ratatui-image project; compatible with existing Ratatui 0.30. Default features disabled, only Crossterm enabled: no Chafa/system-library dependency. The crate still brings pure-Rust Sixel encoding dependencies, but MCS enables only Kitty at runtime.
- `image` 0.25.10 (MIT OR Apache-2.0), image-rs/image; PNG/JPEG features for browser images, plus PNM for internal lossless Poppler transfer. Requires Rust 1.88 or later; the workspace uses stable Rust. Decoder allocation limits are not an OS memory sandbox.
- `tempfile` 3.27.0 (MIT OR Apache-2.0), Stebalien/tempfile; private temporary playback config ownership/cleanup.
- Unix probe readiness uses existing locked `mio` 1.2.5 (MIT), tokio-rs/mio; no new package version or system library.
- Direct text dependencies: `memchr` 2.8.3 (Unlicense OR MIT), `unicode-width` 0.2.3 and Unix `libc` 0.2.191 (MIT OR Apache-2.0). libc is used for the O_NONBLOCK constant; no unsafe block/FFI was added. `crc32fast` is a test-only dependency for a valid oversized PNG header fixture.
- Downloaded host dependency manifests/build configuration were reviewed for license alternatives, portability and build requirements; all new host crates offer a permissive license alternative. `self_cell` offers Apache-2.0 OR GPL-2.0-only; the Apache alternative is applicable. This is not a vulnerability-advisory audit. Poppler/mpv remain external programs, not linked or bundled; reassess distribution licenses before packaging them.

Sources: https://docs.rs/ratatui-image/11.1.0/ratatui_image/ ; https://docs.rs/image/0.25.10/image/ ; https://poppler.freedesktop.org/ ; https://mpv.io/manual/stable/#video-output-drivers ; https://sw.kovidgoyal.net/kitty/graphics-protocol/

Run the normal Rust gates, `cargo build -p mcs`, `python3 scripts/check_tui_pty.py`, and `python3 scripts/check_media_pty.py`. The second script simulates Kitty capability/DSR replies plus Poppler/mpv and asserts protocol cleanup, helper routing and terminal restoration; it is explicitly not visual/media-performance acceptance. All fixtures use exclusively owned temporary paths. Real Kitty acceptance is a separate required check.

## Opening performance

Run `cargo run --release -p mcs -- --backend tui` for normal media viewing. The debug build retains unoptimized code for debugging.

PDF pages now travel from Poppler as bounded binary RGB (P6), avoiding PNG compression followed by immediate decompression. Size/dimension/allocation/time/cancellation checks remain in force; this is an internal pipe format, not additional public file-format support. Software rendering never enlarges a raster beyond its source crop: Kitty scales the virtual placement while preserving on-screen size. Only actual downsampling incurs a software resize.

At startup, a separate positive compression reply enables ratatui-image's lossless zlib transport. Without that acknowledgement, use raw transport. Compression adds CPU work and benefits documents/screenshots most; photo-heavy input can benefit less. No shared-memory transport or global file cache was introduced.

Reproduce local experiments with `cargo test -p mcs-tui benchmark_pdf_open -- --ignored --nocapture`, the same command with `--release`, and `cargo test -p mcs-tui pdf_ppm_matches_png_pixels -- --ignored`. These explicitly require Poppler and use a generated fixed-text PDF, not user documents. Default tests skip both experiments. Recorded observations and caveats are in STATUS.md.

PTY tests accept `MCS_TEST_BINARY=target/release/mcs` to check the release executable as well as the default debug executable.
