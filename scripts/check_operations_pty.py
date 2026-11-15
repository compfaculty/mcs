#!/usr/bin/env python3
"""Exercise confirmed mutations exclusively inside a newly owned temporary fixture."""
import fcntl
import os
import pty
import struct
import subprocess
import tempfile
import termios
import time
from pathlib import Path
from check_tui_pty import BINARY, drain


def main():
    root = Path(tempfile.mkdtemp(prefix="mcs-operations-pty-"))
    source = root / "a.txt"
    source.write_text("original", encoding="utf-8")
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
    process = subprocess.Popen([str(BINARY), "--backend", "tui"], cwd=root,
                               stdin=slave, stdout=slave, stderr=slave,
                               env={**os.environ, "TERM": "xterm-256color", "MCS_ICONS": "never"},
                               start_new_session=True)
    output = bytearray()

    def send(data, seconds=0.3):
        os.write(master, data)
        output.extend(drain(master, seconds))

    def wait_for(predicate):
        deadline = time.monotonic() + 5
        while not predicate():
            assert time.monotonic() < deadline, "Operation timed out"
            output.extend(drain(master, 0.1))

    def target(function, name):
        send(function)
        send(b"\x15" + str(root / name).encode())  # Ctrl-U replaces default.
        assert not (root / name).exists(), "Mutation occurred before Enter confirmation"
        send(b"\r")  # The labeled action executes directly; no extra y.
        wait_for(lambda: (root / name).exists())
        output.extend(drain(master, 0.3))  # Completion returns directly to browser.

    try:
        output.extend(drain(master, 0.9))
        send(b"\x1bOS")  # F4
        send(b"X")
        assert source.read_text() == "original", "Editing wrote before save confirmation"
        send(b"\x13")  # Ctrl-S reviews save
        assert source.read_text() == "original"
        send(b"y")
        wait_for(lambda: source.read_text() == "Xoriginal")
        output.extend(drain(master, 0.3))
        target(b"\x1b[15~", "copied.txt")  # F5
        assert (root / "copied.txt").read_text() == "Xoriginal"
        target(b"\x1b[17~", "moved.txt")  # F6
        assert not source.exists()
        target(b"\x1b[18~", "new-dir")  # F7
        assert (root / "new-dir").is_dir()
        send(b"/moved.txt\r")
        offset = len(output)
        send(b"\x1b[19~")  # F8
        wait_for(lambda: b"n/Esc cancel (default)" in output[offset:])
        assert (root / "moved.txt").exists()
        send(b"\r")  # Enter is deliberately not confirmation.
        assert (root / "moved.txt").exists()
        send(b"y")
        wait_for(lambda: not (root / "moved.txt").exists())
        output.extend(drain(master, 0.3))
        send(b"q")  # No result acknowledgement: browser quit must work immediately.
        assert process.wait(timeout=5) == 0
        assert termios.tcgetattr(slave) == before
        assert b"\x1b[?1049l" in output and b"\x1b[?25h" in output
        print(f"PASS F4-F8: confirmed edit/copy/move/mkdir/delete, explicit confirmations, no success acknowledgement; terminal restored; fixture retained at {root}")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    main()
