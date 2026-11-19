#!/usr/bin/env python3
"""Score how well Rexling shows media streamed into a pane with Kitty graphics.

Each case runs scripts/rexcast in a throwaway Rex session that the app is viewing,
records the device screen, and reads the result back from the recording alone, so the
numbers describe what a person would see. See README.md for the cases and targets.

Usage: run.py [--serial emulator-5554] [--only CASE,...] [--name LABEL] [--rexcast "ARGS"]
"""
import argparse
import json
import os
import re
import shlex
import statistics
import subprocess
import sys
import time

from PIL import Image

sys.dont_write_bytecode = True  # loading scripts/rexcast must not leave a cache beside it

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../.."))
SCRIPTS = os.path.join(ROOT, ".claude/skills/verify-rexling/scripts")
REXCAST = os.path.join(ROOT, "scripts/rexcast")
PAGE = os.path.join(HERE, "page.html")
PACKAGE = os.environ.get("REXLING_PACKAGE", "dev.mobrienv.rexling")
BITS = 12


def sh(*command, check=True, capture=True, timeout=120):
    result = subprocess.run(command, capture_output=capture, text=True, timeout=timeout)
    if check and result.returncode:
        raise RuntimeError(f"{' '.join(command)}: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout if capture else ""


class Device:
    def __init__(self, serial):
        self.serial = serial

    def adb(self, *arguments, **options):
        return sh("adb", "-s", self.serial, *arguments, **options)

    def ui(self, *arguments, check=True):
        env = dict(os.environ, ANDROID_SERIAL=self.serial)
        result = subprocess.run([os.path.join(SCRIPTS, "ui.py"), *arguments], capture_output=True, text=True, env=env)
        if check and result.returncode:
            raise RuntimeError(f"ui.py {' '.join(arguments)}: {result.stderr.strip() or result.stdout.strip()}")
        return result.returncode == 0

    def clock_offset(self):
        """Seconds the device clock runs ahead of this machine's, from the quickest of a few round trips."""
        best = None
        for _ in range(6):
            before = time.time()
            remote = float(self.adb("shell", "date", "+%s.%N"))
            after = time.time()
            if best is None or after - before < best[0]:
                best = (after - before, remote - (before + after) / 2)
        return best[1]

    def screenshot(self, path):
        with open(path, "wb") as out:
            subprocess.run(["adb", "-s", self.serial, "exec-out", "screencap", "-p"], stdout=out, check=True)
        return Image.open(path).convert("RGB")

    def record(self, seconds):
        """Start a screen recording. Returns a function that waits for it and pulls the file."""
        remote = "/sdcard/rexling-media-eval.mp4"
        # Half size: encoding the full screen on the device takes frames away from the app under test.
        width, height = (int(v) for v in re.search(r"(\d+)x(\d+)", self.adb("shell", "wm", "size")).groups())
        process = subprocess.Popen(["adb", "-s", self.serial, "shell", "screenrecord", "--bit-rate", "8000000",
                                    "--size", f"{width // 2}x{height // 2}", "--time-limit", str(int(seconds)), remote],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)

        def finish(path):
            process.wait(seconds + 20)
            time.sleep(0.5)
            self.adb("pull", remote, path)
            self.adb("shell", "rm", "-f", remote)
            return path
        return finish


class Session:
    """A throwaway Rex session the app is pointed at."""

    def __init__(self, device):
        self.device = device
        self.label = sh(os.path.join(SCRIPTS, "scratch-session.sh"), "new", f"verify-rexling-media-{time.strftime('%H%M%S')}").strip()
        self.block = sh("rex", "block", "ls", "-s", self.label).splitlines()[1].split()[1]

    def open(self):
        device = self.device
        device.adb("shell", "am", "force-stop", PACKAGE)
        device.adb("shell", "monkey", "-p", PACKAGE, "-c", "android.intent.category.LAUNCHER", "1")
        # The header's second line is the session strip, or a status line while only viewing.
        # Either opens the switcher. It can change between the two as the app connects, so
        # look again right before tapping.
        deadline, opened = time.monotonic() + 30, False
        time.sleep(4)
        while not opened:
            if time.monotonic() > deadline:
                raise RuntimeError("the app's header did not appear")
            opened = any(device.ui("tap", name, "--timeout", "1", check=False) for name in ("Session strip", "Status"))
        device.ui("tap", self.label, "--timeout", "15")
        device.ui("has", self.label + " · ", "--timeout", "10")
        time.sleep(2)

    def send(self, text):
        sh("rex", "send", "-s", self.label, "-b", self.block, text + "\r")

    def interrupt(self):
        sh("rex", "send", "-s", self.label, "-b", self.block, "\x1c\x03", check=False)

    def capture(self):
        return sh("rex", "capture", "-s", self.label, "-b", self.block, "--trim", check=False)

    def close(self):
        self.device.adb("shell", "am", "force-stop", PACKAGE, check=False)
        sh(os.path.join(SCRIPTS, "scratch-session.sh"), "kill", self.label, check=False)


# ── reading a recording ─────────────────────────────────────────────────────

def magenta(pixel):
    return pixel[0] > 170 and pixel[1] < 110 and pixel[2] > 170


def find_strips(image):
    """Locate the pattern's finder blocks. Returns (left, block width, [(top, bottom) per strip])."""
    width, height = image.size
    pixels = image.load()
    for x in range(2, width // 3, 3):
        runs, start = [], None
        for y in range(height):
            hit = magenta(pixels[x, y])
            if hit and start is None:
                start = y
            elif not hit and start is not None:
                if y - start >= 8:
                    runs.append((start, y))
                start = None
        if runs:
            top, bottom = runs[0]
            middle = (top + bottom) // 2
            left = x
            while left > 0 and magenta(pixels[left - 1, middle]):
                left -= 1
            right = x
            while right < width - 1 and magenta(pixels[right + 1, middle]):
                right += 1
            return left, (right + 1 - left) / 2, runs
    return None


def probe(video, regions):
    """Average colour of each (x, y, w, h) region in every frame, with frame times in seconds."""
    chains, labels = [], []
    for i, (x, y, w, h) in enumerate(regions):
        x, y, w, h = x / 2, y / 2, w / 2, h / 2  # recordings are half size
        chains.append(f"[s{i}]crop={max(2, int(w))}:{max(2, int(h))}:{int(x)}:{int(y)},scale=1:1:flags=area[c{i}]")
        labels.append(f"[c{i}]")
    graph = f"[0:v]split={len(regions)}" + "".join(f"[s{i}]" for i in range(len(regions))) + ";" + ";".join(chains) + \
        ";" + "".join(labels) + f"hstack=inputs={len(regions)}[out]"
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", video, "-filter_complex", graph, "-map", "[out]",
                          "-fps_mode", "passthrough", "-pix_fmt", "rgb24", "-f", "rawvideo", "-"],
                         capture_output=True, check=True).stdout
    times = [float(v) for v in sh("ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                                  "frame=pts_time", "-of", "csv=p=0", video).split() if v.strip(",")]
    n = len(regions) * 3
    frames = [[tuple(raw[f * n + r * 3:f * n + r * 3 + 3]) for r in range(len(regions))] for f in range(len(raw) // n)]
    count = min(len(times), len(frames))
    return times[:count], frames[:count]


def bit_regions(left, block, strip):
    top, bottom = strip
    inset = (bottom - top) * .3
    return [(left + block * (2 + bit) + block * .3, top + inset, block * .4, (bottom - top) * .4) for bit in range(BITS)]


def read_index(colours):
    value = 0
    for colour in colours:
        level = sum(colour) / 3
        if 90 < level < 165:
            return None
        value = value << 1 | (level >= 165)
    return value


def percentile(values, p):
    values = sorted(values)
    return values[round(p / 100 * (len(values) - 1))] if values else float("nan")


# ── cases ───────────────────────────────────────────────────────────────────

class Runner:
    def __init__(self, device, session, out, extra):
        self.device, self.session, self.out, self.extra = device, session, out, extra

    def reset(self):
        """Stop whatever the last case left running and put the keyboard away."""
        self.session.interrupt()
        if "mInputShown=true" in self.device.adb("shell", "dumpsys", "input_method"):
            self.device.adb("shell", "input", "keyevent", "BACK")
        time.sleep(1)

    def cast(self, name, arguments, seconds=0):
        self.reset()
        log = os.path.join(self.out, f"{name}.cast.log")
        limit = f"--seconds {seconds} " if seconds else ""
        self.session.send(f"clear; {shlex.quote(REXCAST)} --log {shlex.quote(log)} {limit}{self.extra} {arguments}")
        return log

    def cast_summary(self, log, duration):
        rows = [line.split() for line in open(log) if not line.startswith("#")] if os.path.exists(log) else []
        size = sum(int(r[3]) for r in rows)
        return {"sent_frames": len(rows), "mbit_per_s": round(size * 8 / 1e6 / max(.001, duration), 2),
                "encode_ms": round(1000 * statistics.fmean([float(r[4]) for r in rows]), 2) if rows else None,
                "write_ms_p95": round(1000 * percentile([float(r[5]) for r in rows], 95), 2) if rows else None}

    def locate(self, name, wait=2.5):
        time.sleep(wait)
        shot = self.device.screenshot(os.path.join(self.out, f"{name}.png"))
        found = find_strips(shot)
        if not found:
            raise RuntimeError("the pattern's finder blocks are not on screen")
        return shot, found

    def stream(self, name, kind, fps, seconds=8, link=None):
        """Frame delivery for a continuously changing picture. link limits the device's network, in kbit/s."""
        if link:
            # Police what the device receives. Needs a rooted (emulator) image.
            tc = ("shell", "su", "0", "tc", "filter")
            police = ("dev", "wlan0", "ingress", "pref", "7")
            self.device.adb(*tc, "del", *police, check=False)
            self.device.adb(*tc, "add", *police, "protocol", "all", "matchall", "action", "police", "rate", f"{link}kbit",
                            "burst", "1m", "mtu", "65535", "conform-exceed", "drop")
            try:
                return self.stream(name, kind, fps, seconds)
            finally:
                self.device.adb(*tc, "del", *police, check=False)
        offset = self.device.clock_offset()
        finish = self.device.record(seconds + 5)
        log = self.cast(name, f"--stamp --fps {fps} pattern {kind}", seconds + 2)
        _, (left, block, strips) = self.locate(name)
        self.device.adb("logcat", "-c")
        video = finish(os.path.join(self.out, f"{name}.mp4"))
        lines = self.device.adb("logcat", "-d", "-v", "epoch", "-s", "RexlingMedia:D")
        drawn = [int(v) for v in re.findall(r"picture t=(\d+)", lines)]
        # Sender's clock when a frame left, against the device's clock when the app drew it.
        sent_at = {}
        for row in open(log):
            if not row.startswith("#"):
                sent_at.setdefault(int(row.split()[1]), float(row.split()[0]))
        delays = [1000 * (float(at) - offset - sent_at[int(frame)]) for at, frame in
                  re.findall(r"^\s*(\d+\.\d+).*picture t=\d+ images=\d+ frame=(\d+)", lines, re.M) if int(frame) in sent_at]
        self.session.interrupt()
        regions = bit_regions(left, block, strips[0]) + bit_regions(left, block, strips[-1])
        times, frames = probe(video, regions)
        seen = [(t, read_index(f[:BITS]), read_index(f[BITS:])) for t, f in zip(times, frames)]
        valid = [(t, a, b) for t, a, b in seen if a is not None and b is not None]
        changes = []
        for t, a, _ in valid:
            if not changes or a != changes[-1][1]:
                changes.append((t, a))
        # Skip the first and last half second: start-up and tear-down aren't steady state.
        if len(changes) > 4:
            begin, end = changes[0][0] + .5, changes[-1][0] - .5
            changes = [c for c in changes if begin <= c[0] <= end]
            valid = [v for v in valid if begin <= v[0] <= end]
        if len(changes) < 3:
            return {"error": "the stream never played", "score": 0}
        duration = changes[-1][0] - changes[0][0]
        span = (changes[-1][1] - changes[0][1]) % (1 << BITS)
        gaps = [b[0] - a[0] for a, b in zip(changes, changes[1:])]
        shown = (len(changes) - 1) / duration
        target = min(fps, 60)
        result = {"fps_target": target, "fps_shown": round(shown, 1), "source_fps": round(span / duration, 1),
                  "stall_ms_max": round(1000 * max(gaps)), "interval_ms_p95": round(1000 * percentile(gaps, 95)),
                  "torn_pct": round(100 * sum(1 for _, a, b in valid if a != b) / max(1, len(valid)), 1)}
        if delays:
            result["delay_ms_p50"], result["delay_ms_p95"] = round(percentile(delays, 50)), round(percentile(delays, 95))
        if len(drawn) > 2:
            result["fps_drawn"] = round((len(drawn) - 1) * 1000 / max(1, drawn[-1] - drawn[0]), 1)
        result.update(self.cast_summary(log, seconds + 2))
        smooth = min(1, shown / target)
        steady = min(1, (2.5 * 1000 / target) / max(1, result["stall_ms_max"]))
        # A picture more than 150 ms behind its source feels detached from the hand that drives it.
        prompt = min(1, 150 / max(1, result.get("delay_ms_p95", 150)))
        result["score"] = round(100 * smooth * (.7 + .3 * steady) * (1 - result["torn_pct"] / 100) * (.5 + .5 * prompt))
        return result

    def prime(self, name, command):
        """Start an interactive picture, tap it once, and measure where it settles on screen."""
        log = self.cast(name, command)
        _, (left, block, strips) = self.locate(name, 6 if "web" in command or "app" in command else 2.5)
        if len(strips) < 2:
            raise RuntimeError("the picture is not fully on screen")
        height, pane_top = strips[-1][1] - strips[0][0], strips[0][0]
        centre = left + block * (BITS + 2) / 2
        # A first tap may open the keyboard and slide the picture; measure where it settles.
        self.device.adb("shell", "input", "tap", str(int(centre)), str(int(pane_top + height * .25)))
        time.sleep(2)
        settled = find_strips(self.device.screenshot(os.path.join(self.out, f"{name}-settled.png")))
        if not settled:
            raise RuntimeError("the picture left the screen after a tap")
        bottom = settled[2][-1][1]
        top = bottom - height
        visible = max(0, bottom - max(pane_top, top)) / height
        return {"log": log, "left": left, "block": block, "top": top, "height": height, "pane_top": pane_top,
                "bottom": bottom, "centre": centre, "visible": visible}

    def taps(self, name, command, count=12):
        """Time from lifting a finger to the picture reacting, from the app's RexlingMedia log."""
        try:
            g = self.prime(name, command)
        except RuntimeError as error:
            return {"error": str(error), "score": 0}
        log, left, block, top, height, pane_top, centre, visible = (g[k] for k in ("log", "left", "block", "top", "height", "pane_top", "centre", "visible"))
        side = min(block * (BITS + 2), height) / 4
        square_x, square_y = centre - side / 2, top + height / 2 - side * 1.5
        shown_top = max(square_y, pane_top + 6)
        shown = square_y + side - shown_top
        if shown < 16:
            return {"error": "the tap target is hidden after a tap", "visible_pct": round(100 * visible), "score": 0}
        tap = (int(square_x + side * .75), int(shown_top + shown * .5))
        self.device.adb("logcat", "-c")
        for _ in range(count):
            self.device.adb("shell", "input", "tap", str(tap[0]), str(tap[1]))
            time.sleep(.7)
        latencies = [float(v) for v in re.findall(r"touch latency=(\d+)", self.device.adb("logcat", "-d", "-s", "RexlingMedia:D"))]
        self.device.screenshot(os.path.join(self.out, f"{name}-after.png"))
        self.session.interrupt()
        if len(latencies) < count // 2:
            return {"error": f"only {len(latencies)} of {count} taps were answered", "score": 0, "visible_pct": round(100 * visible)}
        result = {"visible_pct": round(100 * visible), "taps_answered": len(latencies), "taps_sent": count, "latency_ms_p50": round(percentile(latencies, 50)),
                  "latency_ms_p95": round(percentile(latencies, 95)), "latency_ms_max": round(max(latencies))}
        result.update(self.cast_summary(log, count))
        answered = min(1, len(latencies) / count)
        result["score"] = round(100 * visible * answered * min(1, 100 / max(1, result["latency_ms_p50"])) ** .5 *
                                min(1, 200 / max(1, result["latency_ms_p95"])) ** .5)
        return result

    def pointer(self, name):
        """Whether touch reaches the picture as a pointer: exact taps, drags, right-clicks and scrolling."""
        try:
            g = self.prime(name, "pattern tap")
        except RuntimeError as error:
            return {"error": str(error), "score": 0}
        header = open(g["log"]).readline().split()
        sent_w, sent_h = (int(v) for v in header[-1].split("x"))
        scale = g["height"] / sent_h
        shown_top = max(g["top"], g["pane_top"]) + 20
        shown_bottom = g["bottom"] - g["height"] / 24 - 20

        def events(action):
            before = sum(1 for _ in open(g["log"]))
            action()
            time.sleep(1.2)
            rows = [line.split() for line in list(open(g["log"]))[before:] if line.startswith("# ")]
            return [(r[1], float(r[2]) * scale + g["left"], float(r[3]) * scale + g["top"], int(r[4])) for r in rows if len(r) == 5]

        def swipe(*numbers):
            self.device.adb("shell", "input", "swipe", *(str(int(n)) for n in numbers))

        result, errors = {"visible_pct": round(100 * g["visible"])}, []
        width = sent_w * scale
        for fx, fy in ((.13, .1), (.87, .12), (.31, .37), (.69, .55), (.21, .78), (.9, .93), (.5, .5), (.07, .63)):
            x, y = int(g["left"] + width * fx), int(shown_top + (shown_bottom - shown_top) * fy)
            presses = [e for e in events(lambda: self.device.adb("shell", "input", "tap", str(x), str(y))) if e[0] == "press"]
            errors.append(((presses[0][1] - x) ** 2 + (presses[0][2] - y) ** 2) ** .5 if presses else 999)
        result["tap_error_px_p50"], result["tap_error_px_max"] = round(percentile(errors, 50), 1), round(max(errors), 1)
        x, y1, y2 = g["left"] + width * .5, shown_top + (shown_bottom - shown_top) * .7, shown_top + (shown_bottom - shown_top) * .3
        wheel = [e for e in events(lambda: swipe(x, y1, x, y2, 300)) if e[0] == "wheel"]
        result["scroll_events"] = len(wheel)
        result["scroll"] = bool(wheel) and all(e[3] == 5 for e in wheel)
        held = events(lambda: swipe(x, y1, x, y1, 900))
        result["right_click"] = any(e[0] == "press" and e[3] == 3 for e in held) and not any(e[0] == "press" and e[3] == 1 for e in held)
        x2 = g["left"] + width * .8
        drag = events(lambda: self.device.adb("shell", "input", "draganddrop", str(int(x)), str(int(y1)), str(int(x2)), str(int(y2)), "1500"))
        press = [e for e in drag if e[0] == "press" and e[3] == 1]
        release = [e for e in drag if e[0] == "release"]
        result["drag_moves"] = sum(1 for e in drag if e[0] == "move")
        result["drag"] = bool(press and release and result["drag_moves"] >= 3 and abs(press[0][1] - x) < 40
                              and abs(release[-1][1] - x2) < 40 and abs(release[-1][2] - y2) < 40)
        self.session.interrupt()
        accuracy = max(0, min(1, (24 - result["tap_error_px_p50"]) / 22))
        result["score"] = round(g["visible"] * (40 * accuracy + 20 * result["scroll"] + 20 * result["right_click"] + 20 * result["drag"]))
        return result

    def typing(self, name):
        """Whether typed keys reach a streamed browser, and how soon each one shows."""
        try:
            g = self.prime(name, f"web file://{PAGE}")
        except RuntimeError as error:
            return {"error": str(error), "score": 0}
        width = g["block"] * (BITS + 2)

        def index(label):
            shot = self.device.screenshot(os.path.join(self.out, f"{name}-{label}.png"))
            found = find_strips(shot)
            if not found:
                return None
            left, block, strips = found
            pixels = shot.load()
            middle = (strips[0][0] + strips[0][1]) // 2
            return read_index([pixels[int(left + block * (2.5 + bit)), middle] for bit in range(BITS)])

        # Focus the page's text field, then type with the keyboard closed, as a hardware keyboard would.
        self.device.adb("shell", "input", "tap", str(int(g["left"] + width / 2)), str(int(g["top"] + g["height"] * .768)))
        time.sleep(1.5)
        before = index("before")
        self.device.adb("shell", "setprop", "log.tag.RexlingLatency", "DEBUG")
        self.device.adb("logcat", "-c")
        text = "rexling"
        for letter in text:
            self.device.adb("shell", "input", "keyevent", f"KEYCODE_{letter.upper()}")
            time.sleep(.4)
        time.sleep(1)
        after = index("after")
        totals = [float(v) for v in re.findall(r"keystroke total=([\d.]+)", self.device.adb("logcat", "-d", "-s", "RexlingLatency:D"))]
        self.device.adb("shell", "setprop", "log.tag.RexlingLatency", "INFO")
        self.session.interrupt()
        if before is None or after is None:
            return {"error": "the page left the screen while typing", "visible_pct": round(100 * g["visible"]), "score": 0}
        arrived = (after - before) % (1 << BITS)
        forwarded = sum(len(eval(line[6:])) for line in open(g["log"]) if line.startswith("# key "))
        result = {"visible_pct": round(100 * g["visible"]), "keys_sent": len(text), "keys_forwarded": forwarded, "keys_arrived": arrived,
                  "index": f"{before}->{after}"}
        if totals:
            result["echo_ms_p50"], result["echo_ms_p95"] = round(percentile(totals, 50)), round(percentile(totals, 95))
        quick = min(1, 100 / max(1, result.get("echo_ms_p50", 100))) if totals else .5
        result["score"] = round(100 * g["visible"] * (arrived == len(text)) * (.6 + .4 * quick))
        return result

    def still(self, name):
        """Resolution and accuracy of a still picture."""
        log = self.cast(name, "pattern chart")
        shot, (left, block, strips) = self.locate(name, 3)
        self.session.interrupt()
        header = open(log).readline().split() if os.path.exists(log) else []
        if len(strips) < 2 or len(header) < 2:
            return {"error": "the chart is not fully on screen", "score": 0}
        sent_height = int(header[-1].split("x")[1]) if "x" in header[-1] else 0
        shown_height = strips[-1][1] - strips[0][0]
        density = min(1.0, sent_height / shown_height) if sent_height and shown_height else 0
        # The chart's body is flat grey with a ramp; compare it against what was sent.
        box = (int(left), strips[0][1], int(left + block * (BITS + 2)), strips[-1][0])
        crop = shot.crop(box)
        sys.path.insert(0, os.path.dirname(REXCAST))
        import importlib.machinery
        import importlib.util
        loader = importlib.machinery.SourceFileLoader("rexcast", REXCAST)
        module = importlib.util.module_from_spec(importlib.util.spec_from_loader("rexcast", loader))
        loader.exec_module(module)
        sent_width = int(header[-1].split("x")[0])
        pattern = module.Pattern("chart", 1, 0)
        pattern.start(sent_width, sent_height)
        reference = pattern.frame(0)
        reference = reference.crop((0, pattern.strip, pattern.block * (BITS + 2), sent_height - pattern.strip)).resize(crop.size, Image.LANCZOS)
        a, b = crop.tobytes(), reference.tobytes()
        mse = sum((x - y) ** 2 for x, y in zip(a[::7], b[::7])) / max(1, len(a[::7]))
        import math
        psnr = 99.0 if mse == 0 else 10 * math.log10(255 * 255 / mse)
        result = {"sent_px": header[-1], "shown_px": f"{crop.size[0]}x{shown_height}", "density": round(density, 2), "psnr_db": round(psnr, 1)}
        result["score"] = round(100 * density ** .5 * min(1, psnr / 32))
        return result


CASES = {
    "still": lambda r: r.still("still"),
    "video-30": lambda r: r.stream("video-30", "motion", 30),
    "video-60": lambda r: r.stream("video-60", "motion", 60),
    "film-30": lambda r: r.stream("film-30", "film", 30),
    "film-30-slow": lambda r: r.stream("film-30-slow", "film", 30, link=20000),
    "ui-30": lambda r: r.stream("ui-30", "ui", 30),
    "tap": lambda r: r.taps("tap", "pattern tap"),
    "pointer": lambda r: r.pointer("pointer"),
    "tap-full": lambda r: r.taps("tap-full", "pattern swap"),
    "web-tap": lambda r: r.taps("web-tap", f"web file://{PAGE}"),
    "web-type": lambda r: r.typing("web-type"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--serial", default=os.environ.get("ANDROID_SERIAL", "emulator-5554"))
    parser.add_argument("--only", help="comma-separated case names")
    parser.add_argument("--name", default="run", help="label for this run's folder")
    parser.add_argument("--rexcast", default="", help="extra rexcast options, e.g. '--codec full-png'")
    arguments = parser.parse_args()
    names = arguments.only.split(",") if arguments.only else list(CASES)
    out = os.path.join(ROOT, "rexling-android/.build/media-evals", f"{time.strftime('%Y%m%d-%H%M%S')}-{arguments.name}")
    os.makedirs(out)
    device = Device(arguments.serial)
    session = Session(device)
    results = {}
    try:
        device.adb("shell", "setprop", "log.tag.RexlingMedia", "DEBUG")
        session.open()
        runner = Runner(device, session, out, arguments.rexcast)
        for name in names:
            try:
                results[name] = CASES[name](runner)
            except Exception as error:  # one broken case must not hide the others
                results[name] = {"error": str(error), "score": 0}
                session.interrupt()
            print(f"{name:10} {results[name].get('score', 0):3}  " +
                  "  ".join(f"{k}={v}" for k, v in results[name].items() if k != "score"), flush=True)
            time.sleep(1)
        with open(os.path.join(out, "server-capture.txt"), "w") as capture:
            capture.write(session.capture())
        device.adb("logcat", "-d", "-b", "crash", check=False)
    finally:
        session.close()
    total = round(statistics.fmean(r.get("score", 0) for r in results.values()), 1) if results else 0
    json.dump({"name": arguments.name, "rexcast": arguments.rexcast, "total": total, "cases": results},
              open(os.path.join(out, "score.json"), "w"), indent=1)
    print(f"total {total}  evidence: {out}")


if __name__ == "__main__":
    main()
