# Surfaces and solids

Eligible planar creators follow [UseExtrusions](use-extrusions.md). The typed
outputs below describe its default `Extrusion` mode.

[Command reference](README.md) · [Project overview](../../README.md)

[MergeSrf](merge-srf.md) joins two untrimmed NURBS surfaces with automatic
boundary orientation, smoothing options and undo/redo.

## Planar surfaces

`PlanarSrf` creates exact trimmed planar B-reps from selected closed curves.
Nested, overlapping, touching and repeated boundaries follow the shared region
planner in both builds. See [region grouping, source deletion and qualification](planar-srf.md).

## Primitives and bounding boxes

`Cap` fills planar openings on existing surfaces and B-reps in place, including
nested annular caps. See [capping behavior, evidence, and limits](cap.md).

`Sphere` creates Rhino/OpenNURBS' exact 9-by-5 rational quadratic surface from
a center and numeric radius or point on the sphere. Its longitude domain is
`[0, 2π]`, its latitude domain is `[-π/2, π/2]`, and entering it without
arguments starts the two-pick viewport workflow.

`Sphere FitPoints` fits a sphere through selected point objects or grips.
See [point fitting and its retained Rhino difference](sphere-fit-points.md).

`Ellipsoid` creates the exact closed 9-by-5 rational surface obtained by
scaling that sphere along three orthogonal semi-axes. Supply three numeric
radii in World XY, or center and three axis-radius points for an oriented
ellipsoid; entering it without arguments starts the four-pick viewport workflow.

`Box` follows Rhino's two-opposite-base-corners and signed-height workflow in
the active construction plane. It creates one closed typed extrusion with an
exact shared-topology B-rep view; see [typed creators](../creator-extrusions.md).
The explicit B-rep constructor remains available for SDK-style box topology.
Entering `Box` without arguments starts a three-point interactive workflow;
see [construction planes](../construction-planes.md) for projection and height rules.

`BoundingBox` creates tight World/CPlane enclosures of selected objects, using
analytic/rational extrema and retained trimmed-face interiors. Outputs include
B-rep solids, quad meshes, grouped rectangle curves, and report-only results;
planar enclosures produce rectangles. See [bounding boxes](bounding-box.md) for
individual/cumulative syntax, coordinate reports, failure policy, and retained
Rhino differences.

## Edge surfaces

`SrfPt` creates three- or four-corner surfaces with physical-length UV charts.
Enter after three picks makes a trimmed triangular face. See
[corner construction, repeated points and qualification](srf-pt.md).

`EdgeSrf` creates a ruled, triangular, or four-sided Coons face from two, three,
or four selected open curves. Inputs remain unchanged and selected; the result
is an unselected B-rep on the current layer. See [edge-surface construction and
limits](../edge-surfaces.md), including how gaps can displace boundary geometry
and how homogeneous controls at infinity are handled.

## Point-grid surfaces

`SrfPtGrid` and `SrfControlPtGrid` build a B-rep from a complete rectangular
point grid. `KeepPoints=Yes` additionally retains one ordered point cloud.
See [point-grid construction](../point-grid-surfaces.md) for degree/count syntax,
closed-direction compatibility, crease topology, and numeric limits.

## Lofting

`Loft Type=Normal Closed=No` creates one shared-topology B-rep from selected
curves. Normal, Loose, Tight, Straight and Uniform styles are supported, as are
periodic closed lofts. Inputs are retained; output is unselected on the current
layer. `SplitAtTangents=Yes` adds curvature-break partitions independently of the
global `CreaseSplitting` preference; `Natural` retains supplied profile seams.
`StartPoint=x,y,z` and `EndPoint=x,y,z` add [point poles](../loft-points.md),
including a single closed profile with two poles.
`Rebuild=6` adds [section rebuilding](../loft-rebuild.md).
Bare `Loft` opens ordered curve selection; its `Point` option picks either pole
with snapping. Enter accepts, `Undo` revises input, and Escape cancels.
See [Loft geometry, ordering, tests and limits](../loft.md), especially
the endpoint-weight normalization policy for rational profiles.

