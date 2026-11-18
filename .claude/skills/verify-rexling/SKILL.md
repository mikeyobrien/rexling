---
name: verify-rexling
description: Build, launch and drive the Rexling Android app (unofficial Rex client) on a headless x86_64 emulator against the local Rex server, and prove behavior with screenshots plus server-side terminal captures. Also covers the repo's check tiers (Go, Rust/JNI, Clippy, Gradle unit tests, lint, APK verification). Use after changing rexling-android, rexling-core or rexling-network, before committing, or when asked to test, check, verify or demo the app.
---

# Verify Rexling

The user-facing surface is the Android app (`dev.shulhi.rex`), a client that views and controls Rex terminal sessions. The proof runs through the real path: the app on an emulator, connected over HTTPS to the user's Rex server, typing into a throwaway Rex session, with the result read back on the server with `rex capture`. Secondary surface: the `rex-probe` CLI (`rexling-core/probe/README.md`).

All paths below are relative to the repo root. Helpers are in `.claude/skills/verify-rexling/scripts/` (`S=` below).

## Never drive the user's own sessions

The app connects to the user's live Rex server. Opening one of their sessions in the app takes terminal control and resizes the shared PTY for every client. Drive only scratch sessions that `$S/scratch-session.sh new` creates (labelled `verify-rexling-*`), and kill them afterwards. The emulator is single-instance (`emulator-5554`). If it's already running and you didn't start it, reuse it after the health check rather than starting a second one.

## Launch

1. **Prerequisites:** `sh $S/preflight.sh`. Missing pieces on this machine were JDK 25 (`mise install java@temurin-25`), NDK `30.0.16248370` and the Rust targets `aarch64-linux-android` and `x86_64-linux-android`. Installing them needs the user's go-ahead.
2. **Build** a debug APK that also carries x86_64 libraries (about 9 minutes cold, about 2 cached):
   ```sh
   export JAVA_HOME=$(mise where java@temurin-25)
   mise exec go@1.27.1 -- ./rexling-android/scripts/gradle.sh :app:assembleDebug -Prexling.abis=arm64-v8a,x86_64
   ```
   Without x86_64 the app dies at launch with SIGILL in `librex_network.so`: ARM translation can't emulate Go's `MIDR_EL1` read.
