# ArrayCrvOnSrf

`ArrayCrvOnSrf` (`ArrayCurveOnSurface`) copies selected objects along a curve,
using the curve tangent as X and the surface normal as up. The base point and
construction plane define the original frame. The curve and surface stay unchanged.

Start the command, select sources if needed, pick a base point, select a curve
near the end to start from, and select a surface or B-rep face. Pick positions
on the curve or type distances from the last accepted position. `Divide` asks
for a total count; `Multiple` asks for a copy count and spacing. Both counts must
be at least two and share a remembered default. Multiple's spacing is nonnegative;
zero creates no copies. Multiple stops at the end of the curve, including a closed
curve. Divide includes both ends of open curves and omits the repeated seam of
closed curves. At a polyline corner, the next segment supplies the tangent.

`SubCrv` makes a temporary directed portion with the existing start/end, length,
and direction-lock getters. Its final endpoint supplies the shortened curve's
starting end. The source curve is unchanged. Enter at the placement prompt keeps
that prompt active; Escape ends the command and retains accepted copies.
Repeated placements share one Undo entry. Preselected sources remain selected;
command-first selection and SubCrv release source selection. Copied objects retain
attributes. A single grouped source creates empty group definitions, while a set
of sources receives independent copied memberships at each position. Undo retains
those definitions. Automatic group names avoid previously used names, including
deleted names and case variants.

Scripts can supply complete arguments:

```text
ArrayCrvOnSrf 0,0,0 Curve=rail-id Surface=surface-id Divide=8
ArrayCrvOnSrf 0,0,0 Curve=rail-id Surface=surface-id Multiple=4,2.5 Offset=1
ArrayCrvOnSrf 0,0,0 Edge=brep-id,2 Surface=brep-id Face=0 Positions=1,2,3;4,5,6
ArrayCrvOnSrf 0,0,0 Curve=rail-id Surface=surface-id Distances=3,3,-2
```

Optional arguments are `SubCrv=start,end` (curve parameters), `PickPoint=x,y,z`
(choose the starting end), `Reverse=Yes|No`, `Sources=id,...`, and
`PickedSources=id,...`. `Divide=Default` and `Multiple=Default,Default` use the
remembered values. `Offset` applies to distances and Multiple. Out-of-range
script distances reject the entire invocation without an edit. Interactive input
rejects just that placement. Scripts may use `Finish=No` while joining a command
history group. Up to one million copied objects are allowed per invocation.

The solver reuses captured curve/surface references and the existing arc-length
sampler. Its cursor preview is read-only, and guide changes invalidate the preview.
It uses the existing usd-brep curve trimming path when the native backend is enabled.

[Validation receipt](../array-curve-surface-validation.json) records 44 public
Rhino 8 command workflows, including physical path picks, planar and curved
surfaces, reversed paths/faces, construction planes, groups and real Undo/Redo.
The API and interactive replay compare all five points in every copied basis
cloud, with a maximum allowed distance of `1e-6` model units. This corpus does
not qualify arbitrary trimmed-face boundaries, singular normals, or every
source geometry type. Degenerate placement frames reject without creating copies.

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/arraycrv.htm).
