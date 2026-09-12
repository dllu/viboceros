# Extraction, measurement, and intersections

[Command reference](README.md) · [Project overview](../../README.md)

## Duplicating boundaries and edges

`DupBorder` duplicates the open boundaries of selected NURBS surfaces, B-reps,
and triangle meshes. Surface borders are exact rational isocurves, including
at non-clamped domain ends; closed seams and collapsed singular sides are
omitted. B-reps preserve each exact naked edge, while mesh edges are welded by
exact location into boundary polylines. Multi-edge borders are grouped until
the document gains an exact polycurve primitive. Results use the current layer
and become selected by default; `OutputLayer=Input` uses each source layer.

`DupEdge` duplicates the exact edge nearest a model-space point, or accepts an
ordered zero-based `Edges=0,2`/`Edges=All` selector for every selected NURBS
surface, B-rep, or mesh. Standalone closed-surface seams remain selectable,
collapsed singular sides are omitted, B-rep rational edge curves stay exact,
and mesh indices follow exact-location-welded topology order. Omit the selector
for a one-pick viewport workflow. Fresh selected results default to the current
layer; `OutputLayer=Input` uses each source layer.

`DupMeshEdge` duplicates the logical edge nearest a model-space point on the
selected polygon meshes. `BreakAngle=90` is the default: incident faces below
the angle are locally one smooth region, while creases at the angle, naked
borders, and unwelded seams remain boundaries. Omit the point for a one-pick
viewport workflow. `All` instead duplicates every naked/unwelded edge, with
`Output=Polylines` joining edge-exact trails or `Output=Lines` retaining
individual segments. Fresh selected results use the current layer and sources
are deselected.

`DupMeshHoleBoundary` duplicates the closed naked loop nearest a model-space
point on selected polygon meshes, or accepts ordered zero-based
`Boundaries=0,2`/`Boundaries=All` selectors for every selected mesh. Boundaries
use exact-location-welded topology, remain closed polylines in deterministic
topology order, and fresh selected results use the current layer. Omit the
selector for a one-pick viewport workflow.

`DupFaceBorder` duplicates the exact non-seam border of the nearest selected
surface or B-rep face, or accepts ordered zero-based `Faces=0,2`/`Faces=All`
selectors. Omit the selector for a one-pick viewport workflow. Linear edge
chains become one closed polyline; curved multi-edge chains currently retain exact NURBS
segments in a group. Converting these border outputs to polycurves is pending. Holes and disconnected
borders remain separate, singular and seam trims are omitted, and fresh
selected results default to the current layer (`OutputLayer=Input` is also
supported).

## Control polygons

`ExtractControlPolygon` fits degree-one polylines through the Euclidean controls
of selected curves and creates mixed triangle/quad meshes through selected
untrimmed NURBS surface control nets. Periodic control windows align to the
active domain, closed seams remain explicit, and singular surface sides become
triangles without artificial quad diagonals. Results default to the current
layer; `OutputLayer=Input` and `TargetObject` use each source object's layer.
Polycurves produce one connected polygon through the original segment controls,
sharing duplicated junction controls without elevating segment degrees.

## Arrays, joining, exploding, and measurement

`ArrayPolar 6` picks a
center for a 360-degree array about the construction-plane normal; an optional
angle and `Rotate`/`ZOffset` settings can follow the count. `Array 3 2` picks two
corners measured in the first pick's construction plane. A third count adds
plane-normal levels and requires `ZDistance=...`; `Mode=Fill` uses layout
lengths. See [plane arrays](../plane-arrays.md) for signed lengths and limits. `Join`
extends selected line, polyline, arc, NURBS, and polycurve chains within the
document tolerance, retaining the seed's attributes and groups. Its one-pass
seeded behavior differs from the batch joining API; see [joining policies](../curve-editing.md).
`Explode` turns polylines into line segments and polycurves into exact
NURBS segments in their composite parameter intervals, frees point-cloud
members as points, duplicates polysurface faces as exact trimmed B-reps, and
splits meshes at disconnected or unwelded edges. Parts are emitted in Rhino's
reverse component order, inherit attributes, and replace their source in every
existing group in its original membership order. Preselected outputs, including
exploded point-cloud members, remain selected without selecting untouched group
peers.

