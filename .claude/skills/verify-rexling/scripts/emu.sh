#!/bin/sh
# Start, check or stop the verification emulator. Only stops what `start` started.
# Usage: emu.sh start | health | stop
set -u
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
AVD="${REXLING_AVD:-rexling_api36}"
SERIAL="${ANDROID_SERIAL:-emulator-5554}"
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../../.." && pwd)
STATE="$ROOT/rexling-android/.build/verify"
LOG="$STATE/emulator.log"
PIDFILE="$STATE/emulator.pid"
# Host of the Rex server under test, from $REXLING_SERVER_URL (see `rex server status`).
SERVER_HOST=$(printf %s "${REXLING_SERVER_URL:-}" | sed -E 's#^[a-z]+://##; s#[:/].*##')
mkdir -p "$STATE"

booted() { [ "$(adb -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; }

case "${1:-}" in
start)
  if adb devices | grep -q "^$SERIAL[[:space:]]*device"; then
    echo "$SERIAL is already running and was not started by this script; run 'health' and reuse it, or stop it yourself" >&2
    exit 1
  fi
  # -gpu host: SwiftShader (the default, and -gpu guest's fallback) segfaults qemu
  # when Rexling renders a terminal. DISPLAY=:0 (Xwayland) lets EGL initialize.
  DISPLAY="${DISPLAY:-:0}" nohup "$SDK/emulator/emulator" -avd "$AVD" -no-window -no-audio \
    -no-snapshot-save -no-boot-anim -gpu host >"$LOG" 2>&1 &
  echo $! >"$PIDFILE"
  adb -s "$SERIAL" wait-for-device
  i=0; until booted; do i=$((i+1)); [ $i -gt 80 ] && { echo "boot timed out; see $LOG" >&2; exit 1; }; sleep 3; done
  adb -s "$SERIAL" shell settings put global window_animation_scale 0
  adb -s "$SERIAL" shell settings put global transition_animation_scale 0
  adb -s "$SERIAL" shell settings put global animator_duration_scale 0
  echo "ready: $SERIAL (pid $(cat "$PIDFILE"))"
  ;;
health)
  rc=0
  check() { if eval "$2" >/dev/null 2>&1; then echo "ok    $1"; else echo "FAIL  $1"; rc=1; fi; }
  check "$SERIAL online" "adb devices | grep -q '^$SERIAL[[:space:]]*device'"
  check "booted" booted
  check "host GPU, not SwiftShader" "adb -s $SERIAL shell dumpsys SurfaceFlinger | grep -m1 '^GLES:' | grep -qv -i swiftshader"
  check "Rexling installed" "adb -s $SERIAL shell pm path dev.mobrienv.rexling | grep -q package:"
  check "Rexling runs as x86_64" "adb -s $SERIAL shell dumpsys package dev.mobrienv.rexling | grep -q 'primaryCpuAbi=x86_64'"
  if [ -n "$SERVER_HOST" ]; then check "Rex server reachable from emulator" "adb -s $SERIAL shell ping -c1 -W3 $SERVER_HOST"
  else echo "skip  Rex server reachability (set REXLING_SERVER_URL)"; fi
  exit $rc
  ;;
stop)
  if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    adb -s "$SERIAL" emu kill >/dev/null 2>&1 || kill "$(cat "$PIDFILE")"
    rm -f "$PIDFILE"; echo "stopped"
  else
    rm -f "$PIDFILE"; echo "nothing started by this script is running"
  fi
  ;;
*) echo "usage: $0 start|health|stop" >&2; exit 2 ;;
esac
