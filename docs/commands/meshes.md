# Polygon meshes

[Command reference](README.md) · [Project overview](../../README.md)

Kernel ownership, numerical limits, allocation policy, and regression evidence
are documented in [mesh topology and editing](../mesh-topology.md).

## Meshing and mesh primitives

`Mesh` creates editable polygon meshes from selected NURBS surfaces and B-reps
while retaining the selected sources. The new meshes copy source names, layers,
colors, and display attributes, remain unselected, and do not inherit source
groups, matching Rhino's derived-object behavior. `Density=0..1` selects a
bounded per-knot-span sampling level; `SimplePlanes=Yes` minimizes entirely
planar inputs, and `JaggedSeams=Yes` disables shared-edge snapping on B-reps.
Regular surface cells remain quadrilaterals, singular sides and planar trim
regions use triangles. [Smooth-seam boundary auditing](../brep-meshing.md) checks
open shells as well as closed solids; incompatible shared grids trigger a
conforming triangle fallback without capping intended openings.
General nonplanar trims are constrained-triangulated in parameter space with
interior knot-span grid samples; outer and inner boundaries remain exact mesh
constraints instead of being silently filled.

`MeshBox` draws an unselected closed quadrilateral mesh from two opposite
World-XY base corners and a signed height or height point. `XCount`, `YCount`,
and `ZCount` set the side divisions and default to 1. Each of Rhino's bottom,
top, front, right, back, and left grids retains independent raw vertices while
exact-location topology forms one outward-oriented solid; one box is bounded
to one million faces and invalid extents fail atomically.

`MeshCone` draws a polygonal cone from a base center, numeric or picked radius,
and signed apex height. `VerticalFaces` and `AroundFaces` default to 10,
`Solid=Yes` adds a topology-joined base cap, and `CapFaceStyle=Tri|Quad` selects
triangle fans or Rhino's even-sided quad fan; odd side counts fall back to
triangles. `Axis=x,y,z` orients typed commands, the three-pick toolbar workflow
uses World Z, and one cone is bounded to one million faces.

`MeshTruncatedCone` draws a polygonal frustum from a base center, numeric or
picked base radius, signed height, and positive end radius. Radii interpolate
linearly through the height-major wall rings. `VerticalFaces` and `AroundFaces`
default to 10; `Solid=Yes` adds independent, topology-joined end caps, and
`CapFaceStyle=Tri|Quad` follows Rhino's even-sided quad fans and odd-count
triangle fallback. `Axis=x,y,z` orients typed commands, including the winding
direction for negative heights. The four-pick toolbar workflow uses World Z,
and one truncated cone is bounded to one million faces.

`MeshCylinder` draws a polygonal cylinder from a center, numeric or picked
radius, and signed height. `VerticalFaces` and `AroundFaces` default to 10,
`Solid=Yes` adds independently stored but topology-joined caps, and
`CapFaceStyle=Tri|Quad` chooses triangle fans or Rhino's even-sided quad fan.
Odd side counts always fall back to triangles. `BothSides=Yes` mirrors the
height about the base plane, `Axis=x,y,z` orients typed commands, and the
three-pick toolbar workflow uses World Z. Output is consistently oriented and
bounded to one million faces.

`MeshPlane` draws an unselected quadrilateral grid from two opposite top-view
corners. `XCount` and `YCount` specify face counts and default to 10; corner
order is normalized, the second corner is projected to the first corner's
elevation, and vertices/faces follow Rhino's x-fastest row-major ordering.
One grid is bounded to one million faces and tolerance-degenerate cells are
rejected atomically.

`MeshSphere` creates all three Rhino polygon-sphere styles from a center and
numeric or picked equator radius. `Style=UV` uses a latitude-major quad grid
with shared triangle-fan poles; `VerticalFaces` and `AroundFaces` default to
10. `Style=Quads` applies Catmull-Clark cube refinement before radial
projection, while `Style=Triangles` recursively refines an icosahedron; both
use `Subdivisions=3` by default and preserve Rhino's welded indexing. The
command limits subdivisions to 6 quads or 5 triangles, `Axis=x,y,z` orients
typed commands, and the two-pick toolbar workflow uses World Z. One sphere is
bounded to one million faces.

