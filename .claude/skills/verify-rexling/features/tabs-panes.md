# Tabs and panes

The user switches between the tabs and panes of the current session.

## Sub-features

- A bottom tab bar (`Tabs`, with each tab labelled `Tab <title>, <program>`). Long-pressing a tab closes it, after a confirmation.
- The `Tabs and panes` sheet has `Tabs` and `Panes` counts, `Reorder tabs`, and pane rows such as `Viewing · 1 pane · /tmp`. Close it with `Close tab and pane switcher`.
- Swiping across the terminal switches panes. Circular navigation is covered by `SessionHeaderLoopTest` and `PaneChromeTest`.

## How to get to it (user POV)

The pane button beside the eye at the right of the bottom bar.

## Driving it with ui.py

```sh
S=.claude/skills/verify-rexling/scripts
label=$($S/scratch-session.sh new)
rex split -s "$label" --focus=false            # give the scratch session a second pane
$S/ui.py tap "Session strip"; $S/ui.py tap "$label"
$S/ui.py tap "Tabs and panes"; $S/ui.py list   # expect the Panes count to be 2
$S/ui.py tap "Close tab and pane switcher"
$S/scratch-session.sh kill "$label"
```

## Gotchas

- Closing a tab or pane in the app ends its programs on the server. Do this only in scratch sessions.
