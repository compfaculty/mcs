#!/usr/bin/env python3
"""Simulated Kitty/Poppler/mpv integration; NOT visual or playback performance acceptance."""
import binascii
import fcntl
import os
from pathlib import Path
import pty
import re
import signal
import struct
import subprocess
import tempfile
import termios
import time
import zlib
from check_tui_pty import BINARY, drain


def png():
    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', binascii.crc32(kind + data) & 0xffffffff)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!IIBBBBB', 10, 10, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress((b'\0' + b'\xff\0\0' * 10) * 10)) + chunk(b'IEND', b'')


def script(path, body):
    path.write_text('#!/bin/sh\n' + body + '\n')
    path.chmod(0o700)


def wait_for(master, needle, seconds=6, initial=b''):
    output = bytearray(initial)
    deadline = time.monotonic() + seconds
    while needle not in output and time.monotonic() < deadline:
        chunk = drain(master, 0.05)
        output.extend(chunk)
        if b'\x1b[6n' in chunk:
            os.write(master, b'\x1b[1;1R')
    assert needle in output, (needle, bytes(output[-400:]))
    return bytes(output)


def main():
    # Fixtures intentionally retained; only uniquely owned temporary paths are written.
    root = Path(tempfile.mkdtemp(prefix='mcs-media-pty-'))
    tools = root / 'tools'
    tools.mkdir()
    (root / 'image.png').write_bytes(png())
    (root / 'image.ppm').write_bytes(b'P6\n10 10\n255\n' + b'\xff\0\0' * 100)
    (root / 'paper.pdf').write_bytes(b'%PDF-fake integration fixture')
    (root / 'play.mp4').write_bytes(b'fake video')
    (root / 'fail.mp4').write_bytes(b'fake video')
    script(tools / 'pdfinfo', "printf 'Pages: 3\\n'")
    script(tools / 'pdftoppm', 'cat "$(dirname "$0")/../image.ppm"')
    script(tools / 'pdftotext', 'if [ "$2" = 2 ]; then printf "page two needle"; else printf "other page"; fi')
    script(tools / 'mpv', '''if [ "$2" = '--vo=help' ]; then printf ' kitty terminal output\\n'; exit 0; fi
case "$*" in *fail.mp4*) exit 1;; esac
printf 'FAKE_MPV_OWNS_TTY\\n'
exit 0''')
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
    process = subprocess.Popen([str(BINARY), '--backend', 'tui'], cwd=root, stdin=slave, stdout=slave, stderr=slave,
                             start_new_session=True, env={**os.environ, 'TERM': 'xterm-kitty', 'PATH': str(tools) + os.pathsep + os.environ['PATH']})
    try:
        wait_for(master, b'\x1b[5n')
        os.write(master, b'\x1b_Gi=31;OK\x1b\\\x1b_Gi=32;OK\x1b\\\x1b[6;20;10t\x1b[0n')
        wait_for(master, b'MCS')
        drain(master, 0.3)
        def open_file(name):
            os.write(master, b'\x1b')
            drain(master, 0.3)
            os.write(master, b'/' + name.encode() + b'\r')
            drain(master, 0.1)
            os.write(master, b'v')
        open_file('image.png')
        first = wait_for(master, b'a=T,U=1')
        assert b'o=z,' in first, 'Confirmed compression was not used'
        image_id = re.search(rb'i=(\d+),a=T', first).group(1)
        os.write(master, b'+')
        changed = wait_for(master, b'a=T,U=1')
        assert b'd=I,i=' + image_id in changed, 'Old image was not deleted on replacement'
        os.write(master, b'q')
        closed = wait_for(master, b'F4-F8 files')
        assert b'd=I,i=' in closed and b'd=A' not in closed, 'Owned placement cleanup missing'
        print('PASS simulated Kitty image encode, zoom, owned-ID cleanup and browser return', flush=True)
        open_file('paper.pdf')
        wait_for(master, b'a=T,U=1')
        os.write(master, b'/needle\r')
        wait_for(master, b'a=T,U=1')
        os.write(master, b'n')
        wait_for(master, b'a=T,U=1')
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 100, 0, 0))
        os.kill(process.pid, signal.SIGWINCH)
        wait_for(master, b'a=T,U=1')
        os.write(master, b'q')
        wait_for(master, b'F4-F8 files')
        print('PASS fake Poppler page render/search, resize and cleanup', flush=True)
        open_file('play.mp4')
        played = wait_for(master, b'FAKE_MPV_OWNS_TTY')
        # Preserve a redraw already read with the player marker. Ratatui can skip
        # blank cells with cursor moves, so match a word rather than a phrase.
        wait_for(master, b'F4-F8', initial=played)
        assert b'\x1b[?1049l' in played, 'MCS did not suspend alternate screen'
        open_file('fail.mp4')
        wait_for(master, b'failed')
        os.write(master, b'q')
        wait_for(master, b'F4-F8 files')
        os.write(master, b'q')
        assert process.wait(timeout=5) == 0
        output = drain(master)
        assert termios.tcgetattr(slave) == before
        assert b'\x1b[?1049l' in output and b'\x1b[?25h' in output
        print('PASS fake mpv handoff, child-failure recovery and final termios/cursor/screen restoration', flush=True)
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        os.close(master)
        os.close(slave)


if __name__ == '__main__':
    main()