3. **Emulator:** `$S/emu.sh start`. It's ready when it prints `ready: emulator-5554`. It runs headless on the host GPU with animations off; its log is `rexling-android/.build/verify/emulator.log`.
4. **Install:** `adb install -r rexling-android/app/build/outputs/apk/debug/app-debug.apk`.
5. **First run only:** add the server. Fresh data has no server, so see `features/servers.md`. Use the URL in `$REXLING_SERVER_URL` (the owner's Rex server, for example `https://rex.<tailnet>.ts.net`; `rex server status` prints it). The emulator reaches the tailnet through this host.
6. **Watch it (optional):** a Rex pane mirroring the screen, view only:
   `rex split --focus=false --keep-open --label emulator --ratio 35 -- python3 $S/emu-view.py emulator-5554 0.3`

## Health check

`$S/emu.sh health` is read-only. It checks that the device is online and booted, the renderer is the host GPU (not SwiftShader), Rexling is installed and running as x86_64, and the Rex server answers ping from inside the emulator. Run it first, and again whenever something looks off. Server side: `rex server status` should show `tailscale: running`.

## Drive

`$S/ui.py` acts on accessibility labels (`text` or `content-desc`) from `uiautomator`, never coordinates. Its commands: `list`, `has LABEL`, `tap LABEL`, `type TEXT`, `key ENTER`, `shot PATH`. `tap` and `has` wait up to `--timeout` seconds and match substrings.

- Stable handles: `Session strip`, `Terminal menu`, `Terminal output`, `Tabs and panes`, `Activity`, `Close sheet`, `Terminal keys`, `Escape`, `Control`, `Alt`, `Shift`. Session rows are labelled with the session name.
- Close sheets with `ui.py tap "Close sheet"`, never `key BACK`. Once the keyboard is hidden, BACK leaves the app.
- Opening a session takes control automatically. Tapping `Terminal output` takes control if it was released.
- The app reopens where it was (server, session, control). A run shouldn't assume a fresh UI. Navigate explicitly through `Session strip`.
- Run `adb shell am force-stop dev.shulhi.rex` before killing a session the app is viewing. Killing a viewed session leaves the app stuck at `reconnecting · input paused` with `No sessions yet`, and `Retry now` doesn't recover it (seen 2026-11-17, likely an app bug). After a force-stop, relaunching falls back to another session, possibly the user's. It takes control only if no other client holds the size. `smoke.sh` force-stops at both start and end.

Feature recipes are in `features/` (index: `features/README.md`). A proof that covers one entry point is incomplete when that feature's file lists others.

## Evidence

`$S/smoke.sh` is the canonical end-to-end proof. It creates a scratch session, opens it in the app, takes control and types `echo <nonce>`. It passes only if the server-side capture contains the nonce as its own output line. It writes `1-before.png`, `2-after.png`, `server-capture.txt` and `crash-log.txt` to `rexling-android/.build/verify/<timestamp>/` (gitignored), and that folder survives cleanup.

**Keystroke latency:** `$S/latency.sh [count=40] [gap=0.3]` sends single key events to a scratch session and prints p50, p95 and max per stage. It saves `samples.log`, `summary.txt`, a screenshot and the server capture to `rexling-android/.build/verify/latency-<timestamp>/`. The stages are:
- **send:** input handler until the native layer accepts the key.
- **echo:** until the first update whose screen content changed.
- **present:** waiting for the next vsync, until the frame is handed to the view.
- **draw:** `TerminalView.onDraw`.
- **total:** key press to drawn. It excludes RenderThread and compositor time (about one more frame).

Baseline on 2026-11-17, emulator plus local server: total p50 29 ms and p95 35 ms; echo about 18 ms, present about 10 ms, draw about 1 ms.

To see the same samples by hand, run `adb shell setprop log.tag.RexlingLatency DEBUG` and `adb logcat -s RexlingLatency`. In Perfetto, each keystroke is an async slice named `Rexling.keystroke` (API 29+).

**Scrolling:** `$S/scroll.sh [rounds=4]` fills a scratch session with 5,000 lines, then does slow drags (`input motionevent` with a pause before lifting) and fast flings (`input swipe`). It summarizes the `RexlingScroll` lines per gesture type and saves them to `rexling-android/.build/verify/scroll-<timestamp>/`. The metrics are:
- **touch → drawn:** how fast the local slide follows the finger.
- **catch-up:** a whole-line scroll request until a native frame at that offset is drawn.
- **fling frame intervals:** with frames over 1.5 refresh intervals counted as janky.
- **clamped:** frames where the slide outran the 4 overscan rows (`RP_OVERSCAN` in `rexling-core/native/ghostty_shim.c`), so content stuck at the edge.
- **lines per second.**

Baseline on 2026-11-17, emulator at 60 Hz:
- Drags: touch p50 7.5 ms, catch-up p50 18 ms, no clamping, 42 lines/s.
- Flings: frames p50 16.7 ms, 0.5% janky, catch-up p95 34 ms, **45% of frames clamped**, 210 lines/s.

To see the gestures by hand, run `adb shell setprop log.tag.RexlingScroll DEBUG`. In Perfetto, each gesture is an async slice named `Rexling.scroll`.

**Keyboard placement:** `$S/keyboard-caret.sh NAME COMMAND...` runs a program in a scratch session, opens the on-screen keyboard and saves screenshots from before and after. For example, `keyboard-caret.sh pi pi` checks pi with its default hidden hardware cursor. The program's input line must sit directly above the keyboard, not under it.

This needs an emulator with no hardware keyboard. `rexling_api36` (Pixel 6 profile, `hw.keyboard=no`) is `emu.sh`'s default for that reason; on an AVD with a hardware keyboard the on-screen keyboard never takes space. Don't tap the terminal output to take control: a tap on a link opens the browser. Opening a session already takes control.

**Animations are off on the emulator** (`emu.sh` sets the animation scales to 0 for faster, steadier UI driving). That hides the cost of anything that animates continuously. For a performance or battery check, turn them on first (`adb shell settings put global animator_duration_scale 1`), then count frames over a few idle seconds: `adb shell dumpsys gfxinfo dev.shulhi.rex reset`, wait, then read `Total frames rendered`. An idle screen should draw about 10 frames in 5 s; 300 means something redraws every frame.

Proof standards:
- Use the real user path: taps and typing in the app.
- Verify the side effect on the server (`$S/scratch-session.sh capture LABEL`), not just the screen.
- Capture before and after.
- Save `adb logcat -d -b crash` for any run that fails.

## Cleanup

- `$S/scratch-session.sh kill LABEL` removes a scratch session. `smoke.sh` does this itself, even on failure. The helper refuses any label that doesn't start with `verify-rexling-`.
- `$S/emu.sh stop` stops the emulator only if `emu.sh start` started it; it uses the recorded PID, never a process name.
- Close the mirror pane with `rex block close <id>` (`rex kill` closes sessions, not blocks).
- Evidence in `rexling-android/.build/verify/` is kept.

## Check tiers (no UI)

Run the tiers that match what changed.

| Changed | Run |
|---|---|
| `rexling-network/` | `(cd rexling-network && mise exec go@1.27.1 -- go test ./...)`, then a build |
| `rexling-core/` | `python3 rexling-android/scripts/test_jni.py`, then Clippy (below) |
| `rexling-android/` | `:app:testDebugUnitTest :app:lintRelease`, then `smoke.sh` for UI changes |

```sh
GHOSTTY_PREFIX="$PWD/rexling-core/.build/ghostty-host" CARGO_TARGET_DIR="$PWD/rexling-core/.build/rust" \
  cargo clippy --locked --manifest-path rexling-core/Cargo.toml --workspace --all-targets --all-features -- -D warnings
python3 rexling-android/scripts/verify_apk.py --apk app/build/outputs/apk/release/app-release.apk --require-release
```

- **Instrumented tests:** `ANDROID_SERIAL=emulator-5554 ./rexling-android/scripts/gradle.sh :app:connectedDebugAndroidTest -Pandroid.testInstrumentationRunnerArguments.class=dev.shulhi.rex.<Class>`. The runner uninstalls the app and wipes its data, including the saved server, so emulator only. `rexling-android/TESTING.md` maps features to test classes.
- **Needs explicit approval:** live server tests (`REX_TEST_SERVER=…`, `test_activity_badge.py`), installing toolchains, and `:app:shareRelease`.

## Reporting

For each tier and drive, report passed, failed (with the first error) or not run (and why). Give the evidence path.

## Helpers

| Script | Use |
|---|---|
| `preflight.sh` | Lists the missing prerequisites for each tier. |
| `emu.sh start\|health\|stop` | Emulator lifecycle and the health check. |
| `ui.py` | Label-based UI driving and screenshots. |
| `scratch-session.sh new\|capture\|kill` | Throwaway Rex sessions. `capture` passes `-b` explicitly, because `rex capture` otherwise reads `$REX_BLOCK`, the agent's own block. |
| `smoke.sh` | End-to-end proof run. |
| `latency.sh [count] [gap]` | Keystroke latency percentiles per stage. |
| `scroll.sh [rounds]` | Scroll latency, smoothness, clamping and speed for drags and flings. |
| `emu-view.py` | Mirrors the emulator into a Rex pane. |