## One-rail sweeps

`Sweep1` places profiles automatically: select the rail first, then profiles in
traversal order, or specify `RailName=name` or `RailId=id` and select only profiles.
Starting the command before selection opens the curve picker. `Point` picks a
[start or end pole](../sweep1/point-ends.md) with snapping. See
[automatic placement](../sweep1-placement.md) for interval and direction choices.

Explicit scripts such as `Sweep1 Parameters=0 RailName=Rail` retain their previous
policy: without a named rail or ID, the last selected curve is the rail. Supply
one native rail parameter per profile. `RefitRail=No` (default) uses the shared
rail-basis policy, including the implicit refits described above. `RefitRail=Yes`
refits the rail to cubic arc-length form before interpolating transported profiles.
Both paths permit multiple profiles, with compatibility limits described below.
Retained-basis construction uses Local blending in rational rail parameters,
blends Euclidean control positions and raw relative weights, then normalizes
placed profiles' end weights. Refitted construction instead uses homogeneous
arc-length blending. The retained path shares Loft's common-knot policy, which
can change later multi-span profiles when endpoint weight ratios differ.
`FrameStyle=Freeform|Roadlike`, `Axis=x,y,z` (Roadlike only), and
`GlobalShapeBlending=Yes|No` select the frame and blend model. Inputs and selection
are retained; one unselected B-rep is added on the current layer with atomic undo.
See [sweep construction and compatibility gaps](../sweep1.md): smooth closed
rails can close when their generated seam matches, including a full circular
rail with one profile. General closed-rail holonomy correction, miters,
general branch/seam choices, and complete Rhino multi-section blending remain
unimplemented. Automatic placement follows the qualified interval and direction
policy linked above.

## Face and isocurve extraction

`ExtractSrf` separates the nearest exact face at a model-space point, or accepts
an ordered zero-based list such as `Faces=0,2` (`Faces=All` is also supported).
The face list applies to every selected NURBS surface or B-rep; entering the
command without a selector starts a viewport workflow that accumulates visible
faces until Enter. Ctrl/Command removes picks, and rectangles can select several
faces across objects. Scripts can target one selected source with
`Face=0 Object=<uuid>`. `Copy=No` is
the default: an unextracted B-rep remainder keeps the source identity,
attributes, and groups, while a fully extracted source is deleted. Extracted
faces preserve attributes, become selected independent objects, and never
inherit source groups. `Copy=Yes` retains the source. Output defaults to the
input layer; `OutputLayer=Current` changes only the result layer.
See [face selection, staging, native order, and verification](extract-srf.md).
[SubD face regions](extract-srf-subd.md) use separate connectivity and group rules.

`ExtractIsocurve` creates the exact U, V, or both rational isocurves nearest a
model-space point on every selected NURBS surface or B-rep. B-rep results come
from the nearest trimmed face and are split exactly around outer boundaries and
holes; rational p-curve intersections determine the retained parameter
intervals without faceting the output. Extracted curves preserve the varying
direction's degree, including non-clamped spans. Trim-aware B-rep extraction
uses a [lossless local UV frame](../brep-isocurves.md); native output parameters
are restored only when every knot can be translated exactly. Otherwise the
curve retains a local domain to avoid rounding its geometry. Omit the
point to pick the visible surface or B-rep face in the viewport; the pick uses
the surface hit location even when the construction plane is elsewhere. Scripts
can target one selected face with `point Face=0 Object=<uuid>`.
`IgnoreTrims=Yes` uses the full underlying B-rep face instead. `ExtractAll`
emits the natural boundaries,
knot wires, and density-dependent wires inside each knot span from all selected
surfaces or B-rep faces; the per-object Rhino wire density survives 3DM I/O.
The same wire-density rules drive viewport display and `ExtractWireframe`.
Trim-aware `ExtractAll` generates its stations locally as well. B-rep display
isocurves retain local domains for accurate sampling; source 3D edge domains
are unchanged. [Standalone surfaces and `IgnoreTrims=Yes`](../surface-wire-frames.md)
also generate `ExtractAll` stations and output curve domains locally. Newly
constructed surface B-reps retain native UV data but create local-domain 3D edges.
That command emits each B-rep or exact-location-welded mesh topology edge once,
adds exact trim-clipped interior surface isocurves, selects the results, and can
place them on the current or input layer. `GroupOutput=Yes` forms one output
group; `OutputLayer=TargetObject` currently uses the selected target's layer.

