# ArrayCrv

Start `ArrayCrv` (`ArrayCurve`) with selected objects, or select them at its first
prompt. Pick an ordinary curve or a B-rep edge near the intended starting end.
An open guide uses the half of its parameter domain containing the pick to choose
the starting end. `Basepoint` before the path pick supplies an explicit source
anchor. `SubCrv` opens the shared temporary start/end, length and locked-direction
getters; the original guide remains unchanged.

The instances appear in the viewport without adding objects or history records.
Type an item count of at least two to accept immediately. Choose `Items` or
`Distance` to adjust the preview, and press Enter to accept. `Orientation` chooses
`Freeform`, `Roadlike`, `Stairlike`, or `NoRotation`; spacing and orientation are
remembered across invocations. Esc cancels and preserves the Redo branch.

Counts include both open endpoints and omit a repeated closed seam. Fixed
spacing leaves a shorter remainder. Without a base point, originals supply the
first instance. With a base point, originals remain alongside all requested
copies. Closed paths start at the closest location to the source bounding-box
center, or to the explicit base point. Roadlike and Stairlike use the construction
plane of the viewport in which the guide was picked.

Acceptance creates one undo step and preserves source names and attributes.
Command-first selection and temporary SubCrv release source selection. Arrays
of multiple sources reproduce their group memberships per instance. A single
source produces empty copied group definitions without memberships. Undo retains
the allocated group definitions, matching the captured Rhino behavior.

Complete API examples:

```text
ArrayCrv 8 Orientation=Freeform PathName=Rail
ArrayCrv Distance=2.5 Orientation=Roadlike BasePoint=0,0,0 Curve=rail-id
ArrayCrv Items=6 Edge=brep-id,0 PickPoint=8,0,0 Sources=source-id
ArrayCrv 4 Curve=rail-id SubCrv=2,8 Reverse=Yes
```

`Curve`/`Edge`, `Sources`/`PickedSources`, temporary ranges and picks make script
calls independent of current selection. The legacy last-selected-path form is
also supported. API item count one and spacing at least the rail length retain
the older no-copy behavior without a base point; the interactive getter follows
Rhino's stricter count and distance limits. Arrays are bounded to one million
new objects before any document mutation.

[Rhino's ArrayCrv documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/arraycrv.htm)
describes the orientation modes. The checked-in public-command captures verify
placement, option transitions, selection, groups and real Undo/Redo; see
[the validation receipt](../curve-array-validation.json).
The 62 captured workflows all replay document state and real history. Of these,
57 match every witness point within 1e-6 model units. The receipt lists five
geometry diagnostics separately: four Roadlike/Stairlike cases have vertical
singularities where this Rhino build produces collapsed or indeterminate maps;
Viboceros keeps finite rigid frames. One smooth closed Freeform path differs by
about 2.7e-5 units, consistent with the native frame integration approximation
measured in the independent [curve-frame diagnostics](../curve-frames.md).
These five cases are retained in both API and UI replays and are not counted as
passing the 1e-6 geometry threshold.

Freeform transport at some sharp spatial corners also differs from Rhino's
query-dependent frame restart. The pilots preserve those observations; this
change keeps the stable shared transport solver. Planar corner captures verify
outgoing tangents for Roadlike/Stairlike across native polylines, degree-one
NURBS and polycurves. General trimmed-edge topology, automatic closed seam ties,
grip sources and large or pathological arrays remain outside this capture's
qualification.
