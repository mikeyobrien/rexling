# Tabs and panes

The user switches between the tabs and panes of the current session, and manages them, in one card.

## Sub-features

- The switcher card lists the session's tabs (`Tabs`). A tab with several panes has `Show panes` / `Hide panes` and lists them in place; the card opens with the current tab's panes showing.
- Tapping a tab or pane row opens it. `New tab`, `Split right` and `Split below` sit under the list.
- Swiping a row sideways reveals `Close <title>`. Holding a row opens `Row actions`: `Rename`, `Split right`, `Split below`, `Close`, plus `Float` or `Dock` on panes. Holding and then dragging reorders.
- Closing asks in the row (`Close build?`, `ends cargo`, `Cancel`, `Close`) only when a program would end. An idle shell just closes.
- The heading (`Sessions`) goes up to the session list in the same card.
- On wide screens the same rows and gestures are in the `Tab list` beside the terminal.
- Swiping across the terminal switches panes. Circular navigation is covered by `SessionHeaderLoopTest` and `PaneChromeTest`.

## How to get to it (user POV)

Tap the title or the program icon in the header. The pane dots under a split terminal open the same card.

## Driving it with ui.py

```sh
S=.claude/skills/verify-rexling/scripts
label=$($S/scratch-session.sh new)
rex split -s "$label" --focus=false            # give the scratch session a second pane
$S/ui.py tap "Session strip"; $S/ui.py tap "$label"
$S/ui.py tap "Session program icon"; $S/ui.py list   # expect two pane rows under the tab
$S/ui.py hold "2 panes"; $S/ui.py has "Row actions"; $S/ui.py tap "New tab"   # a tap outside only dismisses the menu
$S/ui.py tap "Close switcher"
$S/scratch-session.sh kill "$label"
```

`SwitcherTest`, `SwitcherReorderTest` and `TerminalEditActionsTest` cover the card without a server.

## Gotchas

- Closing a tab or pane in the app ends its programs on the server. Do this only in scratch sessions.
- `ui.py tap "Close"` matches `Close sheet` first and dismisses the card. Use the full label (`Close bash`) or `--exact`.
- `adb shell input draganddrop` starts moving at the long-press timeout and never reorders; use `ui.py drag`.