`MeshEllipsoid` creates a closed mesh from three positive semi-axis radii or
from a center and three axis-radius points. `VerticalFaces` divides the first
axis from pole to pole, `AroundFaces` divides each rationally parameterized
ring, and both default to 10. `CapFaceStyle=Tri|Quad` selects triangle fans or
Rhino's paired quad pole faces; odd around counts fall back to triangles.
Vertices and faces preserve Rhino's NURBS-parameter sampling and welded index
order. The four-pick toolbar workflow retains these options, and one ellipsoid
is bounded to one million faces.

`MeshTorus` creates a closed quadrilateral ring torus from a center, numeric or
picked major radius, and a positive minor radius smaller than the major radius.
`VerticalFaces` divides the tube circle, `AroundFaces` divides the major circle,
and both default to 10. Vertices and seam-wrapped faces retain Rhino's periodic
row-major ordering. `Axis=x,y,z` orients typed commands; the three-pick toolbar
workflow uses World Z and measures the tube radius from the major-radius point.
One torus is bounded to one million faces and invalid radii fail atomically.

[`MeshToNURB`](mesh-to-nurb.md) converts selected triangle/quad meshes to exact
NURBS faces, splitting edge-disconnected components into separate unselected
B-reps. Outputs retain source attributes but not group memberships. Other
selected geometry is ignored, and triangle-trimming choices survive undo.
See its reference for selection paths, option memory, budgets, and n-gon limits.

## Mesh extraction and editing

`OffsetMesh 2` copies selected meshes by moving each topological vertex along
the average of its raw vertex normals. Coincident unwelded copies move together.
`DirectionMethod=UserSelectedDirection` with
`Direction=0,0,1` uses one specified vector instead. `AverageNormals=Yes`
moves all vertices in their common average direction, falling back to the
active construction plane normal when that average cancels. `FlipAll=Yes`
reverses the side. `BothSides=Yes` creates two offset skins; `Solid=Yes` also
includes the original skin for a one-sided offset and joins naked boundaries
with quadrilateral walls. `AllowDisjoint=No` creates separate objects for
disconnected results, while `AllowDisjoint=Yes` retains one mesh per source.
`DeleteInput=Yes` removes the originals. Output keeps source attributes and
groups, and the edit is undoable. Offsets that collapse faces or fail to form
a closed shell in solid mode are rejected. Exact Rhino offset directions and
mesh storage order still need live oracle comparison.

`AddNgonsToMesh PlanarTolerance=0.01` adds logical n-gon overlays to connected
coplanar mesh faces that share welded, oppositely oriented raw edges. The
default planarity distance is the document's absolute tolerance. A group uses
its first face's plane; groups with holes, disconnected boundaries, or fewer
than two faces are skipped. Existing n-gons remain. `DeleteMeshNgons` removes
all n-gon overlays from selected meshes. Both commands keep stored faces,
vertices, colors, object identity, attributes, groups, and selection, with
undo support. Interactive n-gon conversion policy still needs Rhino oracle
comparison.

`ExtractMeshEdges` creates fresh current-layer curves from selected polygon
meshes. `ExtractBy=Unwelded` (the default) includes both naked edges and seams
whose coincident endpoints use distinct raw mesh vertices; `ExtractBy=Naked`
keeps only one-face edges. `ExtractBy=BreakAngle` accepts strict
`GreaterThanAngle=`/`LessThanAngle=` bounds in degrees. `JoinResults=Yes`
combines each filtered edge network into deterministic edge-exact polylines,
including branched Euler trails; the default emits individual lines. Sources
and results remain selected, matching Rhino's extraction workflow.

