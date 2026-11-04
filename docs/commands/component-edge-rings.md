# Mesh and SubD edge rings

`SelEdgeRing` expands preselected mesh/SubD edges immediately. With no eligible
preselection, pick seed edges and press Enter to accept the previewed rings.
Seeds are bright; other ring edges are dimmer. Ctrl removes a seed, and Escape
clears newly picked seeds. Multiple seeds can contribute separate rings or
share the same ring. Whole-object preselection remains selected and is ignored
by the edge getter; preselected faces and vertices are cleared.

Rings cross opposite sides of quads. They can be open or closed and stop at
triangles and nonmanifold continuations. A mesh seed used by more than two faces
can remain bright during picking but contributes no accepted ring. Vertex
valence, corner tags and crease tags do not choose the opposite side. Meshes use
exact-location welded topology and stored faces, including faces inside logical
n-gons. SubDs use active control faces; mesh-to-SubD n-gon conversion can merge
collinear edges and change the ring. UV coordinates are not required in the
captured mesh cases.

Selection stays outside model history and preserves pending redo. The result
feeds [ToNURBS](to-nurbs-edges.md) directly. `SelNone` clears component selection.
Invalid scripted selectors preserve the current selection; changed sources
cancel the interactive getter. Wireframe, shaded and ghosted views share the
same picking and previews.

Headless callers can use these Viboceros scripting extensions:

```text
SelEdgeRing Edges=3,0 Object=<uuid>
SelEdgeRing Edges=All Object=<uuid>
ToNURBS DeleteInputObjects=No
```

Indices are zero-based mesh topology edges or active SubD edge ordinals. With
one whole owner selected, `Object` can be omitted. The command layer's shared
`ComponentSelection` also accepts typed `ComponentPick` values.

The geometry API exposes `TriangleMesh::try_edge_ring`, `SubD::try_edge_ring`
and reusable `edge_ring_index` queries. Results retain ordered edge and crossed
face ordinals without repeating a closing edge. The command session caches
source topology and completed paths rather than rebuilding topology per seed.
`SubD::component_ids()` maps active ordinals to native IDs, including gaps.

[85 native controls](../oracle/edge-rings.md) qualify topology, seed accumulation,
removal, preselection, cancellation and redo/undo. Independent Rust recipes and
imported native archives agree on complete component sets. In-memory viewport
events cover ordinary grids and n-gons in every display mode without opening
windows. Captures use private Xvfb.

Nested selection inside another command, ring double-click gestures, face loops
and vertex chains remain separate work. Ctrl+Shift-double-click continues to
select an [edge loop](component-edge-loops.md).
