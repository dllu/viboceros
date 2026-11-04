# Mesh and SubD face loops

`SelFaceLoop` selects face strips by mesh or SubD seed edges. Preselected edges
expand immediately; otherwise pick edges and press Enter. Bright seed edges
remain selected alongside the accepted faces. Faces preview dimly while picking.
Ctrl removes a seed; Ctrl on another edge of the strip does nothing. Escape
clears the newly picked seeds. Multiple seeds can contribute intersecting strips.

Strips cross opposite sides of quads and may be open or closed. Triangles,
non-quad SubD faces and nonmanifold continuations stop traversal. Seeds without
any crossed faces are discarded on acceptance. Meshes use exact-location welded
topology and stored faces, including individual faces inside logical n-gons.
SubDs use active control faces; mesh conversion can merge collinear boundaries
and produce different strips. The captured meshes do not require UV coordinates.

Successful expansion preserves other preselected faces and vertices. An empty
expansion clears component preselection. With no eligible preselected edges,
the getter clears preselected components. Whole-object selection remains selected
and its edges are ignored by the getter. These rules also apply to standalone
[edge loops](component-edge-loops.md) and [edge rings](component-edge-rings.md).

Selection adds no model-history entry and preserves pending redo. Changed sources
cancel the getter. Wireframe, shaded and ghosted modes share picking and previews.
Partial mesh n-gons retain their stored-face meaning in highlighting, `SubDivide`
and `ExtractPt`. Direct face picking continues to select logical mesh n-gons.

Headless callers can use these Viboceros scripting extensions:

```text
SelFaceLoop Edges=11 Object=<uuid>
SelFaceLoop Edges=All Object=<uuid>
```

Indices are zero-based mesh topology edges or active SubD edge ordinals. With
one whole owner selected, `Object` can be omitted. Invalid scripted selectors
preserve selection. Typed `ComponentPick` values share the same command backend.

`TriangleMesh::try_face_loop`, `SubD::try_face_loop` and reusable
`edge_ring_index().try_face_loop` return crossed faces, candidate seed edges and
closure. The command caches equivalent seeds on the same immutable source.
Only contributing picked edges remain selected, not every candidate seed edge.

[Native evidence](../oracle/face-loops.md) covers 131 workflows across all three
commands, independent source construction, imported archives and in-memory GUI
events. Nested selection inside another command and face double-click gestures
remain separate work.
