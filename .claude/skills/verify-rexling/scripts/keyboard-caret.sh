#!/bin/sh
# Open the on-screen keyboard over a program in a throwaway session and screenshot whether
# its input line stays visible. Proves keyboard placement for TUIs that hide the cursor.
# Usage: keyboard-caret.sh NAME COMMAND...   e.g. keyboard-caret.sh pi pi
# Needs an emulator without a hardware keyboard (REXLING_AVD=rexling_api36), or the
# on-screen keyboard never takes space.
set -eu
export ANDROID_SERIAL="${ANDROID_SERIAL:-emulator-5554}"
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$HERE/../../../.." && pwd)
NAME="$1"; shift
RUN="$ROOT/rexling-android/.build/verify/keyboard-$NAME-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN"
PAUSE="$ROOT/rexling-android/.build/verify/viewer.pause"; touch "$PAUSE"
ui() { "$HERE/ui.py" "$@"; }
label=$("$HERE/scratch-session.sh" new "verify-rexling-$NAME")
trap 'rm -f "$PAUSE"; adb shell am force-stop dev.mobrienv.rexling || true; "$HERE/scratch-session.sh" kill "$label" >/dev/null 2>&1 || true' EXIT
block=$(rex block ls -s "$label" | awk 'NR==2{print $2}')

adb shell am force-stop dev.mobrienv.rexling
adb shell monkey -p dev.mobrienv.rexling -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
ui has "Session strip" --timeout 20 >/dev/null; sleep 3
ui tap "Session strip" >/dev/null
ui tap "$label" --timeout 15 >/dev/null
ui has "$label · " --timeout 10 >/dev/null; sleep 2
rex send -s "$label" -b "$block" "$*
"
sleep "${SETTLE:-8}"                               # let the program draw its input line
# Opening the session already took control. Don't tap the terminal: a tap on a link opens it.
ui shot "$RUN/1-before-keyboard.png" >/dev/null
ui has "Hide keyboard" --timeout 1 >/dev/null 2>&1 || ui tap "Show keyboard" --timeout 5 >/dev/null
sleep 3
ui shot "$RUN/2-keyboard.png" >/dev/null
"$HERE/scratch-session.sh" capture "$label" >"$RUN/server-capture.txt"
adb shell dumpsys input_method | grep -m1 -oE 'mInputShown=[a-z]+' >"$RUN/ime.txt" || true
adb shell dumpsys window windows | grep -A3 'InputMethod' | grep -m1 -oE 'mFrame=\[[0-9,\]\[ ]+' >>"$RUN/ime.txt" || true
cat "$RUN/ime.txt"
echo "evidence: $RUN"
