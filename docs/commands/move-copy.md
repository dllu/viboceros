# Move and Copy

[Transforms](transforms.md) · [Source selection](transform-sources.md)

Enter `Move` or `Copy`, select sources if needed, and press Enter to begin
point input. Pick a base point and a destination. Move finishes after its
destination; Copy keeps accepting destinations until Enter or Escape.
All accepted copies belong to one Undo entry, including when Escape ends the
session. Undo entered during the point prompt leaves the accepted copies intact.

At the initial base prompt, Enter uses the combined World bounding-box center.
Move's Vertical base prompt instead cancels on Enter. Enter at Move's destination
prompt keeps waiting for a point. Copy's `InPlace` option creates one batch at
the original positions and finishes immediately.

## Placement options

`Vertical` at the base prompt constrains the first destination to a line through
the base along the active construction plane's positive Z direction. A signed
distance accepts a point immediately along that line, including negative and
zero distances. Copy also supports `Vertical=Yes|No` at the base prompt.

After Copy's first placement, these options become available:

| Option | Effect |
| --- | --- |
| `FromLastPoint=Yes` | Use the last resolved destination as the next placement reference. |
| `UseLastDistance=Yes` | Keep the previous placement's distance from the current reference. |
| `UseLastDirection=Yes` | Project onto the previous direction's infinite line. |

The options begin at No for each normal Copy session. A Vertical first copy
turns on UseLastDirection along positive CPlane Z. Turning it off permits free
placement. The line orientation remains stable through negative or zero picks.
A signed scalar on a constrained line overrides UseLastDistance for that pick.
A zero previous step leaves the distance lock inactive; picking the current
reference still accepts a zero step.
Copies always derive from the original sources and original base; FromLastPoint
changes the reference used to resolve the next destination.

Free mouse line picks use the closest point to the 3D viewing line, so they work
with an edge-on CPlane and perspective views. Typed points and object snaps
supply explicit 3D coordinates.

## Selection and history

Preselected sources follow document order; command-first sources follow pick
order. Move renews transformed objects after untouched objects and clears
command-first selection. An exact identity Move retains selection and has no
geometry history entry. Copy retains source selection, leaves copies unselected,
and creates new objects even for an identity placement.

Copy Undo clears source selection for both preselected and command-first sources.
Copied group definitions remain empty after Undo. SelLast identifies all accepted
copies, and Redo restores the output selection captured before Undo. Multiple
sources retain their copied group topology; a single source receives an ungrouped
copy and corresponding empty group definitions.

Full invocations also support these placements, for example:

```text
Move 0,0,0 3,4,5
Copy 0,0,0 3,0,0 FromLastPoint=Yes 6,0,0 9,0,0
Copy InPlace
Copy Vertical 0,0,0 -3.5 2
```

## Verification and limits

Rhino runs through `run_headless.sh` in private Xvfb. Inputs are prescribed before
capture; the raw observations retain table order, attributes, selection, groups,
SelLast, Undo, Redo, and command events.

| Recipes | Raw observations | Coverage |
| --- | --- | --- |
| [110 placement cases](../../tools/rhino_oracle/fixtures/translation.json) | [Results](../../tools/rhino_oracle/observations/translation.json) | Source selection, grouped sources, repeated copies, automatic bases, Vertical, repetition options, cancellation. |
| [18 mouse cases](../../tools/rhino_oracle/fixtures/translation_mouse.json) | [Results](../../tools/rhino_oracle/observations/translation_mouse.json) | Real destination clicks in Front, Right, and Perspective; World/tilted CPlanes; direction locks and FromLastPoint. |
| [36 edge cases](../../tools/rhino_oracle/fixtures/translation_edges.json) | [Results](../../tools/rhino_oracle/observations/translation_edges.json) | Zero steps, negative/zero distances, direction orientation, destination Enter. |

App comparisons use 1e-9 absolute coordinate tolerance. The command registry also
replays applicable full invocations independently of the point prompt.

Move Normal, SubCrv/subobject transforms, direct dragging/nudging, and live source
geometry previews remain unimplemented. Native mouse/window source ordering has
not been exhaustively measured.

```sh
cargo test -p viboceros --bin viboceros move_copy
python3 -m unittest tools.rhino_oracle.test_transform_copy
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/translation.json --scheme VibocerosOracleMirrorPreview --timeout 300
```

Published option descriptions: [Rhino Move help](https://docs.mcneel.com/rhino/8/help/en-us/commands/move.htm),
[Rhino Copy help](https://docs.mcneel.com/rhino/8/help/en-us/commands/copy.htm).
