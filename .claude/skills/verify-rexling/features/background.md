# Staying connected off screen

Leaving Rexling for another app keeps its connections and pane claims for 45 seconds, so coming straight back needs no reconnect. After that, or as soon as the screen turns off, it releases the pane and disconnects, and reconnects on return.

## Sub-features

- Back within 45 seconds: same connections, same pane owner, typing works at once.
- Away longer: the pane is released and both connections close 45 seconds in.
- Screen off: pauses at once, whether Rexling is in front or already held.
- Other apps opened over it: an expedited job (`BackgroundHoldJob`) keeps Android from freezing the process until the hold ends.
- Rotation no longer reconnects.

## How to get to it (user POV)

Press Home or switch apps, then come back.

## Driving it

A private Rex server makes this checkable from the server side without touching the user's sessions. `adb reverse` can tunnel the emulator to its Unix socket, so nothing listens on TCP:

```sh
SOCK=$XDG_RUNTIME_DIR/rexling-hold-test/server.sock; mkdir -p "$(dirname "$SOCK")"   # scratchpad paths are too long for a socket
REX="rex -S unix://$SOCK --autostart=false"                                          # always pass -S, or you drive the real server
(cd /tmp && setsid rex server run --listen unix://$SOCK --state-dir <scratch>/state --restore=false > <scratch>/server.log 2>&1 < /dev/null &)
(cd /tmp && $REX new verify-rexling-hold --cwd /tmp)
adb reverse tcp:8765 localfilesystem:$SOCK
adb shell setprop log.tag.RexlingHold DEBUG       # logs why each hold starts and ends
```

In the app, add the server `http://127.0.0.1:8765` (`servers.md`). Then:

```sh
adb shell input keyevent HOME                      # leave; adb shell am start -n <package>/dev.shulhi.rex.MainActivity returns
adb shell input keyevent SLEEP                     # screen off; WAKEUP and `wm dismiss-keyguard` undo it
adb logcat -d -s RexlingHold                       # holding / returned in time / paused: time up | screen off | returned late
$REX block ls -s verify-rexling-hold               # OWNER is the app's client while the pane is claimed
grep -E 'ownership changed|path=/api/(data|control) .*duration=[0-9.]+(s|m[0-9])' <scratch>/server.log   # disown, and when the long-lived connections closed
adb shell "dumpsys activity processes <package>" | grep -E 'oom adj|isFrozen'       # 227 and unfrozen during a hold
```

Afterwards: `$REX server stop`, `adb reverse --remove-all`.

## What proves it

- Back in time: no new `ownership changed` or connection-close line in the server log during the trip, and a command typed right after returning runs.
- Away longer: `disown` and both closes about 45 seconds after `holding`.
- Two apps opened over Rexling (`am start -a android.settings.SETTINGS`, then `-a android.intent.action.DIAL`): still `isFrozen=false` until the hold ends.

## Gotchas

- `rex client ls` keeps listing the app after it disconnects, because the server keeps a client around to resume. Use the pane owner and the server log instead.
- `ui.py type` passes text through the device shell: a `;` ends the text there.
- `adb shell am freeze --sticky <package>` freezes the process by hand, to see a late return (`paused: time up` on thaw, then a reconnect).
