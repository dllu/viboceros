# Extraction, measurement, and intersections

[Command reference](README.md) · [Project overview](../../README.md)

## Duplicating boundaries and edges

`DupBorder` duplicates the open boundaries of selected NURBS surfaces, B-reps,
and triangle meshes. Surface borders are exact rational isocurves, including
at non-clamped domain ends; closed seams and collapsed singular sides are
omitted. Connected curved borders become native polycurves with consecutive
child domains; linear borders become chord-length polylines. B-rep trims control
edge direction, while mesh edges are welded by exact location into integer-domain
boundary polylines. No artificial groups are created. Results use the current
layer and fresh attributes by default; `OutputLayer=Input` copies the source
attributes **and existing group memberships**. Only the outputs become selected.
`Faces=0,2`/`Faces=All` duplicates individual selected surface/B-rep face borders.
See [border validation and remaining limits](../borders.md).

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
chains become one closed polyline; curved multi-edge chains become exact native
polycurves. Holes and disconnected
borders remain separate, singular and seam trims are omitted, and fresh
selected results default to the current layer (`OutputLayer=Input` is also
supported). Unlike `DupBorder`, its `Input` option copies only the layer, not
the source name, color, or groups.

## Polycurve segment extraction

Select one or more polycurves or polylines and enter
`ExtractSubCrv Segments=0,2 Copy=Yes Join=No OutputLayer=Input`. Segment indices
are zero-based and apply to each selected object; `Segments=All` selects every
segment. The default removes selected segments, leaves the remainder on the
source object where possible, and puts output on the current layer.
`Copy=Yes` preserves the source. `Join=Yes` combines connected selected runs,
including runs across a closed curve's seam. Disconnected remainder runs become
separate objects. Outputs keep exact segment geometry, source attributes, and
group membership. Invalid indices reject the whole command without edits.

Rhino's [ExtractSubCrv command](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractsubcrv.htm)
also offers mouse selection of segments. The native command currently uses
explicit indices for deterministic command-line input.

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
Mesh inputs use a separate alignment and assembly path with remembered
`JoinDisjointMeshes=Yes|No`; see [mesh joining](join.md) for exact topology,
attribute, selection, and tolerance policies.
`Explode` turns polylines into line segments and polycurves into exact
NURBS segments in their composite parameter intervals, frees point-cloud
members as points, duplicates polysurface faces as exact trimmed B-reps, and
splits meshes at disconnected or unwelded edges. Parts are emitted in Rhino's
reverse component order, inherit attributes, and replace their source in every
existing group in its original membership order. Preselected outputs, including
exploded point-cloud members, remain selected without selecting untouched group
peers.

Explode's one-million-output guard pre-counts point-cloud members, polyline
segments, flattened polycurve segments, and B-rep faces before materializing
their parts, including outputs already staged from earlier sources. Mesh output
counts are checked after connectivity analysis but before copying component
geometry. A connected mesh still creates no output when the budget is exhausted.
A million-plus-one
point-cloud regression verifies rejection without changing source storage,
selection, or the existing redo entry.

**Verified mesh restriction behavior (Rhino 8.32, 2026-09-12):** 16 live
[`mesh_explode_picking` cases](../oracle.md) cover object restrictions, layer
restrictions, and overlapping groups. Rhino retains object-hidden/object-locked
and layer-locked sources but deselects them; new pieces inherit attributes and
ordered groups and remain selected. Hidden-layer-only sources are deleted.
Connected sources, including locked peers, are not decomposed and stay selected.
This differs from `SplitDisjointMesh`, which keeps retained sources selected.
The native command now matches those recorded outputs exactly, including
identity retention, coordinates, face counts, object/layer modes, ordered groups,
and selection. It borrows source geometry and batches source-derived copies
before deselecting inputs and deleting ordinary sources. Two native undo/redo
cycles additionally check exact objects, groups and selection membership.
These mesh observations do not establish restricted-source behavior for curves,
point clouds, or polysurfaces, nor document table order. One locked-connected
mixed case records actual Rhino Undo/Redo selection: the unchanged peer remains
selected even though the restored exploded source is unselected.

