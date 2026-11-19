#!/bin/sh
# Measure scrollback latency and speed in a throwaway Rex session full of output.
# Performs slow drags and fast flings, then summarizes the app's RexlingScroll lines.
# Usage: scroll.sh [rounds=4]
# Needs a build that includes ScrollMetrics.kt.
set -eu
# Pin every adb call to the emulator; a connected personal phone must never be driven.
export ANDROID_SERIAL="${ANDROID_SERIAL:-emulator-5554}"
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$HERE/../../../.." && pwd)
ROUNDS="${1:-4}"
RUN="$ROOT/rexling-android/.build/verify/scroll-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN"
PAUSE="$ROOT/rexling-android/.build/verify/viewer.pause"; touch "$PAUSE"   # quiet the mirror pane
ui() { "$HERE/ui.py" "$@"; }
label=$("$HERE/scratch-session.sh" new)
trap 'rm -f "$PAUSE"; adb shell setprop log.tag.RexlingScroll INFO || true; adb shell am force-stop dev.mobrienv.rexling || true; "$HERE/scratch-session.sh" kill "$label" >/dev/null 2>&1 || true' EXIT

block=$(rex block ls -s "$label" | awk 'NR==2{print $2}')
rex send -s "$label" -b "$block" 'seq 1 5000
'
adb shell setprop log.tag.RexlingScroll DEBUG
adb shell am force-stop dev.mobrienv.rexling
adb shell monkey -p dev.mobrienv.rexling -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
ui has "Session strip" --timeout 20 >/dev/null; sleep 3
ui tap "Session strip" >/dev/null
ui tap "$label" --timeout 15 >/dev/null
ui has "$label · " --timeout 10 >/dev/null
sleep 2
adb logcat -c
# Finger down the screen scrolls back into history; up returns toward the latest output.
# `input swipe` always lifts while moving (a fling). A drag holds still before lifting.
drag() {
  adb shell "input motionevent DOWN 540 $1; y=$1; step=\$(( ($2-$1)/20 ));
    for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do y=\$((y+step)); input motionevent MOVE 540 \$y; done;
    sleep 0.4; input motionevent UP 540 $2"
}
i=0
while [ "$i" -lt "$ROUNDS" ]; do
  drag 600 1700; sleep 1.5                                  # slow drag back
  adb shell input swipe 540 600 540 1800 60;  sleep 3        # fling back
  drag 1700 600; sleep 1.5                                  # slow drag forward
  adb shell input swipe 540 1800 540 600 60;  sleep 3        # fling forward
  i=$((i+1))
done
sleep 1
adb logcat -d -s RexlingScroll:D >"$RUN/gestures.log"
ui shot "$RUN/screen.png" >/dev/null

python3 -I - "$RUN/gestures.log" <<'EOF' | tee "$RUN/summary.txt"
import re, sys
def num(s): return float("nan") if s.lower() == "nan" else float(s)
rows = []
for line in open(sys.argv[1]):
    if "scroll kind=" not in line: continue
    r = {"kind": re.search(r"kind=(\w+)", line)[1]}
    for k in ["duration", "lines", "lines_per_s", "unresolved", "janky", "clamped"]:
        r[k] = num(re.search(rf"\b{k}=([\d.]+|NaN|nan)", line)[1])
    for group in ["touch", "catchup", "fling_frames"]:
        m = re.search(rf"{group} n=(\d+) p50=([\d.]+|NaN|nan) p95=([\d.]+|NaN|nan) max=([\d.]+|NaN|nan)", line)
        r[group] = dict(n=int(m[1]), p50=num(m[2]), p95=num(m[3]), max=num(m[4]))
    rows.append(r)
def pct(xs, p):
    xs = sorted(x for x in xs if x == x); return xs[round(p / 100 * (len(xs) - 1))] if xs else float("nan")
print(f"gestures {len(rows)}: {sum(r['kind']=='drag' for r in rows)} drags, {sum(r['kind']=='fling' for r in rows)} flings")
for kind in ["drag", "fling"]:
    rs = [r for r in rows if r["kind"] == kind]
    if not rs: continue
    frames = sum(r["fling_frames"]["n"] for r in rs); janky = sum(r["janky"] for r in rs)
    print(f"\n{kind} (n={len(rs)})                       p50     p95   (ms; p95 = worst gesture's p95)")
    print(f"  touch -> drawn                {pct([r['touch']['p50'] for r in rs],50):7.1f} {max((r['touch']['p95'] for r in rs if r['touch']['n']), default=float('nan')):7.1f}")
    print(f"  catch-up: request -> native   {pct([r['catchup']['p50'] for r in rs],50):7.1f} {max((r['catchup']['p95'] for r in rs if r['catchup']['n']), default=float('nan')):7.1f}")
    if kind == "fling":
        print(f"  fling frame interval          {pct([r['fling_frames']['p50'] for r in rs],50):7.1f} {max((r['fling_frames']['p95'] for r in rs if r['fling_frames']['n']), default=float('nan')):7.1f}")
        print(f"  fling frames {frames}, janky (>1.5 refresh) {int(janky)} ({100*janky/frames if frames else 0:.1f}%)")
    print(f"  clamped slide frames {int(sum(r['clamped'] for r in rs))}, unresolved catch-ups {int(sum(r['unresolved'] for r in rs))}")
    print(f"  speed (median) {pct([r['lines_per_s'] for r in rs],50):.0f} lines/s, peak {max(r['lines_per_s'] for r in rs):.0f}")
EOF
echo "evidence: $RUN"
