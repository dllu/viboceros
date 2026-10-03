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

## Move Normal

At Move's base prompt, enter `Normal`, select a curve, surface, or polysurface,
then pick a base location on that reference. The reference pick leaves source
selection unchanged. Enter a signed distance or pick a destination projected
onto the normal line. Enter at the base cancels; Enter at the destination locks
the remembered positive distance and waits for a point to choose the direction.
A remembered zero leaves that lock inactive. Move displays the magnitude of the
last successful move.

Curve normals follow principal curvature, pointing inward on circles and arcs.
Reversing a curve preserves that direction. Straight lines and straight polyline
segments have undefined normals and end the command without changing geometry.
Surface normals follow the underlying surface orientation; reversing a surface
reverses the direction, while reversing a B-rep face does not.

`IgnoreTrims=Yes|No` is available while selecting the reference and is remembered
even when the prompt is canceled. Typed locations resolve to the closest reference
point; mouse surface locations resolve against the original surface rather than
accepting tessellation coordinates. Full invocations use:

```text
Move Normal=reference-id 3,0,0 -2
Move Normal=reference-id Face=0 IgnoreTrims=Yes 3,3,2 8,-3,9
```

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
| [102 Normal cases](../../tools/rhino_oracle/fixtures/move_normal.json) | [Results](../../tools/rhino_oracle/observations/move_normal.json) | Curve/surface orientation, signed distances, projected targets, off-reference bases, cancellation, selection and history. |
| [20 Normal input cases](../../tools/rhino_oracle/fixtures/move_normal_edges.json) | [Results](../../tools/rhino_oracle/observations/move_normal_edges.json) | Actual reference and base clicks in Top/Perspective, surface orientation, IgnoreTrims options. |
| [8 Normal trim cases](../../tools/rhino_oracle/fixtures/move_normal_trims.json) | [Results](../../tools/rhino_oracle/observations/move_normal_trims.json) | IgnoreTrims on a trimmed curved surface; typed points outside the retained face; selection and history. |
| [8 Normal default cases](../../tools/rhino_oracle/fixtures/move_normal_defaults.json) | [Results](../../tools/rhino_oracle/observations/move_normal_defaults.json) | Enter locks distance while the next point chooses direction; negative moves and zero defaults. |

App comparisons use 1e-9 absolute coordinate tolerance, with 2e-8 for free curve
mouse picks. Those locations are bounded screen-space minimizations; Rhino's
circle pick also differs from the analytic ray reference by more than 1e-9.
Raw observations are retained. The command registry also
replays applicable full invocations independently of the point prompt.

SubD normal references, SubCrv/subobject transforms, direct dragging/nudging, and live source
geometry previews remain unimplemented. Native mouse/window source ordering has
not been exhaustively measured.

```sh
cargo test -p viboceros --bin viboceros move_copy
python3 -m unittest tools.rhino_oracle.test_transform_copy
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/translation.json --scheme VibocerosOracleMirrorPreview --timeout 300
```

Published option descriptions: [Rhino Move help](https://docs.mcneel.com/rhino/8/help/en-us/commands/move.htm),
[Rhino Copy help](https://docs.mcneel.com/rhino/8/help/en-us/commands/copy.htm).
