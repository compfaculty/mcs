#!/usr/bin/env python3
"""Exercise real pane shells in an exclusively owned fixture; no user commands/files."""
import re
import tempfile
from check_tui_pty import (BINARY, drain, fcntl, os, pty, signal, struct,
                           subprocess, termios, time)

TOGGLE = b"\x14"
F10 = b"\x1b[21~"


def run(shell="/bin/sh"):
    fixture = tempfile.mkdtemp(prefix="mcs-shell-")
    os.mkdir(os.path.join(fixture, "child"))
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
    process = subprocess.Popen(
        [str(BINARY), "--backend", "tui"], cwd=fixture,
        stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        env={**os.environ, "TERM": "xterm-256color", "SHELL": shell,
             "HOME": fixture, "ENV": "/dev/null", "PS1": "TEST> ",
             "MCS_TEST_EXECUTABLE": str(BINARY), "ZDOTDIR": fixture,
             "HISTFILE": "/dev/null"},
    )
    all_output = bytearray()
    shell_pids = []

    def receive(seconds=0.35):
        data = drain(master, seconds)
        all_output.extend(data)
        return data

    def send(data, seconds=0.35):
        os.write(master, data)
        return receive(seconds)

    def command(data, expected):
        output = send(data + b"\r", 0.55)
        assert expected in output, (expected, output)
        return output

    try:
        deadline = time.monotonic() + 5
        while b"MCS" not in all_output and time.monotonic() < deadline:
            receive(0.2)
        assert b"MCS" in all_output, all_output
        reserved = send(b"\x1b[23~")
        assert b"reserved" in reserved, reserved
        output = send(TOGGLE, 0.7)
        assert b"Shell" in output, output
        if shell == "/usr/bin/false":
            assert b"Shell exited" in output, output
            assert process.poll() is None
            send(b"q")
        elif shell.startswith("/nonexistent/"):
            assert b"/nonexistent/mcs-shell" in output, output
            send(TOGGLE)
            send(b"q")
        else:
            send(b"\x1b[23~")  # Unassigned F11 must not hide the shell.
            output = command(b"printf 'PID:%s\\n' $$", b"PID:")
            shell_pids.append(int(re.search(rb"PID:(\d+)", output)[1]))
            command(b"\"$MCS_TEST_EXECUTABLE\"; printf 'NESTED:%s\\n' $?", b"NESTED:0")
            command(b"env -u MCS_EMBEDDED_SHELL \"$MCS_TEST_EXECUTABLE\"; printf 'GUARD:%s\\n' $?", b"GUARD:0")
            command(b"MCS_VALUE=retained; cd child; printf 'READY_OK\\n'", b"READY_OK")
            send(TOGGLE)
            # Original listing restored, retained shell must not navigate browser.
            assert process.poll() is None
            send(TOGGLE)
            command(b"printf 'STATE:%s\\n' $MCS_VALUE", b"STATE:retained")
            command(b"printf 'DIR:%s\\n' ${PWD##*/}", b"DIR:child")
            # Running commands continue while hidden.
            send(b"sleep 1; printf 'ASYNC_DONE\\n'\r", 0.1)
            send(TOGGLE, 1.3)
            output = send(TOGGLE)
            assert b"ASYNC_DONE" in output, output
            send(b"sleep 30\r", 0.2)
            send(b"\x03")  # interrupts foreground command, not MCS
            command(b"printf 'INTERRUPTED_OK\\n'", b"INTERRUPTED_OK")
            assert process.poll() is None
            # Ctrl-] changes focus; Tab is passed through for shell completion.
            send(b"\x1d")
            send(TOGGLE, 0.6)
            output = command(b"printf 'PID:%s\\n' $$", b"PID:")
            shell_pids.append(int(re.search(rb"PID:(\d+)", output)[1]))
            assert shell_pids[0] != shell_pids[1]
            command(b"printf 'OTHER:%s:end\\n' $MCS_VALUE", b"OTHER::end")
            command(b"printf 'DIR:%s\\n' ${PWD##*/}", b"DIR:" + os.path.basename(fixture).encode())
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 140, 0, 0))
            os.kill(process.pid, signal.SIGWINCH)
            receive()
            command(b"printf 'SIZE:%sx%s\\n' $(stty size)", b"SIZE:24x68")
            send(b"\x1d")
            command(b"printf 'BACK:%s\\n' $MCS_VALUE", b"BACK:retained")
            send(b"exit\r", 0.7)
            assert process.poll() is None
            send(TOGGLE, 0.5)
            command(b"printf 'NEW:%s:end\\n' $MCS_VALUE", b"NEW::end")
            send(F10)
        assert process.wait(timeout=5) == 0
        receive()
        assert termios.tcgetattr(slave) == before, "termios not restored"
        assert b"\x1b[?1049l" in all_output and b"\x1b[?25h" in all_output
        for pid in shell_pids:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                continue
            raise AssertionError(f"shell {pid} survived exit")
        print(f"PASS pane shell lifecycle/restore ({shell}); fixture {fixture}")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    run()
    if os.path.exists("/bin/zsh"):
        run("/bin/zsh")
    run("/usr/bin/false")
    run("/nonexistent/mcs-shell")
