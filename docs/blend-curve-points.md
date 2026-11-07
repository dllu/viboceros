# BlendCrv point endpoints and partial joins

[Command](commands/blend-curve.md) · [Qualification record](blend-curve-points-validation.json)

One BlendCrv endpoint can be a free point. It supplies position only and forces
G0, while the curve endpoint supports G0–G4. Curve-to-point defaults use chord
length as endpoint speed, independent of degree. Explicit shape lengths and
bulges retain their usual meaning. Point placement and movement share drafting,
object snaps, typed coordinates and transient previews; no point object is
created or owned by the command.

Join with Trim disabled includes only whole curves connected to the blend at
their endpoints. Disconnected originals remain unchanged and outside the result.
If none connects, Rhino creates a PolyCurve containing just the blend. The
result inherits the first participating source's attributes and identity, or
current-layer attributes when no original participates. Trim enabled retains
source portions and joins them normally. All changes share one undo entry.

The bounded record retains 42 native observations: 41 matched workflows and
one incomplete second-Point attempt. Three matched workflows additionally cover
two-curve partial joining. Both builds check geometry, original object identity,
attributes, selection, undo/redo and 82 3DM round trips, using 1,254 native
point/tangent stations and 441 derivative stations through fourth order.
Sources include lines, an analytic arc, polynomial and rational NURBS. Local
windowless tests also cover hover/acceptance, 3D point snapping, typed input,
adjustment cancellation, pending redo and stale references.

Input evidence matters for the spatial G4 case. Initial public prompts restore
remembered BlendStart settings. Public
[PickContext](https://developer.rhino3d.com/api/rhinocommon/type/rhino.input.custom.pickcontext/)
and [ObjRef.CurveParameter](https://developer.rhino3d.com/api/rhinocommon/rhino.docobjects.objref/curveparameter)
measurements precede command execution. One original receipt lacked the selected
parameter and its approximate ClosestPoints result exceeded the coefficient
gate. A fresh calibration supplies the missing input only when prescribed source
geometry, view, construction plane, tolerances, source index, pixel and viewing
ray agree exactly. The natural-end calibration's selected pick point differs
from its blend endpoint, which also checks that calibration does not use output
coordinates. Native outputs never seed construction or input matching.

An earlier fixture opened a selection menu because two sources overlapped. It
was interrupted after screenshot verification. Its producer snapshots, worker
log and screenshot are retained and contribute no native observations. Some
holdout IDs describe intended shape or copied-portion edits; remembered PickPoint
changed the actual selected control or join behavior. Their actual inputs and
outputs remain intact, and an explicit CurveEnd case verifies shape editing.

Geometry uses the existing `1e-6` gate. Normalized derivative relative error and
approximate native length-domain error retain separate `1e-6` and `2e-6` gates;
absolute derivative errors are recorded. The inherited
[higher-order limits](blend-curve-high.md), including Native's zero-fourth G4
construction policy, still apply.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_points
python3 -m tools.rhino_oracle.report_blend_curve_points --implementation-revision <full-sha>
```

Captures use private Xvfb; tests create no desktop windows. Native snap and
numeric/Escape trajectories, exact hit-box/depth ranking, independent native
pick-frustum parameter reproduction, curved-source join ownership, closed
sources, SubCrv, symmetry/Alt edits, curvature display, history re-editing,
continuous distance and comparative speed remain unqualified.
