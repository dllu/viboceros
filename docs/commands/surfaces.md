# Surfaces and solids

[Command reference](README.md) · [Project overview](../../README.md)

## Planar surfaces

`PlanarSrf` creates exact single-face B-reps from selected closed planar curves.
Wholly contained curves become inner trim loops, alternating nested regions
become independent islands, and partially overlapping curves remain separate
surfaces. Every original rational curve remains an exact boundary edge and
parameter-space trim, so curved, concave, and holed regions shade and export
without filling excluded areas. Inputs are retained by default;
`DeleteInput=Yes` removes each successfully used boundary.

## Primitives and bounding boxes

`Sphere` creates Rhino/OpenNURBS' exact 9-by-5 rational quadratic surface from
a center and numeric radius or point on the sphere. Its longitude domain is
`[0, 2π]`, its latitude domain is `[-π/2, π/2]`, and entering it without
arguments starts the two-pick viewport workflow.

`Ellipsoid` creates the exact closed 9-by-5 rational surface obtained by
scaling that sphere along three orthogonal semi-axes. Supply three numeric
radii in World XY, or center and three axis-radius points for an oriented
ellipsoid; entering it without arguments starts the four-pick viewport workflow.

`Box` follows Rhino's default two-opposite-base-corners and height workflow in
World XY. The height may be a signed number or a point, and the result is one
closed B-rep with eight shared vertices, twelve shared edges, six outward
bilinear faces, and exact rational parameter-space trims.

`BoundingBox` creates one cumulative World-coordinate enclosure of the selected
objects by default; `Cumulative=No` creates one per object. `Output=` supports
exact B-rep solids, shared-vertex triangle meshes, six grouped rectangle curves,
or report-only `None`. World-plane selections produce a rectangle or mesh plane.
`CoordinateSystem=CPlane` is accepted and currently shares the World XY basis;
analytic bounds are tight, while NURBS bounds use their control geometry and
can be non-conservative for negative-weight projective inputs.

## Face and isocurve extraction

`ExtractSrf` separates the nearest exact face at a model-space point, or accepts
an ordered zero-based list such as `Faces=0,2` (`Faces=All` is also supported).
The face list applies to every selected NURBS surface or B-rep; entering the
command without a selector starts a one-pick viewport workflow. `Copy=No` is
the default: an unextracted B-rep remainder keeps the source identity,
attributes, and groups, while a fully extracted source is deleted. Extracted
faces preserve attributes, become selected independent objects, and never
inherit source groups. `Copy=Yes` retains the source. Output defaults to the
input layer; `OutputLayer=Current` changes only the result layer.

`ExtractIsocurve` creates the exact U, V, or both rational isocurves nearest a
model-space point on every selected NURBS surface or B-rep. B-rep results come
from the nearest trimmed face and are split exactly around outer boundaries and
holes; rational p-curve intersections determine the retained parameter
intervals without faceting the output. Extracted curves preserve the varying
direction's degree and parameter values even at non-clamped spans. Omit the
point to pick a surface location in the viewport. `IgnoreTrims=Yes` uses the
full underlying B-rep face instead. `ExtractAll` emits the natural boundaries,
knot wires, and density-dependent wires inside each knot span from all selected
surfaces or B-rep faces; the per-object Rhino wire density survives 3DM I/O.
The same wire-density rules drive viewport display and `ExtractWireframe`.
That command emits each B-rep or exact-location-welded mesh topology edge once,
adds exact trim-clipped interior surface isocurves, selects the results, and can
place them on the current or input layer. `GroupOutput=Yes` forms one output
group; `OutputLayer=TargetObject` currently uses the selected target's layer.

## Cylinders and cones

`Cylinder` creates an exact 9-by-2 rational NURBS wall from a center, radius
(or base-circle point), and signed height. `Axis=` accepts an arbitrary axis,
while `BothSides=Yes` makes the height symmetric about the base center.
`Solid=Yes` adds exact rational polar-disk caps and shared rim/seam topology as
a closed B-rep; `Solid=No` retains the single open wall surface.

`Cone` uses the same center, numeric radius or base-point radius, signed height,
and `Axis=` conventions. It produces Rhino/OpenNURBS' exact 9-by-2 rational
wall with a weighted collapsed apex. `Solid=Yes` adds an exact polar-disk base
and shared rim/seam topology as a closed B-rep; `Solid=No` leaves the wall open.

## Solid primitives, extrusion, and revolution

`Paraboloid` supports Rhino's `Focus focus direction-point end-point` and
`Vertex vertex focus end-point` constructions; `Focus` is the default. The
vertex form projects the end point perpendicular to the focus axis and derives
the height from that radius. Both produce Rhino's exact 9-by-3 rational
quadratic wall with an arc-length meridian domain and singular/shared seam
topology. `Solid=Yes` adds the exact planar rim cap, while `MarkFocus=Yes` adds
a point object at the focus in the same undo step.

