#!/usr/bin/env python3
"""Read-only macOS/Linux PTY checks. Run after `cargo build -p mcs`."""
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import termios
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/debug/mcs"


def drain(master, seconds=0.15):
    output = bytearray()
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if select.select([master], [], [], max(0, deadline - time.monotonic()))[0]:
            output.extend(os.read(master, 65536))
    return bytes(output)


def check_exit(name, exit_key):
    print(f"Checking {name}", flush=True)
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    process = None
    try:
        process = subprocess.Popen(
            [str(BINARY), "--backend", "tui"],
            cwd=ROOT, stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, "TERM": "xterm-256color"},
            start_new_session=True,  # Avoid inheriting the invoking shell's controlling terminal.
        )
        print("PTY child started", flush=True)
        output = bytearray()
        deadline = time.monotonic() + 5
        while b"MCS" not in output and time.monotonic() < deadline:
            output.extend(drain(master))
        assert b"MCS" in output, "TUI did not render"
        output.extend(drain(master, 0.3))  # Allow initial asynchronous scans to commit.
        for action in [b"\t", b"\r", b"\x7f", b" ", b"s", b".", b"[", b"]", b"B", b"b"]:
            os.write(master, action)
            output.extend(drain(master))
        assert b"Bookmarks" in output, "Bookmark sidebar did not render"
        os.write(master, b"\x1b")
        output.extend(drain(master, 0.3))
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
        os.kill(process.pid, signal.SIGWINCH)
        output.extend(drain(master))
        os.write(master, b"/q")  # q must be text in filter mode, not an exit action.
        output.extend(drain(master))
        assert process.poll() is None, "Filter text unexpectedly exited the TUI"
        if name != "Ctrl-C in filter":
            os.write(master, b"\r")
            output.extend(drain(master))
        os.write(master, exit_key)
        assert process.wait(timeout=5) == 0, "Nonzero exit"
        output.extend(drain(master))
        assert termios.tcgetattr(slave) == before, "Terminal modes not restored"
        assert b"\x1b[?1049l" in output, "Alternate screen not left"
        assert b"\x1b[?25h" in output, "Cursor not restored"
        print(f"PASS {name}: 80x24 -> 100x30, input/sidebar/filter, termios/screen/cursor restored")
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        os.close(master)
        os.close(slave)


def check_cli():
    for args, expected in [(["--help"], 0), (["--backend"], 2), (["--unknown"], 2),
                           (["--backend", "native"], 1)]:
        result = subprocess.run([str(BINARY), *args], capture_output=True, timeout=5, check=False)
        assert result.returncode == expected, (args, result.returncode)
        assert b"\x1b[?1049h" not in result.stdout + result.stderr, "CLI unexpectedly entered TUI"
    print("PASS help, malformed CLI, and unsupported native backend without terminal entry")


if __name__ == "__main__":
    check_cli()
    check_exit("q", b"q")
    check_exit("F10", b"\x1b[21~")
    check_exit("Ctrl-C in filter", b"\x03")
