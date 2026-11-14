# macOS — setup for RustRover + Codex

Mandatory: macOS, Xcode Command Line Tools (`xcode-select --install`), stable Rust via https://rustup.rs/, RustRover via https://www.jetbrains.com/rust/. RustRover can also install Rustup for you. Check: `rustc --version; cargo --version`.

Recommended: Git (`git --version`); Kitty (`brew install --cask kitty`) for **later** image experiments; Codex CLI (`brew install --cask codex` OR `npm install -g @openai/codex`). Codex CLI login: `codex` -> sign in. These are optional for running M0.

Later M2 optional tools: `brew install mpv yt-dlp ffmpeg pkg-config` (verify current formulas and library linkage with the chosen `libmpv` crate). PDF renderer dependencies chosen during M2 (do not install everything now).

RustRover: File -> Open -> select extracted folder MCS_Starter_Kit; use stable toolchain. Open built-in terminal and run:

```
cargo check --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p mcs -- --backend tui
```

Run CLI `codex` from project root and paste `prompts/01-start.md`. Codex CLI is not guaranteed to have native RustRover UI integration; use the IDE terminal. Keep changes in Git.

macOS cannot directly test Linux DRM/KMS; macOS Unix PTYs and terminal media protocol are not substitutes for testing Linux VT and seat management.
