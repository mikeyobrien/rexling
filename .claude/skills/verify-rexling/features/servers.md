# Servers and connection

The user adds a Rex server by URL (HTTPS or `ssh://user@host`), and Rexling connects and lists its sessions.

## Sub-features

- Add server dialog: an icon picker (`Choose server icon`), `Name`, `Server URL`, a `Connection network` choice (`Phone network / VPN` or built-in Tailscale), and `Save & connect`.
- The header status line: `disconnected · No server` before any server, then `<session> · <server>`.
- `Terminal menu` → `Servers` manages saved servers. The row adds `N connected` when there are several servers or none is connected.
- SSH and built-in Tailscale servers: `docs/connections.md`. Neither is driven by this skill yet.

## How to get to it (user POV)

On a fresh install: the welcome card's `Add server` button. Afterwards: `Terminal menu` → `Servers`.

## Driving it with ui.py

```sh
S=.claude/skills/verify-rexling/scripts
$S/ui.py tap "Add server"; sleep 1
$S/ui.py tap "Server URL"; $S/ui.py type "$REXLING_SERVER_URL"     # see `rex server status`
$S/ui.py tap "Name";       $S/ui.py type "local"
$S/ui.py tap "Save & connect"
$S/ui.py has " · local" --timeout 20     # connected: the header shows a session on "local"
```

## Gotchas

- Tap a field's label before typing. Text goes to whichever field has focus, and the dialog animates in, so wait about a second after opening it.
- Instrumented tests wipe app data, including the saved server, so re-add it afterwards.
- The emulator reaches the tailnet only through this host's Tailscale. If the health check's ping fails, check `tailscale status` on the host.