See [Length, Area, and Volume](measurements.md) for read-only measurement
commands, supported geometry, and signed-volume behavior. The separate
[mass-properties documentation](../mass-properties.md) describes trimmed-boundary
integration, translation-stable accumulation, and numerical validation.
Use [AreaCentroid](area-centroid.md) to create one cumulative area-weighted marker.

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
selection order determines their orientation as in Rhino. Nonplanar rational
bilinear patches with weights of one sign intersect finite planes in exact
conics, straight rulings, or isolated corner points, clipped to the plane patch.
Coincident nonsingular convex bilinear patches with weights of one sign, along
with certified affine or projective patches of any degree, produce an exact
shared edge or closed overlap perimeter, including Rhino's distinct edge
orientation and loop-domain rules. Injective planar strips with a monotone
parameter direction and separated ruled edges also return exact curved overlap
perimeters in either U or V orientation, including their original NURBS control
points and domains.
Supported surface/B-rep and B-rep/B-rep
intersections are clipped to exact face trim regions when needed, deduplicated
at shared edges and vertices, and joined into maximal components. Curved
surfaces against trimmed planar and spherical faces are included. Coincident
planar surface/B-rep and B-rep/B-rep pairs return the shared region perimeter,
including trim holes and curved cap boundaries. Multiple coincident planar
B-rep faces contribute each unique linear edge once and join the edge graph
into traversable curves.
Canonical spherical surfaces and planar surface patches intersect in exact
rational circles or circular arcs clipped to the finite patch. Exact tangency
creates a point. Rhino's surface intersection API can instead return tiny
numerical curves for this tangent case. The
[sphere/plane oracle fixture](../../tools/rhino_oracle/fixtures/sphere_plane_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_plane_surface_intersection.json)
record full, clipped, tangent, and disjoint cases.
Two canonical spherical surfaces intersect in two exact rational semicircles,
matching Rhino's two-curve result for a secant pair. External and internal
tangencies produce exact points; disjoint and contained pairs produce no result.
The [sphere/sphere oracle fixture](../../tools/rhino_oracle/fixtures/sphere_sphere_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_sphere_surface_intersection.json)
record secant, tangent, disjoint, and concentric cases. Rhino's numerical API
missed the internal tangent point in that fixture. Coincident spheres still
report an unsupported two-dimensional overlap.
Canonical coaxial spheres and finite cylindrical walls intersect in exact
rational circles, including tangent circles and sections on the cylinder rims.
Smooth noncoaxial sphere/cylinder branches are fitted as cubic curves within
the modeling tolerance and clipped at the cylinder rims. This includes the
single loop formed when the upper and lower branches meet at two radial turns.
Isolated rim and external radial contacts create points.
The [sphere/cylinder fixture](../../tools/rhino_oracle/fixtures/sphere_cylinder_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_cylinder_surface_intersection.json)
record two circles, clipping to one, rim contact, tangency, and noncoaxial
full, clipped, rim-tangent, and radial-turning reference geometry. Rhino's surface API also
reports duplicate seam points on transverse curves and a tiny numerical curve
at the isolated rim tangency. The native result retains the exact circles and
point. Rhino splits the radial-turning loop into two open halves and misses the
external radial tangent in the saved observation. The native result retains one
closed loop and the exact tangent point. The exact singular case where two
branches cross is represented by an exact rational quartic figure-eight curve;
finite cylinder rims trim it to exact subcurves or an isolated contact point.
Canonical coaxial spheres and finite cone walls intersect in exact rational
circles, including tangent circles and sections on the cone's base rim. A
contact only at the singular apex produces no event. The
[sphere/cone fixture](../../tools/rhino_oracle/fixtures/sphere_cone_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/sphere_cone_surface_intersection.json)
record these cases. Rhino's surface API missed the exact tangent circle and
reported redundant seam points alongside transverse circles in this fixture;
the native result retains the exact circles. When a noncoaxial sphere strictly
contains the cone apex, its intersection with the cone is a fitted cubic loop,
an arc clipped at the cone base, or an isolated base-rim contact. Spheres
through the cone apex produce fitted loops that may close at the apex, finite
arcs ending there, or isolated base-rim contacts. An apex-only contact is
omitted, following Rhino's surface API. Spheres outside the apex can produce
two separate fitted cubic loops; the cone base clips either loop to an arc or
isolated rim contact. At internal tangency the
two loops become one nodal cubic curve, passing twice through their contact;
the cone base can retain a closed lobe, an arc, or an isolated rim point.
When the two axial branches meet at radial turns, they produce two fitted open
curves, clipped at the base or reduced to one tangent point. This includes
sections whose opposite cone generator intersects the sphere only at negative
cone heights. The saved noncoaxial fixture covers a turning section; its two
native branch lengths agree
with Rhino within 2×10⁻⁶. Rhino also reports redundant seam points there.
An external tangency creates one point, clipped to the finite cone height.
Other noncoaxial sphere/cone sections remain unsupported.
Canonical cylinders intersect perpendicular planar patches in exact circles
and parallel patches in straight generatrices. Oblique sections are exact
rational ellipses clipped to both finite surfaces. Rhino returns cubic fitted
curves for those sections; the native rational curves follow the same geometry.
At a tangent parallel plane, the two coincident generatrices are retained to
match Rhino's result. The
[cylinder/plane fixture](../../tools/rhino_oracle/fixtures/cylinder_plane_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cylinder_plane_surface_intersection.json)
cover those cases.
Parallel canonical cylinder walls intersect in exact, height-clipped
generatrices. External tangencies retain two coincident lines and internal
tangencies retain four, matching Rhino's surface API. Rim-only transverse
contacts produce points; coaxial walls sharing one rim produce its exact circle.
Coaxial walls overlapping over an area produce no API events, as Rhino does.
The [cylinder/cylinder fixture](../../tools/rhino_oracle/fixtures/cylinder_cylinder_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cylinder_cylinder_surface_intersection.json)
record these parallel cases.
Equal-radius cylinder walls whose axes cross intersect in two exact rational
ellipses, including oblique crossings. Both finite height ranges clip them to exact conic
arcs or isolated rim points. Unequal-radius walls with crossing axes produce
two cubic curves fitted within the modeling tolerance, including oblique
crossings. Finite heights clip them to arcs and retain isolated rim contacts.
Skew-axis cylinders with disjoint finite wall bounds report no intersection.
When the larger radius exceeds the smaller radius plus the axis separation,
their walls produce two separate fitted cubic curves; finite heights clip the
curves to arcs or isolated rim contacts. When the axis separation lies between
the radius difference and the radius sum, the walls produce one connected
fitted cubic loop, likewise clipped to finite heights. External tangency
produces an isolated point. At internal tangency, the fitted cubic curve makes
two turns through one crossing point; finite heights can leave arcs or isolated
rim contacts.
Coaxial canonical cone and cylinder walls meet in one exact rational circle
when the cylinder radius occurs within both finite height ranges. Opposed
surface axes yield two exact semicircles, matching Rhino's event structure.
The [cone/cylinder fixture](../../tools/rhino_oracle/fixtures/cone_cylinder_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cone_cylinder_surface_intersection.json)
cover aligned and opposed axes, rim contacts, clipping, and a noncoaxial pair.
Rhino also reports two redundant seam points with the full circle; the native
result retains the circle. With parallel offset axes, smooth intersections are
fitted as cubic curves within the modeling tolerance and clipped to both finite
surfaces; isolated rim contacts create points. A cylinder wall through the cone
apex produces a cubic curve with an exact corner there. Full sections can be
closed; finite rim cuts produce arcs and an apex-only contact produces no event.
When a perpendicular cylinder axis passes through the cone apex, the walls
intersect in a fitted cubic loop. Both finite height ranges clip it to arcs or
isolated rim contacts; a cone base tangent to the loop creates two points.
Other nonparallel cone/cylinder axes remain unsupported.

