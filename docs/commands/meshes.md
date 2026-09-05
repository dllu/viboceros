# Polygon meshes

[Command reference](README.md) · [Project overview](../../README.md)

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

`MeshToNURB` duplicates every selected mesh polygon as an exact degree-one
NURBS face. Quads remain potentially warped bilinear surfaces; triangles are
trimmed planar parallelograms by default, or untrimmed patches with a collapsed
side when `TrimTriangularFaces=No`. Exact-location edges become shared B-rep
topology and edge-disconnected pieces become separate, unselected objects.
Sources and their selections are retained, derived attributes are copied, and
source groups are not inherited. `UseNgons` is accepted for Rhino script
compatibility; the current native mesh model contains triangles and quads only.

## Mesh extraction and editing

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

`TriangulateMesh` splits every quad on selected meshes along its shortest 3D
diagonal, choosing A-C on exact ties. First triangles replace their source
quads in place and second triangles append in source-quad order; vertices,
object identity, attributes, groups, and selection remain unchanged.

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

`SplitMeshEdge` divides a selected topology edge at a normalized parameter in
its deterministic wireframe direction. Use `Edge=1 Parameter=0.25`, provide an
edge point followed by a split point, or enter the bare command for the same
two-pick viewport workflow. Affected triangles become two triangles and quads
become three; unaffected faces stay first and replacements append in Rhino
order. Welded faces share one appended split vertex, while unwelded replacement
triangles remain fully separated. Exact endpoint parameters preserve Rhino's
coincident topology behavior. Object identity, attributes, groups, selection,
and undo are retained; tolerance-degenerate results are rejected atomically.

`FillMeshHole` follows the closed naked boundary containing a picked topology
edge and fills it with a constrained-Delaunay triangle patch. Use `Edge=1`,
pick near an edge, or enter the bare command for a one-pick viewport workflow.
`JoinMesh=Yes` (the default) keeps the source identity, attributes, groups, and
selection; `JoinMesh=No` creates a separately selected patch with the source
attributes. The joined representation preserves Rhino's duplicated raw
boundary storage while exact-location topology closes the seam. Patch winding
is made consistent with the source, tilted and mildly nonplanar boundaries are
projected stably, and ambiguous branched or self-crossing boundaries are
rejected atomically.

`FillMeshHoles` fills every simple closed naked boundary on every selected
mesh, including outer borders, and keeps each repaired mesh's identity,
attributes, groups, and selection. It stages the full selection atomically,
leaves already-closed meshes unchanged, and rejects ambiguous branched or
self-crossing boundary topology rather than guessing a repair.

## Direction, welding, and topology

`UnifyMeshNormals` repairs inconsistent face winding across exact
location-welded manifold edges, including STL-style triangle soup, and rejects
non-orientable constraints atomically. `Dir UReverse|VReverse|SwapUV` reverses
either parameter direction or transposes selected untrimmed NURBS surfaces
exactly; `Mode=FlipU|FlipV|SwapUV` is also accepted. Rational weights,
non-clamped domains, periodic directions, identities, attributes, groups,
selection, and undo are preserved. `Dir Flip` reverses the same selected curve
and mesh types as `Flip`; standalone surface-normal orientation is not yet a
separate property in the native surface model. `Flip` (aliases `Reverse` and `Rev`)
reverses selected curve directions or every face in selected meshes without
changing object identities, attributes, groups, or closed-curve seams.

`Weld` merges exactly coincident endpoints only where mesh faces share a whole
edge and their normal angle is within the supplied 0-to-180-degree tolerance.
It matches Rhino's survivor ordering, compacts unused vertices, never merges a
vertex-only contact, and preserves object identity, metadata, groups, and
selection. The default is 180 degrees; `Angle=90` is also accepted.

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
trigger Rhino-compatible vertex compaction. `ModifyNormals=Yes|No` is accepted,
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

`SplitDisjointMesh` separates exact-location edge-connected components (a lone
shared vertex does not connect them), retaining the first object identity and
copying attributes and group membership to additional pieces.

`ExtractDuplicateMeshFaces` separates all but one face from each duplicate
class. Face equality uses exact vertex locations and ignores raw indices,
cyclic ordering, and winding; attributes and group membership are preserved.

`ExtractNonManifoldMeshEdges` removes faces around edges shared by three or
more faces into attribute- and group-preserving mesh objects. Options can limit
the operation to hanging faces or raise the minimum incident-face count.
