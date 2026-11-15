#!/usr/bin/env python3
"""Verify global singleton, simultaneous launches, crash release and silent nested ban."""
import concurrent.futures
import tempfile
from check_tui_pty import (BINARY, drain, fcntl, os, pty, struct,
                           subprocess, termios, time)

ENV = {key: value for key, value in os.environ.items() if key != "MCS_EMBEDDED_SHELL"}
ENV["TERM"] = "xterm-256color"


def start(cwd):
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
    child = subprocess.Popen([str(BINARY)], stdin=slave, stdout=slave, stderr=slave,
                             cwd=cwd, env=ENV, start_new_session=True)
    return child, master, slave, before


def wait_render(child, master):
    output = bytearray()
    deadline = time.monotonic() + 5
    while b"MCS" not in output and time.monotonic() < deadline:
        output.extend(drain(master, 0.1))
    assert b"MCS" in output and child.poll() is None, (child.poll(), output)


def ignored(cwd, args=(), env=None):
    result = subprocess.run([str(BINARY), *args], cwd=cwd, env=env or ENV,
                            stdin=subprocess.DEVNULL, capture_output=True, timeout=3)
    assert result.returncode == 0 and not result.stdout and not result.stderr, result


def close(entry):
    child, master, slave, _ = entry
    if child.poll() is None:
        child.kill()
        child.wait(timeout=5)
    os.close(master)
    os.close(slave)


def run():
    fixture = tempfile.mkdtemp(prefix="mcs-instance-pty-")
    elsewhere = tempfile.mkdtemp(prefix="mcs-instance-other-")
    # Marker ban applies even without an existing parent/lock.
    ignored(fixture, env={**ENV, "MCS_EMBEDDED_SHELL": "1"})
    entry = start(fixture)
    child, master, slave, before = entry
    try:
        wait_render(child, master)
        for args in [(), ("--backend", "tui"), ("--help",), ("--unknown",)]:
            ignored(elsewhere, args, {**ENV, "TMPDIR": elsewhere, "HOME": elsewhere})
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            list(pool.map(lambda _: ignored(elsewhere), range(8)))
        assert child.poll() is None
        os.write(master, b"q")
        assert child.wait(timeout=5) == 0
        output = drain(master)
        assert termios.tcgetattr(slave) == before
        assert b"\x1b[?1049l" in output
    finally:
        close(entry)
    # The persistent inode is reusable after normal exit, and a crash leaves no stale lock.
    entry = start(elsewhere)
    try:
        wait_render(entry[0], entry[1])
        entry[0].kill()
        entry[0].wait(timeout=5)
    finally:
        close(entry)
    # Simultaneous starts must have exactly one winner.
    entries = [start(fixture), start(elsewhere)]
    try:
        deadline = time.monotonic() + 5
        while all(item[0].poll() is None for item in entries) and time.monotonic() < deadline:
            time.sleep(0.02)
        winners = [entry for entry in entries if entry[0].poll() is None]
        losers = [entry for entry in entries if entry[0].poll() is not None]
        assert len(winners) == len(losers) == 1
        loser, loser_master, loser_slave, loser_before = losers[0]
        assert loser.returncode == 0 and drain(loser_master) == b""
        assert termios.tcgetattr(loser_slave) == loser_before
        winner, winner_master, winner_slave, winner_before = winners[0]
        wait_render(winner, winner_master)
        os.write(winner_master, b"q")
        assert winner.wait(timeout=5) == 0
        assert termios.tcgetattr(winner_slave) == winner_before
    finally:
        for entry in entries:
            close(entry)
    print("PASS singleton: silent duplicates across cwd/HOME/TMPDIR, simultaneous starts, normal/crash release, nested marker, terminal restore")


if __name__ == "__main__":
    run()
