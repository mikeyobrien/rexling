#!/usr/bin/env python3
"""Display a phone render in a Rex pane, with a truecolor fallback."""
import base64
import io
import os
from pathlib import Path
import re
import select
import signal
import sys
import termios
import time
import tty

from PIL import Image


VIEWS = {
    'a': ('A · Bottom sheet', (53, 188, 720, 1223)),
    'b': ('B · Above-keyboard tray', (765, 188, 1432, 1223)),
}


def emit(s):
    sys.stdout.write(s)
    sys.stdout.flush()


def kitty_supported(fd):
    # The base terminal answers this only if it implements the graphics protocol.
    emit('\x1b_Ga=q,i=7319,s=1,v=1,f=24;AAAA\x1b\\')
    response = b''
    until = time.monotonic() + 0.6
    while (remaining := until - time.monotonic()) > 0 and select.select([fd], [], [], remaining)[0]:
        response += os.read(fd, 256)
        if b'\x1b\\' in response:
            break
    return re.search(rb'\x1b_G[^;]*\bi=7319(?:,|;)[^\x1b]*', response) is not None and b';OK' in response


def show_kitty(image, width, height, left):
    png = io.BytesIO()
    image.save(png, format='PNG')
    data = base64.b64encode(png.getvalue()).decode()
    emit(f'\x1b[3;{left + 1}H')
    for offset in range(0, len(data), 4096):
        more = int(offset + 4096 < len(data))
        controls = f'a=T,f=100,q=2,C=1,c={width},r={height},i=7320,' if offset == 0 else ''
        emit(f'\x1b_G{controls}m={more};{data[offset:offset + 4096]}\x1b\\')


def show_blocks(image, width, rows, left):
    pixels = image.resize((width, rows * 2), Image.Resampling.LANCZOS).load()
    out = []
    for row in range(rows):
        out.append(f'\x1b[{row + 3};{left + 1}H')
        prev = None
        for col in range(width):
            pair = pixels[col, 2 * row], pixels[col, 2 * row + 1]
            if pair != prev:
                fg, bg = (';'.join(map(str, color)) for color in pair)
                out.append(f'\x1b[38;2;{fg};48;2;{bg}m')
                prev = pair
            out.append('▀')
        out.append('\x1b[0m')
    emit(''.join(out))


def main():
    label, crop = VIEWS[sys.argv[1]]
    image = Image.open(Path(__file__).with_name('comparison.png')).convert('RGB').crop(crop)
    fd = sys.stdin.fileno()
    original = termios.tcgetattr(fd)
    dirty, zoom, offset, kitty = True, False, 0, False

    def resize(*_):
        nonlocal dirty
        dirty = True

    signal.signal(signal.SIGWINCH, resize)
    try:
        tty.setcbreak(fd)
        emit('\x1b[?1049h\x1b[?25l')
        kitty = "--kitty" in sys.argv[2:] or kitty_supported(fd)
        while True:
            if dirty:
                cols, lines = os.get_terminal_size()
                available = max(1, lines - 4)
                ratio = image.width / image.height
                width = max(1, min(cols - 2, int(available * 2 * ratio)))
                if zoom:
                    width = max(1, cols - 2)
                full_rows = max(1, round(width / (2 * ratio)))
                offset = min(offset, max(0, full_rows - available))
                height = min(available, full_rows)
                lo, hi = (round(y * image.height / full_rows) for y in (offset, offset + height))
                piece = image.crop((0, lo, image.width, hi))
                if kitty:
                    emit('\x1b_Ga=d,d=i,i=7320\x1b\\')
                emit(f'\x1b[2J\x1b[1;1H{label[:cols]}')
                left = max(0, (cols - width) // 2)
                if kitty:
                    show_kitty(piece, width, height, left)
                else:
                    show_blocks(piece, width, height, left)
                hint = 'z: fit / detail   ↑↓: scroll   q: close preview'
                emit(f'\x1b[{lines};1H\x1b[2m{hint[:cols]}\x1b[0m')
                dirty = False
            if not select.select([fd], [], [], 0.2)[0]:
                continue
            key = os.read(fd, 32)
            if not key or key == b'q':
                break
            if key == b'z':
                zoom, offset = not zoom, 0
            elif key in (b'\x1b[A', b'k'):
                offset = max(0, offset - 4)
            elif key in (b'\x1b[B', b'j'):
                offset += 4
            dirty = True
    finally:
        if kitty:
            emit('\x1b_Ga=d,d=i,i=7320\x1b\\')
        emit('\x1b[0m\x1b[?25h\x1b[?1049l')
        termios.tcsetattr(fd, termios.TCSADRAIN, original)


if __name__ == '__main__':
    main()