`ExtractMeshFaces` separates an ordered zero-based face list from every
selected mesh (`Faces=All` is supported), or omits the selector for a one-pick
viewport workflow. The unselected remainder keeps its source identity; the
selected result inherits attributes and group membership. Extracting every
face reuses the source identity. `MakeCopy=Yes` instead leaves each source
unchanged. Both parts compact unused vertices in Rhino source order.

`DeleteFaces` removes an ordered zero-based face list from every selected mesh
or B-rep (`Faces=All` is supported), or omits the selector for a one-pick
viewport workflow. A partial edit keeps the unselected source object's
identity, attributes, groups, and surviving source face order. Mesh results
compact unused vertices in source order; deleting every face removes the
object. SubD input awaits a native SubD geometry type.
Mesh extraction and deletion preserve n-gon overlays in any result containing
every member face of the n-gon. A partial face group has no retained overlay.

`ExtractMeshFacesByArea SmallerThan=2` extracts stored triangle and quad faces
whose unsigned areas are strictly below the limit. `LargerThan=area` can be
used alone or together with `SmallerThan=area`; `MinArea` and `MaxArea` are
accepted aliases. `MakeCopy=Yes` retains the input faces. `BorderOnly=Yes`
creates boundary polylines and leaves the input mesh intact. The command
stages all selected meshes before editing, preserves output attributes and
group memberships, and rejects a selection containing non-mesh objects.

`ExtractMeshFacesByEdgeLength EdgeLength=0.1 Select=Shorter` extracts faces
with any boundary edge strictly shorter than the given length. `Select=Longer`
uses the longest boundary edge and a strict greater-than test. Quad diagonals
are excluded. It shares the area command's `MakeCopy` and `BorderOnly` behavior,
atomic staging, attributes, groups, selection, and undo.

