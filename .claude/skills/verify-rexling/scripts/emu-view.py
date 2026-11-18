#!/usr/bin/env python3
"""Mirror an Android device's screen into this terminal with Kitty graphics.

Usage: emu-view.py [serial] [interval-seconds]
View only; input still goes through adb (e.g. `adb shell input tap X Y`).

Fits the pane while keeping the device's aspect ratio, scales frames down to the
pane's pixel size before sending, and sends only frames that changed.
"""
import base64
import fcntl
import hashlib
import io
import os
import struct
import subprocess
import sys
import termios
import time

from PIL import Image

serial = sys.argv[1] if len(sys.argv) > 1 else "emulator-5554"
# Measurement scripts create this file so screen captures don't skew their timings.
PAUSE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../../rexling-android/.build/verify/viewer.pause")
interval = float(sys.argv[2]) if len(sys.argv) > 2 else 0.3
out = sys.stdout


def pane():
    """Columns, rows and cell size in pixels. Falls back to a 1:2 cell when pixels are unknown."""
    rows, cols, xpix, ypix = struct.unpack("HHHH", fcntl.ioctl(sys.stdout.fileno(), termios.TIOCGWINSZ, b"\0" * 8))
    cw = xpix / cols if xpix and cols else 10.0
    ch = ypix / rows if ypix and rows else 20.0
    return cols, rows, cw, ch


def frame():
    try:
        return subprocess.run(["adb", "-s", serial, "exec-out", "screencap", "-p"],
                              capture_output=True, timeout=10).stdout
    except (subprocess.SubprocessError, OSError):
        return b""


def fit(image, cols, rows, cw, ch):
    """Largest cell box with the image's aspect ratio inside cols × rows (last row is the status line)."""
    rows -= 1
    w, h = image.size
    scale = min(cols * cw / w, rows * ch / h)
    c = max(1, min(cols, int(w * scale / cw)))
    r = max(1, min(rows, int(h * scale / ch)))
    return c, r, (max(1, int(c * cw)), max(1, int(r * ch)))


def draw(png, last):
    cols, rows, cw, ch = pane()
    image = Image.open(io.BytesIO(png)).convert("RGB")
    c, r, pixels = fit(image, cols, rows, cw, ch)
    key = (hashlib.sha1(png).digest(), cols, rows)
    if key == last:
        return last
    buffer = io.BytesIO()
    image.resize(pixels, Image.LANCZOS).save(buffer, "PNG", optimize=False, compress_level=3)
    data = base64.b64encode(buffer.getvalue()).decode()
    left = (cols - c) // 2
    out.write(f"\033[2J\033[1;{left + 1}H")
    chunks = [data[i:i + 4096] for i in range(0, len(data), 4096)]
    for i, chunk in enumerate(chunks):
        head = f"a=T,f=100,i=1,c={c},r={r},q=2,C=1," if i == 0 else ""
        out.write(f"\033_G{head}m={1 if i < len(chunks) - 1 else 0};{chunk}\033\\")
    out.write(f"\033[{rows};{left + 1}H\033[K{serial}  {time.strftime('%H:%M:%S')}  {len(buffer.getvalue()) // 1024} KB")
    out.flush()
    return key


out.write("\033[?25l\033[2J")
last = None
try:
    while True:
        start = time.monotonic()
        if os.path.exists(PAUSE):
            out.write(f"\033[{pane()[1]};1H\033[Kpaused while measuring…");out.flush()
            time.sleep(0.5);last = None;continue
        png = frame()
        if png.startswith(b"\x89PNG"):
            last = draw(png, last)
        else:
            last = None
            out.write(f"\033[H\033[2Jwaiting for {serial}…")
            out.flush()
        time.sleep(max(0.0, interval - (time.monotonic() - start)))
except KeyboardInterrupt:
    pass
finally:
    out.write("\033_Ga=d,d=I,i=1,q=2\033\\\033[?25h\n")
    out.flush()