Coaxial canonical cone walls meet in an exact rational circle when their
linear radius profiles cross inside both finite height ranges. This includes
opposed axes and a circle shared by both base rims. An apex-only contact is
omitted; coincident wall regions remain unsupported. Parallel offset cones
with equal slopes (within modeling tolerance) use a conic plane
section, clipped to both finite height ranges. When their bases meet in only
one axial plane, isolated contacts are returned as points. Parallel offset cones
with unequal slopes produce two cubic branches fitted within the modeling
tolerance, clipped at both cone bases; a single tangent contact produces a
point. Cones with different axis directions and a shared apex intersect in
zero, one, or two exact finite generators. Other nonparallel cone pairs remain
unsupported.

Canonical ring tori intersect planes perpendicular to their axes in two exact
rational circles, or one tangent circle at the tube's top or bottom. Planes
containing the axis cut two exact tube circles. Finite planar patches clip
these circles to arcs or points. Axis-parallel offset planes produce one or two
cubic loops fitted to the modeling tolerance and clipped to the finite patch;
the inner tangent plane produces two pinched loops meeting at one point.
Nearby offsets retain their distinct one-loop or two-loop topology. Oblique
planes produce one or two cubic loops fitted to the modeling tolerance, with
finite patches clipping them to arcs. Critical oblique cuts retain crossing
loops, while isolated oblique tangencies return points. Shallow tilted cuts
near an axis-containing plane retain stable full meridian loops, including
finite-patch clipping. The
[torus/plane oracle fixture](../../tools/rhino_oracle/fixtures/torus_plane_surface_intersection.json)
covers perpendicular, axis, oblique, finite-patch, and disjoint cuts. Coaxial torus and finite
cylinder walls meet in one or two exact rational circles, including tangent
sections and circles on cylinder rims. Parallel offset cylinder axes produce
fitted cubic loops, finite rim-clipped arcs, or isolated contact points.
Critical sections retain their single, pinched, and crossing loops. Cylinders
whose perpendicular axes cross the torus center and whose radii fit inside
the torus's inner rim produce four cubic loops, finite rim arcs, or isolated
rim contacts. The tube-radius case retains crossing curves. At the inner rim
radius, two crossing loops with a doubled period join the four-loop and two-loop
regimes. Larger centered perpendicular cylinders produce two turning loops
and finite arcs up to the outer tangent radius, which gives two contact points.
The same centered-axis construction also handles thick-tube ring tori when the
cylinder radius lies between the inner rim and tube radius, with separate outer
and inner loops and finite rim clipping. At the inner rim radius, the inner
curves cross. At the tube radius, two turning loops cross. Other nonparallel
torus/cylinder axes remain unsupported. The
[torus/cylinder oracle fixture](../../tools/rhino_oracle/fixtures/torus_cylinder_surface_intersection.json)
covers exact circles, offset loops, finite arcs, crossings, and contacts.
Spheres centered on a torus axis intersect it in one or two exact rational
circles, including tangent circles. Offset spheres produce fitted cubic loops
or isolated tangent points. The
[torus/sphere oracle fixture](../../tools/rhino_oracle/fixtures/torus_sphere_surface_intersection.json)
covers axial, offset, tangent, contained-meridian, and disjoint cases.
Coaxial tori likewise intersect in one or two exact rational circles, including
tangent circles. Equal tori with parallel offset axes intersect along a
symmetry-plane section and an elliptic section. Tori with equal tube radii,
unequal major radii, and parallel offset axes at the same axial level intersect
along lifted hyperbolic and elliptic sections. These produce fitted cubic loops,
pinched loops, exact meridian circles at conic collapse, or isolated contacts.
Equal tori with a common center and different axis directions intersect in two
planar sections, also returned as fitted cubic loops. Coincident tori, unequal
tube radii with offset axes, axial offsets, and other intersecting nonparallel
pairs remain unsupported. Separated surface control hulls return no intersection
events. The
[torus/torus oracle fixture](../../tools/rhino_oracle/fixtures/torus_torus_surface_intersection.json)
covers coaxial circles, parallel offsets, tangency, and disjoint cases.
Coaxial tori and finite cone walls intersect in up to two exact rational
circles, clipped to the cone height. Tangent circles and cone rims are included;
offset and nonparallel torus/cone axes remain unsupported.
The [torus/cone oracle fixture](../../tools/rhino_oracle/fixtures/torus_cone_surface_intersection.json)
covers finite coaxial circles and disjoint cases.