**Verified mesh restriction behavior (Rhino 8.32, 2026-09-12):** 13 live
[`mesh_explode_picking` cases](../oracle.md) cover object restrictions, layer
restrictions, and overlapping groups. Rhino retains object-hidden/object-locked
and layer-locked sources but deselects them; new pieces inherit attributes and
ordered groups and remain selected. Hidden-layer-only sources are deleted.
This differs from `SplitDisjointMesh`, which keeps retained sources selected.
The native command now matches those recorded outputs exactly, including
identity retention, coordinates, face counts, object/layer modes, ordered groups,
and selection. It borrows source geometry and batches source-derived copies
before deselecting inputs and deleting ordinary sources. Two native undo/redo
cycles additionally check exact objects, groups and selection membership.
These mesh observations do not establish restricted-source behavior for curves,
point clouds, or polysurfaces, nor Rhino undo/redo or document table order.

See [Length, Area, and Volume](measurements.md) for read-only measurement
commands, supported geometry, and signed-volume behavior. The separate
[mass-properties documentation](../mass-properties.md) describes trimmed-boundary
integration, translation-stable accumulation, and numerical validation.

## Curvature

`Curvature [MarkCurvature=Yes|No] point` measures the selected curve or surface
nearest the point; omit the point for a one-pick viewport workflow. The default
is read-only. Permanent markers retain the source and selection and form one
undo step. B-rep measurements respect trims and face orientation. See
[curvature measurement](../curvature.md) for principal-curvature signs, marker
geometry, numerical limits and the separate sphere comparison tolerance.

## Intersection and trimming

`Intersect` compares every supported pair of selected curve-compatible objects,
untrimmed NURBS surfaces, and B-reps. Isolated and tangent contacts create
current-layer point objects, while finite shared intervals create exact NURBS
subcurves. Curve/B-rep contacts are clipped against exact face trim regions and
deduplicated across shared edges and vertices. Transverse planar surface pairs
produce exact, arc-length-parameterized lines clipped to both finite patches;
selection order determines their orientation as in Rhino. Coincident
nonsingular convex non-rational bilinear patches produce an exact shared edge
or closed overlap perimeter, including Rhino's distinct edge orientation and
loop-domain rules. Planar surface/B-rep and B-rep/B-rep intersections are
clipped to every exact face trim region, deduplicated at shared edges and
vertices, and joined into maximal linear components; coincident faces are
currently limited to untrimmed natural domains and one area-overlap face pair.
Curve/curve overlaps use the later curve's orientation and parameterization,
matching Rhino. Pairwise duplicates are intentionally retained when three or
more source objects meet at one location. Inputs remain in the document and are
deselected, outputs are selected, and all output creation is one undo step. A
no-hit run still clears the input selection but creates no undo record.
Non-planar and more general coincident surface/surface intersections, curved
B-rep face pairs, and coincident trimmed regions remain future extensions.

`Trim point` treats the selected curve nearest the point as the target and all
other selected curves, untrimmed NURBS surfaces, and B-reps as cutters; omit the
point in the UI to pick the interval to remove in a viewport. Only the nearest
cutting intersection on either side of the pick bounds the removed interval,
so unused intersections do not split the retained geometry.
`ApparentIntersections=Yes` is the Rhino-compatible default and projects the
curve target and all cutter geometry orthogonally along world Z unless
`ViewNormal=x,y,z` is supplied; the UI passes the active view direction (an
orthogonal approximation in the perspective viewport). Use
`ApparentIntersections=No` for actual 3D intersections. End trims and
closed-curve trims retain the source identity.
Removing a middle interval creates two new native curve objects and deletes the
source, matching Rhino; both pieces inherit its attributes and groups. Results
replace the cutter selection and the complete edit is one undo step. Mesh
cutters and trimming surfaces or B-reps as targets remain future extensions.
Circular, polyline, and composite outputs preserve their native parameterization;
closed seam intersections are retained. [Native cutting](../curve-cutting.md)
documents the rational/native parameter bridge, projected edits, and validation.

## Extracting points

`ExtractPt` duplicates curve controls, surface control nets, and every raw mesh
vertex (including unused and coincident vertices). Closed seams and periodic
NURBS control rings follow Rhino ordering. Point results default to each input
layer; `OutputLayer=Current` puts them on the active layer with fresh
attributes. `Output=PointCloud` merges the locations into one cloud in selection
order, and selects the result while deselecting the sources. Input-layer output
copies the first selected source's attributes when that source contributes
locations; otherwise it uses fresh current-layer attributes.
