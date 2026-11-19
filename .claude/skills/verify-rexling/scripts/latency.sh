#!/bin/sh
# Measure keystroke latency (key press → echo drawn) in a throwaway Rex session.
# Sends N single key events at a typing pace and reads the app's RexlingLatency samples.
# Usage: latency.sh [count=40] [gap-seconds=0.3]
# Needs a debug or release build that includes KeystrokeLatency.kt.
set -eu
# Pin every adb call to the emulator; a connected personal phone must never be driven.
export ANDROID_SERIAL="${ANDROID_SERIAL:-emulator-5554}"
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$HERE/../../../.." && pwd)
COUNT="${1:-40}"; GAP="${2:-0.3}"
RUN="$ROOT/rexling-android/.build/verify/latency-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN"
PAUSE="$ROOT/rexling-android/.build/verify/viewer.pause"; touch "$PAUSE"   # quiet the mirror pane
ui() { "$HERE/ui.py" "$@"; }
label=$("$HERE/scratch-session.sh" new)
trap 'rm -f "$PAUSE"; adb shell setprop log.tag.RexlingLatency INFO || true; adb shell am force-stop dev.mobrienv.rexling || true; "$HERE/scratch-session.sh" kill "$label" >/dev/null 2>&1 || true' EXIT

adb shell setprop log.tag.RexlingLatency DEBUG
adb shell am force-stop dev.mobrienv.rexling
adb shell monkey -p dev.mobrienv.rexling -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
ui has "Session strip" --timeout 20 >/dev/null; sleep 3
ui tap "Session strip" >/dev/null
ui tap "$label" --timeout 15 >/dev/null
ui has "$label · " --timeout 10 >/dev/null
ui tap "Terminal output" >/dev/null; sleep 2
adb logcat -c
i=0
while [ "$i" -lt "$COUNT" ]; do
  adb shell input keyevent KEYCODE_A; sleep "$GAP"; i=$((i+1))
done
sleep 1
adb logcat -d -s RexlingLatency:D >"$RUN/samples.log"
ui shot "$RUN/screen.png" >/dev/null
"$HERE/scratch-session.sh" capture "$label" >"$RUN/server-capture.txt"

python3 -I - "$RUN/samples.log" "$COUNT" <<'EOF' | tee "$RUN/summary.txt"
import re, sys
rows = [dict((k, float(v)) for k, v in re.findall(r"(\w+)=([\d.]+)", l)) for l in open(sys.argv[1]) if "keystroke total=" in l]
def pct(xs, p):
    xs = sorted(xs); return xs[round(p / 100 * (len(xs) - 1))] if xs else float("nan")
print(f"samples {len(rows)} of {sys.argv[2]} keys")
print(f"{'stage':8} {'p50':>7} {'p95':>7} {'max':>7}  (ms)")
for stage in ["send", "echo", "present", "draw", "total"]:
    xs = [r[stage] for r in rows]
    print(f"{stage:8} {pct(xs,50):7.1f} {pct(xs,95):7.1f} {max(xs) if xs else float('nan'):7.1f}")
EOF
echo "evidence: $RUN"
