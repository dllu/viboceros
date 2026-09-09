# Point and Points

[Command reference](README.md) · [Coordinate input](../point-input.md)

`Point x,y,z` creates one point object; bare `Point` (or `Pt`) starts one viewport
pick. `Points` repeatedly places independent point objects until Enter or Escape.
Accepted points are immediately visible and available to object snapping.
Duplicate locations are retained. Each point uses the current layer.

```text
Point 1,2,3
Points 1,2,3 4,5,6 1,2,3
Points 1,2,3 4,5,6 Undo 7,8,9
```

In an interactive Points session, type `Undo` to remove the most recently placed
point, without undoing earlier commands. Undo on an empty session is a no-op.
Typed coordinates and viewport picks share the same placement path; malformed
coordinates leave the current session available for retry. Relative input uses
the previous accepted point; undoing all session points restores the prior
relative-input base.

Unlike a cancelled unfinished curve, **Escape keeps accepted Points**. Starting
another modeling command or using a layer-sidebar action also finishes the session,
so unrelated document edits cannot join its transaction. A nonempty native session
commits as one document undo entry. A session with no remaining points rolls back,
preserving prior geometry, selection, and redo. The complete typed command stages
all coordinates and local Undo actions before making any document edits.

## Verification and limits

The [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/point.htm)
documents repeated placement and command-local Undo. Seven live Rhino 8.32
fixtures cover duplicates, local Undo, empty input, Undo-all, and Escape with or
without points. Native fixture replay and independent Python checks validate the
accepted point sequence and unselected output state.
All seven live comparisons passed with zero coordinate difference.

The [initial raw response](../../tools/rhino_oracle/observations/points_command.json)
also contains attempted outer document Undo/Redo observations made inside the
Python worker. Those did **not** remove the placed points and do not establish
normal command-history behavior. The current comparison deliberately tests only
accepted points. Native single-entry history, redo, session retry, and switching
commands are separately covered by command/UI tests; Rhino outer-history parity
remains unverified.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/points_command.json --timeout 180
```
