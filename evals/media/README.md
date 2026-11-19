# Media evals

These evals score how well Rexling shows pictures that a program streams into a pane with
Kitty graphics: stills, video, a web browser and desktop apps. The sender is
[`scripts/rexcast`](../../scripts/rexcast). The score is read from what the device shows.

```sh
python3 evals/media/run.py                      # every case, about ten minutes
python3 evals/media/run.py --only video-30,tap  # some cases
python3 evals/media/test_rexcast.py             # the sender's unit tests, no device
```

A run needs the emulator from the `verify-rexling` skill (`emulator-5554`, Rexling installed
and pointed at the local Rex server). It creates a throwaway `verify-rexling-media-*` session
and removes it afterwards. Results go to `rexling-android/.build/media-evals/<time>-<name>/`:
`score.json`, a recording and screenshots per case, and each case's sender log.

## Cases

| Case | What it does | Full marks |
|---|---|---|
| `still` | Shows a chart of text. | Sent at the screen's resolution and pixel-exact on a screenshot. |
| `video-30`, `video-60` | A picture whose every pixel changes, at 30 and 60 frames a second. | Every frame shown, no stall over 2.5 frames, none torn, drawn within 150 ms of being sent. |
| `film-30` | The same with film grain, which lossless coding cannot shrink. | As above. |
| `film-30-slow` | `film-30` with the device's download policed to 20 Mbit/s. | As above. |
| `ui-30` | A page of text with a caret-sized change each frame. | As above. |
| `tap` | Taps a picture that changes one square in answer. | The whole picture stays visible; the answer is drawn within 100 ms (median) and 200 ms (95th percentile). |
| `tap-full` | Each tap replaces the whole picture with noise. | As `tap`. |
| `pointer` | Taps eight points, scrolls, long-presses, and long-press drags. | Taps land within 2 px; the scroll, right-click and drag all arrive. |
| `web-tap` | `tap` against a page in a real Chromium. | As `tap`. |
| `web-type` | Types seven letters into that page's text field. | All seven arrive; each echo is drawn within 100 ms. |

The total is the mean of the case scores.

## How a case is measured

The test pictures carry their frame number as a row of black and white blocks along the top
and bottom edges, after a magenta finder block. `run.py` records the device screen, finds the
blocks, and reads the number in every recorded frame. That gives frames shown per second,
the longest stall, and frames whose two edges disagree (torn). The eval page in `page.html`
draws the same blocks, so a real browser is read the same way.

Timing that a recording cannot give comes from the app's `RexlingMedia` debug log, which is
off unless `log.tag.RexlingMedia` is `DEBUG`:

- **Delay.** `rexcast --stamp` writes each frame's number as text under the picture. The app
  logs that number when it draws the frame. The eval compares the device's clock at the draw
  with the sender's clock at the send.
- **Tap latency.** The app logs the time from a finger lifting to the next new picture drawn.
- **Typing echo.** From the existing `RexlingLatency` log.

All three stop at `onDraw`, so they leave out RenderThread and the compositor, about one frame.

## Scoreboard

Emulator (`rexling_api36`, 1080 by 2400, 60 Hz, x86_64 debug build) against the local Rex
server, 2026-11-19. "Before" is `mobrienv` at `d5fea76` with only the measurement log added.

| Case | Before | After | What changed it |
|---|---:|---:|---|
| `still` | 59 | 100 | The app reports its cell size in pixels, so pictures arrive at 1008 by 2030 instead of 504 by 928. |
| `video-30` | 99 | 100 | Before is at a quarter of the pixels. After: 30.0 frames a second at full resolution, 34 ms behind the sender. |
| `video-60` | 73 | 72 | Before: 52 frames a second at a quarter of the pixels. After: 46 at full resolution. |
| `film-30` | 46 | 99 | JPEG for regions in motion: 18 frames a second at 172 Mbit/s became 30 at 39 Mbit/s. |
| `film-30-slow` | 0 | 59 | Frame acknowledgements: 0.6 frames a second arriving 7 s late became 25 arriving within 20 ms. |
| `ui-30` | 62 | 100 | |
| `tap` | 0 | 100 | A tap on a picture no longer opens the keyboard over half of it. 24 ms median. |
| `tap-full` | 0 | 97 | 106 ms median for a full-screen change. |
| `pointer` | 0 | 100 | Taps land on the exact pixel; long-press drags and right-clicks. |
| `web-tap` | 0 | 100 | 89 ms median through Chromium. |
| `web-type` | 0 | 100 | Keys go through XTest; the browser is resized in place when the keyboard opens. 64 ms median echo. |
| **Total** | **30.8** | **93.4** | |

Frame-rate cases move by ten points or more when the host is busy. Compare runs taken close
together, and trust `fps_shown` only when it agrees with `fps_drawn`.

## What is still short of a native window

- **60 frames a second.** At full resolution the emulator shows about 46. The engine decodes
  each JPEG on its one thread, about 20 ms for two megapixels.
- **The first second on a metered link.** `rexcast` grows the picture until the link pushes
  back, which costs one stall of about 1.6 s before it settles at 25 to 30 frames a second.
- **Sound.** Nothing carries audio.
- **Real desktop windows.** `rexcast` runs programs on a private X display. It does not
  capture a window that is already open on the Wayland desktop.
- **Phones.** Every number here is from an emulator. The cases run on a phone, except
  `film-30-slow`, which needs root for traffic shaping.

## Hill-climbing

Change one thing, run the cases it should move with `--only`, and read the sender's log
(`<case>.cast.log`: time, frame, tiles, bytes, encode seconds, write seconds, size level, with
`# ack` and input lines between) next to the app's log. `--rexcast "--codec rgbz"` and the
other `rexcast` options pass through, so the sender can be compared against itself.
