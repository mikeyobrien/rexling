# Servers and connection

The user adds a Rex server by URL (HTTPS or `ssh://user@host`), and Rexling connects and lists its sessions.

## Sub-features

- Add server dialog: an icon picker (`Choose server icon`), `Name`, `Server URL`, a `Connection network` choice (`Phone network / VPN` or built-in Tailscale), and `Save & connect`.
- The header status line: `disconnected · No server` before any server, then `<session> · <server>`.
- `Terminal menu` → `Servers` manages saved servers. The row adds `N connected` when there are several servers or none is connected.
- SSH and built-in Tailscale servers: `docs/connections.md`. Neither is driven by this skill yet.

## How to get to it (user POV)

On a fresh install: the `Connect to Rex` card. Type the host into `Server address` (a bare host means HTTPS; the name comes from the host) and tap `Connect`. `Name, icon, network` opens the full Add server dialog with the address filled in. Afterwards: `Terminal menu` → `Servers`.

## Driving it with ui.py

```sh
S=.claude/skills/verify-rexling/scripts
$S/ui.py tap "Server address"; $S/ui.py type "${REXLING_SERVER_URL#https://}"     # see `rex server status`
$S/ui.py tap "Connect" --exact
$S/ui.py has "Session strip" --timeout 20     # connected: the header shows a session
```

## Gotchas

- On a wide screen (the unfolded Fold) sheets do not rise from the bottom: Preferences, Activity, Servers and the rest fill the terminal side, and the action card and switcher sit at its bottom. The tab list stays usable beside them; `Hide tab list` leaves a `Tab rail` of program tiles, and `Show tab list` brings the list back.
- Tap a field's label before typing. Text goes to whichever field has focus, and the dialog animates in, so wait about a second after opening it.
- Instrumented tests wipe app data, including the saved server, so re-add it afterwards.
- The emulator reaches the tailnet only through this host's Tailscale. If the health check's ping fails, check `tailscale status` on the host.
