# Ubuntu Server deployment — no GUI required

## Recommended workflow

Develop/test on Mac. Commit to Git. On future Ubuntu Server, `git clone`/`git pull`, `cargo build --release`, then run `./target/release/mcs --backend tui`. Alternatively build a Linux binary in Linux CI/container for the correct architecture and libc; copy via `scp` (do not copy a macOS Mach-O binary to Linux).

On Ubuntu Server text mode:
```
sudo apt update
sudo apt install -y openssh-server git curl build-essential pkg-config ca-certificates
# Install Rust for your own account from the official rustup instructions at https://rustup.rs/
# On server, clone repo and:
cargo build --release -p mcs
./target/release/mcs --backend tui
```

SSH remote development: connect from Mac `ssh user@ubuntu-host`; edit locally with RustRover, send commits, or use `rsync`/`scp` and run checks remotely. IDE need not be installed on the Ubuntu host. Running `codex` CLI on Ubuntu is optional (requires connectivity/auth and supported binary), not a prerequisite.

## Linux Native DRM later

`mcs --backend drm` intentionally reports unsupported today. This requires a **local physical seat** with usable DRM GPU and display (not just SSH), carefully managed `/dev/dri` permissions through a session manager, input devices, VT handling, and rendering. Do NOT `chmod 666 /dev/dri/*`, start as root by default, or assume remote SSH owns the active VT.

Test only on a spare test session / recoverable host with physical access; start with a read-only demo. SSH is useful as an emergency recovery channel, not as a DRM image transport. Use `/dev/dri` discovery and logs; don't install drivers blindly.

For Linux-only continuous testing without laptop: use an Ubuntu container or GitHub Actions to compile and test **TUI/core** on Linux; real DRM/VT testing still requires a relevant GPU/seat environment.
