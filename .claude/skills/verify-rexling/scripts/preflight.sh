#!/bin/sh
# Report which Rexling verification tiers this machine can run. Read-only.
set -u
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
NDK_VER=30.0.16248370
ok=0; miss=0
pass() { printf '  ok    %s\n' "$1"; ok=$((ok+1)); }
fail() { printf '  MISS  %s\n' "$1"; miss=$((miss+1)); }
have() { command -v "$1" >/dev/null 2>&1; }

echo "host tier (Go, Rust, JNI smoke, Clippy)"
if have mise && mise exec go@1.27.1 -- go version >/dev/null 2>&1; then pass "go 1.27.1 via mise"
elif have go && go version >/dev/null 2>&1; then pass "go $(go version | awk '{print $3}')"
else fail "go (install: mise install go@1.27.1)"; fi
have cargo && pass "cargo $(cargo --version | awk '{print $2}')" || fail "cargo"
have python3 && pass "python3" || fail "python3"
have curl && pass "curl" || fail "curl"
if [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/javac" ]; then
  jv=$("$JAVA_HOME/bin/java" -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')
  [ "${jv:-0}" -ge 25 ] && pass "JDK $jv at JAVA_HOME" || fail "JDK 25 (JAVA_HOME has JDK $jv)"
else fail "JAVA_HOME with javac"; fi

echo "android tier (Gradle unit tests, lint, APK)"
[ -d "$SDK" ] && pass "SDK at $SDK" || fail "ANDROID_HOME SDK"
ls -d "$SDK"/platforms/android-37* >/dev/null 2>&1 && pass "platform android-37" || fail "platforms;android-37"
[ -d "$SDK/build-tools/36.0.0" ] && pass "build-tools 36.0.0" || fail "build-tools;36.0.0"
[ -d "$SDK/ndk/$NDK_VER" ] || [ -n "${ANDROID_NDK_HOME:-}" ] && pass "NDK $NDK_VER" || fail "ndk;$NDK_VER"
rustup target list --installed 2>/dev/null | grep -qx aarch64-linux-android \
  && pass "rust target aarch64-linux-android" || fail "rustup target add aarch64-linux-android"

echo "device tier (instrumented tests; emulator only)"
have adb && pass "adb" || fail "adb"
if have adb; then
  devs=$(adb devices | awk 'NR>1 && $2=="device"{print $1}')
  [ -n "$devs" ] && pass "attached: $(echo $devs)" || fail "no attached device/emulator"
fi
ls -d "$SDK"/system-images/*/*/arm64-v8a >/dev/null 2>&1 \
  && pass "arm64 system image" || fail "arm64 system image (APK is arm64-v8a only)"

echo "$ok ok, $miss missing"