`ExtractMeshFacesByAspectRatio AspectRatio=9` extracts faces whose aspect ratio
is strictly greater than the threshold. A triangle's ratio is its longest edge
divided by the opposite altitude. A quad uses the largest ratio among its four
vertex triples, matching [McNeel's stated quad rule](https://discourse.mcneel.com/t/mesh-elements-aspect-ratio/181343/11).
A square therefore has ratio 2. Collinear triples count as infinite ratio.
`MakeCopy` and `BorderOnly` use the same output policy as the other metric
extraction commands. The triangle formula is an independent implementation;
direct Rhino oracle comparison is still needed for exact threshold parity.

`ExtractMeshFacesByDraftAngle StartAngle=0 EndAngle=45 ViewDirection=0,0,1`
extracts stored faces whose oriented polygon normal makes an angle in the
inclusive range with the direction from the model toward the viewer. The app
supplies its active viewport direction when `ViewDirection` is omitted;
standalone command calls supply it explicitly. `MakeCopy=Yes` and
`BorderOnly=Yes` follow the other face-filter commands. Rhino's older
`StartAngleFromCameraDir`, `EndAngleFromCameraDir`, and `GetBorder` option
names are accepted. Exact command thresholds and defaults still require direct
Rhino oracle comparison.

`ExtractConnectedMeshFaces Face=0 Angle=0 Compare=Less` extracts the region
reachable from stored face 0 across topological edges. Each neighboring face
pair must have a normal angle less than or equal to `Angle`; `Compare=Greater`
uses greater than or equal instead. The default is 0 degrees with `Less`, so
connected coplanar faces are extracted. `Face` is a zero-based index on each
selected mesh; the same seed index is used for every selected mesh. Unwelded
vertices at identical positions count as connected. `MakeCopy=Yes` and
`BorderOnly=Yes` preserve the source mesh and use the other extraction
commands' attributes, groups, result selection, and undo behavior. Viewport
subobject picking is still pending, so scripts must supply `Face`.

`ExtractMeshPart Face=0` extracts the region reachable from stored face 0
without crossing naked, unwelded, or nonmanifold topology edges. `Faces=0,2`
selects multiple seed regions, and `Faces=All` selects every region. The script
option `ExtractToNonManifoldEdges=No` allows traversal across nonmanifold
edges, while `ExtractWholeDisjointParts=Yes` crosses both nonmanifold and
unwelded edges, stopping only at naked edges. An unwelded edge has distinct
raw vertex indices on both ends for each incident face. `JoinOutput=Yes`
combines selected faces into one mesh per source; `JoinOutput=No` emits one
mesh per face. `MakeCopy=Yes` retains the source; `BorderOnly=Yes` emits a line
segment for each boundary edge and leaves the source unchanged. Viewport
subobject picking remains pending, so scripts supply `Face` or `Faces`.

`TriangulateMesh` splits every quad on selected meshes along its shortest 3D
diagonal, choosing A-C on exact ties. First triangles replace their source
quads in place and second triangles append in source-quad order; vertices,
object identity, attributes, groups, and selection remain unchanged.

`QuadrangulateMesh Planarity=1 Rectangularity=2` merges consistently oriented
triangle pairs that share raw vertex indices. `Planarity` is the maximum angle
between face normals in degrees; `Rectangularity` bounds the ratio of the two
prospective quad diagonal lengths. The local defaults are 1 degree and 2.
The candidate must also form a convex, nondegenerate quad. Pairing favors the
most balanced diagonals, then the smallest normal angle, with stable face-index
tie breaking. Unwelded seams and n-gon boundaries remain intact; vertex colors,
object identity, attributes, groups, selection, and undo are preserved.

`SwapMeshEdge` replaces a welded interior edge shared by exactly two
consistently oriented triangle faces with their opposite diagonal. Use
`Edge=1` for the deterministic exact-location topology index, or omit it to
pick one edge in the viewport. Vertex storage, face slots, object identity,
attributes, groups, and selection are preserved. Swaps that would create a
degenerate face are rejected to retain Viboceros's validated-mesh invariant.

`CollapseMeshEdge` follows RhinoCommon's deterministic API behavior by moving
both topology endpoints to their midpoint. Use `Edge=1` for the topology index,
or omit it to pick one selected-mesh edge in the viewport. Collapsed triangles
are removed, collapsed quad sides become triangles, independent unwelded seam
components remain distinct, and surviving faces and vertices retain source
order. Surviving objects keep identity, attributes, groups, and selection; an
empty result deletes the object. A collapse that would leave a zero-area face
is rejected atomically to preserve the validated-mesh invariant.
N-gon overlays follow their surviving face groups; an overlay whose entire
face group is removed disappears.

`SplitMeshEdge` divides a selected topology edge at a normalized parameter in
its deterministic wireframe direction. Use `Edge=1 Parameter=0.25`, provide an
edge point followed by a split point, or enter the bare command for the same
two-pick viewport workflow. Affected triangles become two triangles and quads
become three; unaffected faces stay first and replacements append in Rhino
order. Welded faces share one appended split vertex, while unwelded replacement
triangles remain fully separated. Exact endpoint parameters preserve Rhino's
coincident topology behavior. Object identity, attributes, groups, selection,
and undo are retained; tolerance-degenerate results are rejected atomically.
N-gon overlays are rebuilt over retained and replacement faces when they still
form one valid raw-edge-connected region.

`FillMeshHole` follows the closed naked boundary containing a picked topology
edge and fills it with a constrained-Delaunay triangle patch. Use `Edge=1`,
pick near an edge, or enter the bare command for a one-pick viewport workflow.
`JoinMesh=Yes` (the default) keeps the source identity, attributes, groups, and
selection; `JoinMesh=No` creates a separately selected patch with the source
attributes. The joined representation preserves Rhino's duplicated raw
boundary storage while exact-location topology closes the seam. Patch winding
is made consistent with the source, tilted and mildly nonplanar boundaries are
projected stably, and ambiguous branched or self-crossing boundaries are
rejected atomically. Existing source n-gons are retained in the joined mesh.

`FillMeshHoles` fills every simple closed naked boundary on every selected
mesh, including outer borders, and keeps each repaired mesh's identity,
attributes, groups, and selection. It stages the full selection atomically,
leaves already-closed meshes unchanged, and rejects ambiguous branched or
self-crossing boundary topology rather than guessing a repair.
Existing source n-gons remain attached through each filled boundary.

## Direction, welding, and topology

`UnifyMeshNormals` repairs inconsistent face winding across exact
location-welded manifold edges, including STL-style triangle soup, and rejects
non-orientable constraints atomically. `Dir UReverse|VReverse|SwapUV` reverses
either parameter direction or transposes selected untrimmed NURBS surfaces
exactly; `Mode=FlipU|FlipV|SwapUV` is also accepted. Rational weights,
non-clamped domains, periodic directions, identities, attributes, groups,
selection, and undo are preserved. `Dir Flip` currently supports curves and
meshes only; its interactive surface menu remains incomplete. Standalone
[`Flip`](flip.md) (aliases `Reverse` and `Rev`) also reverses open surfaces and
B-reps without reparameterizing them. Consistently oriented closed B-rep solids
and unsupported objects are skipped; closed meshes and inconsistently oriented
closed B-reps can flip. Identities, attributes, groups, and
closed-curve seams are preserved.
These three commands borrow selected input geometry while constructing their
replacement results, avoiding an extra copy of every mesh, curve, or surface
before the geometry operation. All results are still staged before document
mutation; the document retains its normal undo/redo states.

`Weld` merges exactly coincident endpoints only where mesh faces share a whole
edge and their normal angle is within the supplied 0-to-180-degree tolerance.
It matches Rhino's survivor ordering, compacts unused vertices, never merges a
vertex-only contact, and preserves object identity, metadata, groups, and
selection. The default is 180 degrees; `Angle=90` is also accepted.
Whole-mesh `Weld`, `Unweld`, `CombineIdenticalMeshVertices`, and
`CullUnusedMeshVertices` likewise borrow selected inputs while staging new
geometry, without an extra input snapshot. This does not remove the allocations
needed to construct results or retain document history.
The shared topology-source adapter also borrows meshes for edge/vertex weld
and unweld, edge swap, collapse, and split staging. Hole filling borrows the
document mesh and attributes through staging, copying attributes only into
owned output plans and passing the same borrowed mesh to topology selection.
These borrows end before document mutation; staged results remain owned.
A shared topology-source validator preserves selection action order and
command-specific unsupported-geometry errors, rejecting an empty selection
or a non-mesh source before topology work. Tests check borrowed identity,
hidden group-selected peers, and unchanged state on a late non-mesh source.

`WeldEdge` merges the raw endpoint sets incident to selected exact-location
mesh topology edges. Use `Edges=0,2`, `Edges=All`, or omit the selector to pick
one edge in the viewport. The earliest source vertex survives; unrelated
coincident fan components remain separate, including at half-welded and
non-manifold seams. Naked or already-welded selections still perform
Rhino-compatible unused-vertex compaction while preserving the object.

`WeldVertices` welds every joined seam incident to selected topology vertices,
including both endpoints of each seam. Use `Vertices=0,2`, `Vertices=All`, or
omit the selector for one viewport pick. It preserves Rhino's later-source
vertex ordering, ignores coincident vertex-only contacts, limits non-manifold
edges to their first two face uses, and performs unused-vertex compaction.

`Unweld` performs the inverse topology edit at edges whose face-normal angle is
greater than or equal to its tolerance. It preserves smoother face regions,
rebuilds affected vertices in OpenNURBS radial order, and compacts unused
vertices. The zero-degree default separates every adjacent face region;
`ModifyNormals=Yes|No` is accepted for Rhino script compatibility while mesh
normals remain derived data.

`UnweldEdge` adds seams at selected exact-location mesh topology edges. Use
`Edges=0,2`, `Edges=All`, or omit the selector to pick one edge in the viewport.
It partitions closed and high-valence radial face fans, handles non-manifold
edges, and preserves existing seams; naked or already-unwelded selections only
trigger Rhino-compatible vertex compaction. At a non-manifold edge, an endpoint
with existing partial sharing stays unchanged; only endpoints shared by every
incident edge face are separated. `ModifyNormals=Yes|No` is accepted,
but normals remain derived data.

`UnweldVertex` gives every face incident to each selected topology vertex its
own raw mesh vertex. Use `Vertices=0,2`, `Vertices=All`, or omit the selector
for one viewport pick. Closed, high-valence, already-unwelded, and non-manifold
fans follow Rhino's radial ordering, and unused raw vertices are compacted.
`ModifyNormals=Yes|No` is accepted while normals remain derived data.

`CombineIdenticalMeshVertices` turns exactly coincident raw vertices into shared
indices without culling unused vertices, preserving object identity, metadata,
face order, and winding.

`CullUnusedMeshVertices` removes raw vertices not referenced by any face while
preserving the source order and identity of every referenced vertex, including
coincident vertices, as well as object metadata, face order, and winding.

These vertex and edge rewrites retain n-gon overlays when their member faces
still form one valid raw-edge-connected region. An unweld that separates an
n-gon's internal edge removes that overlay.

`SplitDisjointMesh` separates exact-location edge-connected components (a lone
shared vertex does not connect them), creating fresh IDs for every piece and
copying source attributes and group membership. Ordinary split sources are
deleted; object-hidden, object-locked, and layer-locked group-selected sources
remain intact alongside their new pieces. A hidden layer alone does not prevent
source deletion. Connected sources remain unchanged.
Output selection is exact: pieces and retained selected sources are selected,
including restricted results, without expanding overlapping groups.
Native regression tests cover both restrictions, either source order, rejected
arguments with redo history, and exact object/group undo and redo.
Native output staging has a cumulative one-million-piece safety limit. Mesh
connectivity is analyzed before the limit is checked, but component geometry is
not copied for an over-budget source. Connected sources consume no piece budget.
This is a native resource guard, not a measured Rhino limit.

**Live Rhino 8.32 evidence (2026-09-12):** the dedicated
[`mesh_split_picking` probe](../oracle.md) observes normal split sources being
deleted, with new IDs for every piece. Hidden/locked group-selected sources stay
intact while new pieces with the same mode and group are added. All pieces and
the retained restricted source remain selected. The native regression
`split_disjoint_mesh_matches_live_rhino_picking_observations` compares all 16
recorded cases exactly: coordinates, face counts, identity retention, modes,
layer visibility/locking, ordered groups, and selection, including untouched
peers. Coverage includes every seed in a two-group overlap in both bridge
membership orders, and hidden/locked layers. It does not verify document table
order or combinations of object and layer restrictions. Mixed connected sources
remain selected; one locked-connected case also verifies live Undo/Redo selection.
The same 16 native setups also run two complete undo/redo cycles, comparing
object identity, geometry, attributes, table order, group records, and selection
membership before and after each replay. Object/group replay is a native invariant;
selection uses live history observations where available. The comparison adapter
in `mesh_decomposition_tests.rs` is shared with Explode; other split-specific
tests live beside the command in `split_disjoint_mesh/tests.rs`.
The geometry-only `SplitDisjointPieces` probe cannot verify this document behavior.

`ExtractDuplicateMeshFaces` separates all but one face from each duplicate
class. Face equality uses exact vertex locations and ignores raw indices,
cyclic ordering, and winding; attributes and group membership are preserved.

`ExtractNonManifoldMeshEdges` removes faces around edges shared by three or
more faces into attribute- and group-preserving mesh objects. Options can limit
the operation to hanging faces or raise the minimum incident-face count.
