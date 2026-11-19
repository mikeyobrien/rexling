# Terminal control and input

The user takes control of a pane and types. Keys go to the server's PTY, and output streams back.

## Sub-features

- Taking control: tap `Terminal output`, or open a session. The `Terminal menu` shows `In control` or viewing, the grid size (`63 × 50`) and `Release control`. When this device is not in control, the header's second line reads `Viewing only` (content-desc `Status`) with `Take over` beside it. While the connection is down it reads `Offline`, `Connecting…` or `Reconnecting…` with `Retry`, and the terminal dims.
- Keyboard accessory (`Terminal keys`): `Escape`, `Control`, `Alt`, `Shift`, Tab, the arrow group and the keyboard toggle. Holding `Control` opens a 3×3 popup (^C ^D ^Z ^R ^L ^A ^E ^W ^J).
- Links, selection and copy: long-press. Mouse-mode taps go to TUI apps. See `rexling-android/TESTING.md`.

## How to get to it (user POV)

Open a session and tap the terminal. The soft keyboard and key row appear.

## Driving it with ui.py

`scripts/smoke.sh` is the canonical recipe: scratch session, take control, `echo <nonce>`, then check the server capture.

```sh
.claude/skills/verify-rexling/scripts/smoke.sh   # prints PASS/FAIL and the evidence path
```

Latency: `.claude/skills/verify-rexling/scripts/latency.sh 40 0.3` reports p50 and p95 for each stage. A regression shows up as a stage moving, not just the total.

## Gotchas

- Proof is the server capture containing the nonce as its own output line, not the screenshot.
- `ui.py type` sends text through `adb input`. Characters `adb input text` can't send (such as `'`, `"` and `&`) need care. Prefer plain alphanumeric nonces.
- `key BACK` with the keyboard hidden exits the app.
