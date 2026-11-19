# Rexling commands: design review

Open `index.html` in a browser for the interactive comparison or view `comparison.png` for the rendered side-by-side mockups. `preview.py a` and `preview.py b` render the same review states in a terminal. Neither preview connects to Rex or runs real commands.

To show the actual renders in Rex panes, run `python3 render-pane.py a --kitty` and `python3 render-pane.py b --kitty` in Rexling (requires Pillow). Press **z** for detail, **↑/↓** to scroll, and **q** to close. Omit `--kitty` for capability detection and the lower-resolution terminal-cell fallback.

| | A · Bottom sheet | B · Above-keyboard tray |
|---|---|---|
| Tap Commands in the extra-key row | Temporarily hide the terminal IME, show the full list | Keep the IME visible, show three quick actions |
| Find another command | Tap search in the sheet | Tap **More commands…** to open the sheet |
| Dismiss | Restore terminal focus and the prior keyboard visibility | Close the tray without changing the keyboard |

**Recommendation: A.** One tap gets to the full set, including with tall keyboards or short viewports. B is useful if keeping the keyboard visible makes common actions noticeably faster; it adds a second tap for everything else.

- Make Commands an optional key in the existing extra-key editor. Keep the default layout unchanged; the stock six keys leave a seventh slot. The highlighted terminal icon in the visual mocks marks the launch key. Its accessible name is **Commands**.
- **⋮ → Commands** opens the full sheet even if the terminal keyboard is hidden.
- Capture the displayed tab/pane when opening. Filter actions by capability and control state, and revalidate the target before dispatch. If the target is gone, dismiss rather than acting on the newly focused pane.
- The key is local UI, not terminal input. Opening and dismissing must not submit an unfinished IME line. Search is opt-in; if its keyboard is open, Back dismisses that keyboard before the sheet. Commands with arguments hand off to the existing picker/dialog where possible.
- V1 should reuse existing local commands and supported Rex operations, not mirror the Mac palette or discover arbitrary server actions. Keep tab and pane navigation where it is.
