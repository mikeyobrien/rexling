# Rexling feature map

One file per user-facing feature. Each has: what it is, how a user reaches it, how to drive it with `scripts/ui.py`, and what proves it works.

| Feature | File | Proof |
|---|---|---|
| Servers and connection | [servers.md](servers.md) | Header shows `<session> · <server>`; `Terminal menu` → `Servers` |
| Sessions | [sessions.md](sessions.md) | Header names the chosen session; `rex ls` shows new ones |
| Terminal control and input | [terminal-input.md](terminal-input.md) | `smoke.sh` PASS (the server ran what the app typed) |
| Tabs and panes | [tabs-panes.md](tabs-panes.md) | Pane count and active tab match `rex block ls -s <scratch>` |
| Activity | [activity.md](activity.md) | Entries grouped by server, and filters change the list |
| Staying connected off screen | [background.md](background.md) | Server log shows no disconnect during a short trip away, and a disown 45 seconds into a long one |

Not mapped yet: Appearance and themes, Font size, Port forwarding, Attach image, the keyboard toolbar editor, and built-in Tailscale (see `rexling-android/TESTING.md`, `docs/connections.md`).