Canonical cones intersect planar patches in exact circles, rational elliptical,
parabolic, and hyperbolic arcs, or straight generators. A plane touching only the
singular apex produces no result, matching Rhino. The two coincident tangent
generators are retained. Rhino fits some conics as cubics, so its curve lengths
can differ slightly from the exact rational sections. The base rim tangent
is returned as one exact point; Rhino's numerical intersection
returns two tiny curves there. The
[cone/plane fixture](../../tools/rhino_oracle/fixtures/cone_plane_surface_intersection.json)
and [observations](../../tools/rhino_oracle/observations/cone_plane_surface_intersection.json)
record the reference cases. Finite arcs are retained when the full projective
conic has a pole outside the cone. Sections without two distinct base-rim
crossings can still be unsupported.

Curve/curve overlaps use the later curve's orientation and parameterization,
matching Rhino. Pairwise duplicates are intentionally retained when three or
more source objects meet at one location. Inputs remain in the document and are
deselected, outputs are selected, and all output creation is one undo step. A
no-hit run still clears the input selection but creates no undo record.
Other non-planar and more general coincident surface/surface intersections,
curved B-rep face pairs, and multiple coincident curved face regions remain
future extensions.

`IntersectTwoSets first-id[,id...] second-id[,id...]` evaluates only pairs
across the two sets. Either set may be `Selected` to use the current selection.
`OutputLayer=Current|FirstSet|SecondSet` places each result on the current
layer or the corresponding member's layer. The command shares `Intersect`'s
geometry calculations, output limits, selection, and undo behavior.
Enter bare `IntersectTwoSets` in the app to pick the first set, press Enter,
pick the second set, then press Enter again. An existing selection supplies
the first set. `OutputLayer=...` may be entered before or during either phase.

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
