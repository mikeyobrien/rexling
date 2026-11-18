#!/bin/sh
# End-to-end proof: in a throwaway Rex session, type a unique command in Rexling and
# confirm the server-side terminal ran it. Evidence survives in the printed directory.
# Usage: smoke.sh   (emulator up, Rexling installed and connected to the server)
set -eu
# Pin every adb call to the emulator; a connected personal phone must never be driven.
export ANDROID_SERIAL="${ANDROID_SERIAL:-emulator-5554}"
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$HERE/../../../.." && pwd)
RUN="$ROOT/rexling-android/.build/verify/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN"
ui() { "$HERE/ui.py" "$@"; }
label=$("$HERE/scratch-session.sh" new)
# Stop the app before killing the session it's viewing: killing a viewed session leaves
# Rexling stuck at "reconnecting · input paused" with an empty session list.
trap 'adb shell am force-stop dev.shulhi.rex || true; "$HERE/scratch-session.sh" kill "$label" >/dev/null 2>&1 || true' EXIT
nonce="rexling-smoke-$(date +%s)"

adb shell am force-stop dev.shulhi.rex   # a fresh start never inherits a wedged connection
adb shell monkey -p dev.shulhi.rex -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
ui has "Session strip" --timeout 20 >/dev/null
sleep 3                                   # let the session list load before opening it
ui tap "Session strip"
ui tap "$label" --timeout 15
ui has "$label · " --timeout 10           # header shows the scratch session
ui tap "Terminal output" --timeout 10     # take control
sleep 1
ui shot "$RUN/1-before.png" >/dev/null
ui type "echo $nonce"
ui key ENTER
sleep 2
ui shot "$RUN/2-after.png" >/dev/null
"$HERE/scratch-session.sh" capture "$label" >"$RUN/server-capture.txt"
adb logcat -d -b crash >"$RUN/crash-log.txt" 2>&1 || true

# The command line echoes the nonce once; its output adds a line that is the nonce alone.
if grep -qx "$nonce" "$RUN/server-capture.txt"; then
  echo "PASS  typed in Rexling, ran on the server ($label)"; echo "evidence: $RUN"
else
  echo "FAIL  nonce not run in $label"; echo "evidence: $RUN"; exit 1
fi
