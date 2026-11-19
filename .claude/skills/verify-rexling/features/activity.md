# Activity

The user sees which programs across connected servers need attention, are working, or have finished, and jumps to one.

## Sub-features

- Filters: `All`, `Needs you`, `Working`, `Finished`. Entries are grouped by server, with counts. Each entry is labelled `<title>, <state>, <session>, <server>`.
- `Mark seen` and `Refresh activity`. Tapping an entry opens and takes control of that exact pane.
- The bell (`Activity`) in the header: an amber count for what needs you off screen (other sessions and this session's other tabs), a small blue dot while something works. Its live check is `test_activity_badge.py`, which needs approval.

## How to get to it (user POV)

The bell (`Activity`) at the right of the header.

## Driving it with ui.py

```sh
S=.claude/skills/verify-rexling/scripts
$S/ui.py tap "Activity"; sleep 2
$S/ui.py tap "Working"; $S/ui.py list | grep -E ', (Working|Finished|Needs you),'
$S/ui.py tap "Close activity"
```

## Gotchas

- Tapping an entry for one of the user's sessions takes control of it. Read the list, but tap only entries in scratch sessions.
- Programs must report Rex status; quiet shells don't appear.
