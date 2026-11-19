# Sessions

The user picks which Rex session to view, finds one, or creates one.

## Sub-features

- The switcher card's `Sessions` level lists sessions (`Sessions list`), grouped under each server once there are several. Each row shows the session name, then `2 tabs · 4 panes`, and `Needs you` or a working dot.
- Tapping a session opens it; tapping the one you are in shows its tabs. Holding a row offers `Rename` and `Close`; closing always asks in the row (`ends 2 tabs`).
- `Find` appears once there are more than six sessions. `New session` creates one.
- `Terminal menu` → `New session`. Pulling the `Sessions list` down asks the servers for their sessions again.
- Opening a session takes control and resizes the shared PTY to the phone.

## How to get to it (user POV)

Tap the session line under the title in the header (`Session strip`).

## Driving it with ui.py

```sh
S=.claude/skills/verify-rexling/scripts
label=$($S/scratch-session.sh new)
$S/ui.py tap "Session strip"
$S/ui.py tap "$label"
$S/ui.py has "$label · local"           # the header now names the scratch session
$S/scratch-session.sh kill "$label"
```

## Gotchas

- Never open the user's own sessions. That takes control and resizes their PTY.
- A session created on the server shows up when the sheet opens. Pull the list down if it doesn't.
- Making a session with the app's `New session` leaves a non-scratch session behind; kill it with `rex kill <label>`.
