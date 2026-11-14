# Codex instructions for MCS

You are working on a console-first Rust project, not a desktop environment.

## First actions
1. Read README, ARCHITECTURE, MILESTONES, TEST_PLAN, SECURITY, and the current milestone status.
2. Run `cargo check --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Fix compile/setup issues first.
3. Perform only the explicitly requested milestone. Do not silently expand scope or swap the architecture.
4. Keep macOS development and Linux deployment separate; use cfg-gated modules and clearly named capability stubs.
5. Report changed files, commands actually run, their results, known limitations, and next actions. Update `docs/STATUS.md` with evidence.

## Non-negotiable design constraints
- `mcs` is launched by a user from a normal shell and exits back to it.
- No X11/Wayland/desktop environment is required for the *eventual local Linux native backend*.
- macOS TUI must function without Linux-specific libraries.
- SSH terminal backend stays available even if native Linux graphics is added.
- No auto-start, system-wide shell replacement, sudo-in-app, or privileged daemon without explicit user approval.
- Video and PDF are *roadmap features*, not already implemented.
- Avoid implementing video encoding, PDF parsing, GPU drivers or DRM from scratch when mature components can be used.
- Avoid `unsafe` unless necessary at a documented FFI/platform boundary; isolate it, test it, and document ownership/lifetimes.
- Do not claim a terminal image protocol is supported by the Linux kernel virtual console.
- Never claim `cargo check`, screenshot comparisons, DRM tests, or media tests passed unless actually run.

## Quality gates
- No regressions in the text-only frontend.
- Format, clippy, tests, manual terminal-restore check on each significant change.
- Dependency changes require source, license, portability and maintenance review.
- Changes to Linux DRM require test on an actual local seat and a safe way to recover the VT.
- File mutations require explicit user confirmation and path safety checks.

## Agent roles (logical roles, not mandatory autonomous subagents)
- Architect: ADRs, boundaries, interfaces; propose changes before sweeping refactor.
- Rust implementer: incremental clean code, cargo tests.
- Platform engineer: macOS-vs-Linux isolation, DRM/logind, input, audio.
- QA/reviewer: error paths, VT restoration, accessibility, perf, regression.
- Security reviewer: untrusted paths/files, command injection, media parser isolation, privilege boundaries.

## Task format
Read `prompts/01-start.md`. Deliver one milestone at a time with explicit acceptance evidence.