`TruncatedCone` creates an exact rational frustum from base and end radii. Its
linear V domain is the physical slant length, matching Rhino; negative heights
reverse the construction direction while preserving the base seam. `Solid=Yes`
adds outward planar caps with Rhino-compatible shared rim and seam topology.

`Pyramid` and `TruncatedPyramid` create regular polygonal joined B-reps with
Rhino-compatible vertex, edge, face, trim, and planar-surface parameterization.
They accept a side count, numeric or picked base radius, signed height,
arbitrary `Axis=`, and optional exact planar caps through `Solid=Yes`.

`Tube` creates a closed exact B-rep with concentric rational cylinder walls and
annular planar caps. The two radii may be entered in either order;
`WallThickness=` expands outward from the first radius, and `BothSides=Yes`
centers the full doubled height on the starting point.

`Torus` creates an exact closed 9-by-9 rational quadratic surface from a center,
major radius (or point on the major circle), and minor radius. `Axis=` orients
the torus; the major radius must exceed the positive minor radius. Its U and V
domains are the major- and minor-circle circumferences, matching OpenNURBS.

`ExtrudeCrv` creates exact rational NURBS surfaces from selected analytic,
polyline, and NURBS curves. A numeric distance uses World Z; two points define
an arbitrary direction; entering it without either starts the two-pick viewport
workflow. `BothSides=Yes` sweeps equally in both directions and
`DeleteInput=Yes` removes the profiles. Matching Rhino, outputs are unselected,
ungrouped objects with fresh attributes on the current layer. `Solid=Yes`
turns each closed planar profile into an exact closed B-rep with a ruled wall,
two planar trimmed caps, shared rational rim curves, and one shared wall seam;
open and nonplanar profiles remain exact NURBS surfaces.
Profile parameters are retained, including native polyline vertex intervals;
extrusion does not apply `ToNURBS` chord-length reparameterization. The
[native extrusion oracle](../curve-cutting.md) verifies the single-surface policy.

`ExtrudeCrvToPoint` creates exact rational NURBS surfaces that taper selected
curves to one apex. Its U direction runs from the profile to the apex while V
preserves the source curve's degree, knots, and weights, matching Rhino's NURBS
form. Enter it without an apex to pick one in the viewport. Inputs are retained
by default; `DeleteInput=Yes` removes them. `Solid=Yes` turns each closed planar
profile into an exact closed B-rep with a singular-apex ruled wall, planar
trimmed cap, shared rational rim, and one twice-used wall seam; open and
nonplanar profiles remain surfaces. `Output=Surface` and `Solid=No` are accepted
explicitly; SubD output is not yet represented.

`ExtrudeCrvAlongCrv` creates an exact fixed-orientation sum surface from each
selected profile and a curve path. Name the path with `PathName=`, or select it
last; the toolbar uses the last-selected convention. The path is retained and
deselected. `DeleteInput=Yes` removes only successfully extruded profiles.
Profile degree, knots, and weights become U; path data becomes V; tensor weights
are multiplied exactly. With an open path whose endpoints leave the profile
plane, `Solid=Yes` turns each closed planar profile into an exact capped B-rep
with translated planar trims, shared rational rims, and an exact copy of the
path as the twice-used wall seam. Open and nonplanar profiles, or profiles swept
along a closed path, remain surfaces. `Output=Surface` and `Solid=No` are
accepted explicitly.

`Revolve` creates exact rational NURBS surfaces around an arbitrary two-point
axis. Supply a signed sweep from -360 through 360 degrees, or use
`FullCircle=Yes`; `StartAngle=` rotates the beginning of a partial sweep.
Entering `Revolve` without an axis starts the two-pick viewport workflow, which
defaults to a full turn and also accepts `Angle=`, `StartAngle=`, and
`DeleteInput=`. The exact surface keeps the profile in V and uses fully
multiple quadratic quadrant knots in U. `Output=Surface` and `Deformable=No`
are supported; deformable and SubD revolves are not yet represented.

## Surface extension

`ExtendSrf Edge=West|South|East|North Distance=value` performs Rhino's
physical-distance operation on one selected untrimmed NURBS surface. Positive
distances use Rhino's homogeneous-control-curve RMS scaling, including
rational weights and non-clamped ends. Negative distances trim inward by true
arc length along an on-surface isocurve, use the selected edge's parameter
midpoint by default, and restore the original surface domain; `At=value`
chooses the edge parameter explicitly. A model point in place of `Edge=` picks
the closest non-singular open natural boundary and supplies that path parameter
for shrinking. In the UI, enter `ExtendSrf Distance=value` with any type and
merge options to pick this boundary in a viewport. `Direction=U|V
Domain=start,end` exposes the corresponding analytic-domain extension.
`Type=Smooth` analytically extrapolates the edge span, while `Type=Line` joins
an exact degree-matched straight tangent span. `Merge=No` retains the source
and creates the extension as a separate patch with matching attributes and
group membership. Tensor structure, identity, attributes, groups, selection,
and undo are preserved.
