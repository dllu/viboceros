# Plane

`Plane` draws a rectangular planar NURBS surface. Start the command and pick or
type two opposite corners, or choose `Center`, `3Point`, `EdgeMidpoint`,
`Vertical` or `AroundCurve`. Construction modes, numeric lengths, orientation
picks and Enter defaults share the [Rectangle getter](rectangle.md).
AroundCurve accepts curves and surface/B-rep edges.

Enter cancels an empty point prompt. After an ordinary numeric length it accepts
an equal width. In 3Point and EdgeMidpoint, Enter at the width prompt uses the
picked edge distance, then Enter accepts the current preview orientation. A
numeric first edge followed by Enter accepts the pointer direction. Vertical's
final orientation prompt cancels on Enter. Pointer-dependent command strings
require viewport pointer context; the application supplies its last viewport ray.

```text
Plane 1,2,3 9,7,11
Plane Center 1,2,3 8 5
Plane 3Point 1,2,3 4,6,8 7,-1,3
Plane Vertical 1,2,3 5,8,3 1,2,10
Plane AroundCurve Curve=object-id 0,0,5 8 5
Plane Deformable UDegree=3 VDegree=2 UPointCount=6 VPointCount=5 1,2,3 9,7,11
```

Ordinary opposite corners and Center use the receiving viewport's construction
plane, translated through the first point. The last point's perpendicular
coordinate is projected away. The first accepted point latches that viewport's
plane. Vertical uses the edge direction and the component of the CPlane normal
perpendicular to it; a spatial first edge can therefore produce a tilted plane.
Point heights use the unbounded unit-endpoint line anchored at the second edge
point, following native getter behavior even when endpoint rounding matters.
Numeric heights remain exact dimensions.

Default output is bilinear, with two controls per direction. `Deformable` enables
editable U/V degrees and counts at the initial construction prompt. Degrees range
from 2 to 11; each control count must exceed its degree. Increasing a degree
raises a smaller count automatically. Invalid entries leave the current setting
and prompt intact. Settings are remembered within the command registry, including
interactive cancellation; each new construction starts with ordinary output.
Integer settings accept integral decimal and scientific notation. `Deformable`
is a menu action: `Deformable=Yes`, `Deformable=No` and other nonempty suffixes all
enable the mode. An empty suffix or repeated mode action is rejected.

Both output modes retain physical-length UV domains. Uniform clamped knots and
controls at their Greville stations preserve the affine parameter map. Construction
work is linear in the control-net size, with a limit of 100,000 controls.

Previews and incomplete inputs create no model objects. A successful construction
creates one unselected surface on the current layer with fresh attributes and
preserves preselection. AroundCurve accepts a single preselected guide; opening
its guide picker otherwise clears unrelated selection, including on Cancel.
One Undo removes the surface and Redo restores it. Cancel preserves geometry and
pending Redo. Source geometry,
attributes and groups remain unchanged. Document or tolerance changes invalidate
an active draft. The viewport's CPlane is unchanged.

The [basis qualification](../plane-surface-validation.json) and
[completion qualification](../plane-completion-validation.json) distinguish actual
CLI constructions from UI recovery and cancellation workflows. They compare native
NURBS degrees, counts, knots, weights, controls and a parameter grid, together with
attributes, selection and history. Constructed UI outputs also round-trip through
3DM. The geometry gate remains `1e-6`; this is not a continuous native distance
certificate or a full parity claim. Completion tests record camera rays for Top,
Front and Perspective views and rotated CPlanes. General mouse trajectories,
curved-guide and edge-frame cases, cross-restart preferences, reactive Rhino History and
comparative performance remain unqualified. Numeric dimensions use the
existing Rectangle input rules.

```sh
cargo test -p viboceros-command --test plane_surface --test plane_surface_native --test plane_completion
cargo test -p viboceros --bin viboceros app::tests::plane_
python3 -m unittest tools.rhino_oracle.test_plane_surface tools.rhino_oracle.test_plane_completion
```

Reference: [Rhino Plane](https://docs.mcneel.com/rhino/8/help/en-us/commands/plane.htm).
