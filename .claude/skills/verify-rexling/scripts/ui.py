#!/usr/bin/env python3
"""Drive Rexling's UI by its accessibility handles (text or content-desc), not coordinates.

  ui.py list                     print every labelled node: label, bounds
  ui.py has LABEL [--timeout S]  exit 0 once a node's label contains LABEL
  ui.py tap LABEL [--timeout S]  tap the center of the first node whose label contains LABEL
  ui.py type TEXT                type into the focused field (spaces allowed)
  ui.py key KEYCODE              e.g. ENTER, BACK, DEL
  ui.py shot PATH                save a PNG screenshot

Uses ANDROID_SERIAL (default emulator-5554).
"""
import os
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

SERIAL = os.environ.get("ANDROID_SERIAL", "emulator-5554")


def adb(*args, **kw):
    return subprocess.run(["adb", "-s", SERIAL, *args], capture_output=True, check=True, **kw).stdout


def nodes():
    # uiautomator dump fails transiently (fresh boot, window transitions); a miss is "no nodes yet".
    try:
        adb("shell", "uiautomator", "dump", "/sdcard/rexling-ui.xml")
        root = ET.fromstring(adb("shell", "cat", "/sdcard/rexling-ui.xml"))
    except (subprocess.CalledProcessError, ET.ParseError):
        return
    for n in root.iter("node"):
        label = n.get("text") or n.get("content-desc") or ""
        if label:
            x1, y1, x2, y2 = map(int, re.findall(r"\d+", n.get("bounds")))
            yield label, (x1, y1, x2, y2)


def find(label, timeout):
    end = time.time() + timeout
    while True:
        for text, box in nodes():
            if label in text:
                return text, box
        if time.time() >= end:
            return None
        time.sleep(0.5)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    cmd, args = sys.argv[1], sys.argv[2:]
    timeout = 10.0
    if "--timeout" in args:
        i = args.index("--timeout")
        timeout = float(args[i + 1])
        del args[i:i + 2]
    if cmd == "list":
        for text, box in nodes():
            print(f"{text!r}\t{box}")
    elif cmd in ("has", "tap"):
        hit = find(args[0], timeout)
        if not hit:
            sys.exit(f"not found: {args[0]!r}")
        text, (x1, y1, x2, y2) = hit
        if cmd == "tap":
            adb("shell", "input", "tap", str((x1 + x2) // 2), str((y1 + y2) // 2))
        print(f"{cmd} {text!r}")
    elif cmd == "type":
        adb("shell", "input", "text", " ".join(args).replace(" ", "%s"))
    elif cmd == "key":
        adb("shell", "input", "keyevent", f"KEYCODE_{args[0]}")
    elif cmd == "shot":
        with open(args[0], "wb") as f:
            f.write(adb("exec-out", "screencap", "-p"))
        print(args[0])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