## Cylinders and cones

`Cylinder` creates a typed circular extrusion from a center, radius (or
base-circle point), and signed height. `Axis=` accepts an arbitrary axis;
`BothSides=Yes` sweeps in both signed directions from the base center.
`Solid=Yes` is the default, with exact trimmed planar caps; `Solid=No` leaves
an open wall. The profile interval is the circumference, and the path interval
runs from zero to its length. See [typed creators](../creator-extrusions.md).

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

`Tube` creates a closed typed extrusion with concentric rational cylinder walls and
annular planar caps. The two radii may be entered in either order;
`WallThickness=` expands outward from the first radius, and `BothSides=Yes`
centers the full doubled height on the starting point.

`Torus` creates an exact closed 9-by-9 rational quadratic surface from a center,
major radius (or point on the major circle), and minor radius. `Axis=` orients
the torus; the major radius must exceed the positive minor radius. Its U and V
domains are the major- and minor-circle circumferences, matching OpenNURBS.

`ExtrudeCrv` sweeps selected curves by a distance or two direction points.
It supports exact capped solids, typed extrusion storage, independent tangent
and crease splitting, and fitted SubD output. See [straight extrusion options,
interactive direction picking and qualification](extrude-crv.md).

`ExtrudeCrvToPoint` tapers selected curves to an apex. It supports exact NURBS
sheets, capped planar profiles, independent tangent/crease splitting, and fitted
SubDs. Start without an apex to pick it in the viewport. Inputs are retained
unless `DeleteInput=Yes`. See [surface splitting and caps](extrude-crv-to-point.md)
and [SubD fitting and output switching](extrude-crv-to-point-subd.md).

`ExtrudeCrvAlongCrv` creates exact fixed-orientation sum surfaces from selected
profiles and a path named with `PathName=`, or selected last. `SplitAtTangents`
and `CreaseSplitting` now create shared edges in both directions. `Solid=Yes`
caps eligible planar boundary loops; `DeleteInput` applies only to profiles.
See [curve-path extrusion](../smlib-translational-sweep.md) for defaults,
source policy, native comparisons, and remaining limitations.

`Revolve` creates exact rational surfaces, cubic deformable surfaces or SubDs
around a two-point axis. Its [command guide](revolve.md) covers signed sweeps, profile preparation,
parameter domains, point-count options and interactive axis picking.

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
merge options to pick this boundary on the rendered surface in a viewport;
the hit is independent of the construction plane. `Direction=U|V
Domain=start,end` exposes the corresponding analytic-domain extension.
`Type=Smooth` analytically extrapolates the edge span, while `Type=Line` joins
an exact degree-matched straight tangent span. `Merge=No` retains the source
and creates the extension as a separate patch with matching attributes and
group membership. Tensor structure, identity, attributes, groups, selection,
and undo are preserved.

## Shrink underlying trimmed surfaces

`ShrinkTrimmedSrf` and `ShrinkTrimmedSrfToEdge` crop underlying NURBS domains
while preserving visible trims and topology. Both support whole surfaces and
polysurfaces; see [behavior, verification, and limits](shrink-trimmed-surfaces.md).
