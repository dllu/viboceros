# Point selection

## Directional selection

`NextU`, `PrevU`, `NextV` and `PrevV` shift the selected curve or surface
controls by one logical index. `AddNextU`, `AddPrevU`, `AddNextV` and `AddPrevV`
retain the original picks and add their neighbors. Each operation uses the
selection at command start, so adding neighbors does not cascade through a net.

`SelU` and `SelV` extend the picks to complete control-net rows in the named
parameter direction. Both directions follow the curve on curve controls.
Surface indices follow the displayed net, independent of the camera or CPlane.

Periodic directions and analytic circle controls wrap. Open and closed clamped
NURBS directions stop at their boundaries: a shifted end pick disappears if it has no neighbor. Edit-point
grips have no logical neighbors; shifting clears their picks, additive commands
retain them, and row commands leave them alone. Mesh grips are excluded and
retain their selections. These rules follow the captured Rhino commands.

The commands preserve geometry, object attributes, groups, ordinary object
selection, grip display, Undo and Redo. They accept no arguments and also work
inside the grip-capable source getters described below. For example, pick a
surface control, run `SelU`, then `AddNextV` to select two adjacent U rows for
the next transform.

The [validation receipt](../grip-navigation-validation.json) covers 25 Rhino
workflows and 1,650 completed command invocations. It includes rational,
analytic, composite, closed and periodic curves, curve edit points, open/closed
and periodic surfaces, meshes, mixed owners, boundaries, empty/all selections,
and chained operations. API and application tests compare every grip identity,
selection state and position, and retain a real Redo witness. Positions use
`1e-8`; indices, flags and command results compare exactly. General native
mouse/getter parity and performance remain unqualified.

## Interactive surface rows

`SelUV` selects a surface, enables its control points, and accepts repeated row
picks. Choose `Direction=U`, `Direction=V` or `Direction=Both`; `Toggle` exchanges
U and V, and leaves Both unchanged. Each pick adds its rows. Enter and Escape
both keep accepted picks and leave the grips enabled. Options survive either
exit. The source selection is released when the getter ends; unrelated ordinary
preselection survives. Postselection clears the previous selection.

Row selection uses the largest non-rational B-spline basis factor at the surface
parameter closest to the supplied point. Periodic and closed seam aliases share
one logical control. This matters on rational surfaces and nonuniform knot
vectors: closest control-polygon distance can select a different row. Mouse
input resolves a point on the selected surface; typed world coordinates use the
same surface query. A source or grip-display change ends the getter safely.

For scripting, select one surface and execute:

```text
SelUV Points Direction=Both 1,1,0.25 Direction=U 0,0,0
```

The registry remembers options, also accepts `Toggle` and `HideOthers=Yes|No`,
and preserves geometry, attributes, groups, Undo and Redo. Malformed batch input
is rejected before changing selection or display. UI point input supports the
existing coordinate systems, units, point filters and constraints.

The [SelUV validation receipt](../select-uv-rows-validation.json) records 125
completed Rhino workflows on rational quadratic, cubic, nonuniform, closed and
periodic surfaces. Native, registry, session and application replays compare
complete grip selections and locations, owner/peer selection and visibility,
initial/enabled grip display, repeated commands, cancellation and Redo. Native
EndCommand and post-script states are retained separately. Positions use `1e-8`;
discrete fields compare exactly.

`HideOthers` is parsed and remembered. The captured getters and completed states
retain object and grip visibility, which this implementation follows. General
native mouse display effects of this option remain unqualified. Trimmed-face
picking, ambiguous surface intersections, SubD, hidden individual grips and
performance comparisons need further validation.

```sh
cargo test -p viboceros-command -p viboceros-oracle -p viboceros select_uv_rows
python3 -m unittest tools.rhino_oracle.test_sel_uv
```

## SelPt and InvertPt

`SelPt` adds selectable point objects and every displayed control or curve edit
grip to the selection. Existing object picks remain selected. Grip owners and
group peers are selected only if they were already selected; point cloud samples
remain part of their point-cloud object.

`InvertPt` exchanges selected and unselected visible grips. It preserves ordinary
object picks and the geometry. Running it twice restores the original grip picks.
Hidden, locked, disabled or stale grip displays are excluded. Coincident mesh
grips retain their separate `(object ID, index)` identities.

Both commands preserve Undo and Redo. They accept no arguments, support the usual
case-insensitive command names and underscore/hyphen prefixes, and retain the
enabled grip display. `SelNone` clears selections while retaining that display;
`PointsOff` also hides the grips.

You can use these selectors during the source-selection phase of Move, Copy,
affine grip transforms, SetPt, Smooth and Circle FitPoints. The active source
getter remains open. SelPt respects its object filter: for example, Smooth
accepts displayed grips while excluding ordinary point objects. Circle FitPoints
can select both. A malformed selection command leaves the prompt and selection
unchanged. Enter then accepts the selected sources and continues the operation.

For example, create and select a curve, run `EditPtOn`, then enter:

```text
SetPt XSet=No YSet=No ZSet=Yes
SelPt
<Enter>
0,0,5
```

The command selects the displayed edit stations through the SetPt source getter
and aligns them to world Z=5. To edit the complement of a mouse-picked set, run
`InvertPt` before starting the transform.

McNeel's [command reference](https://docs.mcneel.com/rhino/8/help/en-us/commandlist/command_list.htm)
includes grips in SelPt, while the detailed
[selection page](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm)
describes SelPt as point-object selection and describes InvertPt's visible-grip
behavior. This implementation follows the broader SelPt reference. Independent
regressions cover both grip modes, grouped point objects, hidden/locked displays,
coincident mesh controls, history preservation, filtered getters, Circle fitting
and keyboard SetPt input. Fresh native SelPt/InvertPt getter parity and
performance remain unqualified. Solid grips, SubD grips, per-grip hiding,
control-point range selection need further work.

The [six pending Rhino recipes](../../tools/rhino_oracle/fixtures/point_selection.json)
use the public command/SDK probe and have no captured native results. The
[qualification record](../point-selection-qualification.json) retains source
hashes, test results and these limits.

```sh
cargo test -p viboceros-command point_selection
cargo test -p viboceros point_selection
```
