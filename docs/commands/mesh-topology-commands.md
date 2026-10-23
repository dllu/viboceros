# Direction, welding, and topology

[Polygon meshes](meshes.md) · [Meshes and SubDs](meshes-index.md)

[`UnifyMeshNormals`](unify-mesh-normals.md) repairs inconsistent face winding
across exact coincident locations, with distinct SDK normal-channel and command
replacement behavior. Its guide records component-seed and contradictory-cycle
limits. `Dir UReverse|VReverse|SwapUV` reverses
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

`AlignVertices DistanceToAdjust=0.05 AverageVertexesToAdjust=Yes` aligns
nearby naked mesh vertices across all selected meshes. Distances must be
strictly below the threshold. The default is the document absolute tolerance;
without averaging, the earlier mesh/topology vertex anchors each pair.
`Vertices=0,2` restricts movable topology vertices, and `NakedEdges=0,2`
restricts them to vertices on those naked topology edges. Unselected naked
vertices can anchor selected ones. Vertices sharing a face are never paired,
and an alignment that would degenerate any face is rejected before document
mutation. Raw face indices, colors, n-gons, object identities, attributes,
groups, selection, and undo are preserved.

`MatchMeshEdge DistanceToAdjust=0.05` first aligns close naked vertices, then
splits naked edges near an unmatched naked vertex and moves that vertex to the
new edge point. `AverageVertexesToAdjust=Yes` moves both the old vertex and new
split point to their midpoint. `RatchetMode=Yes` uses four increasing distance
passes. `PickEdges=0,2` uses those initial naked topology edge indices on each
selected mesh; `PickEdges=0:0,1:2` selects edge 0 on the first selected mesh
and edge 2 on the second.
`Join=Yes` joins touching output meshes; the default keeps input objects
separate. Invalid resulting faces are rejected before document mutation.

`ExtrudeMesh 2 Faces=0,1` displaces selected polygon faces and adds quads along
the selected region boundary. With no component option it extrudes every face
of each selected mesh. `Edges=0,2` extends selected naked topology edges with
quads. `Basis=UVN Direction=N` follows averaged selected face normals and is
the default; boundary edges also accept `Direction=V` to extend outward in
their adjacent face planes. `Basis=WCS Direction=X|Y|Z` uses a world axis;
`Direction=x,y,z` accepts any nonzero world vector. Original object identity,
attributes, colors, and valid n-gons are retained. Face and edge indices are
the source mesh's original indices. Invalid components or degenerate faces
reject the whole selection before document mutation.

[Mesh seam commands](mesh-seams.md) cover `Weld`, `WeldEdge`, `WeldVertices`,
`Unweld`, `UnweldEdge`, and `UnweldVertex`, including stored-normal options,
component selection, and measured compatibility limits.

`CombineIdenticalMeshVertices`, and
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
