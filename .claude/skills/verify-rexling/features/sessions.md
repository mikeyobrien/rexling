# Sessions

The user picks which Rex session to view, finds one, or creates one.

## Sub-features

- The `Sessions` sheet lists sessions under each server. Each row shows the session name, then `this device · 2 tabs · 4 panes` or `detached · …`, plus `current` and `Session actions for <name> on <server>`.
- `Find session…` filters the list. `New session` creates one.
- `Terminal menu` → `New session` and `Refresh sessions`.
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
- A session created on the server shows up when the sheet opens. Use `Refresh sessions` if it doesn't.
- Making a session with the app's `New session` leaves a non-scratch session behind; kill it with `rex kill <label>`.
