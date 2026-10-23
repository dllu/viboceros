# TriangulateMesh

[Meshes and SubDs](meshes-index.md) · [Mesh editing](mesh-editing.md)

`TriangulateMesh` splits the quadrilateral faces of selected meshes. Start it
with meshes preselected, or start the command, select meshes and press Enter.
Escape or Cancel abandons command-first selection. There are no numeric options.

OpenNURBS chooses the shortest spatial diagonal, using A-C on a tie. First
triangles replace their source quad slots; second triangles append in source
order. Vertices, including unused vertices, and stored normals, texture
coordinates and colors remain unchanged. Valid small features do not depend on
document tolerance.

The command removes **all n-gon overlays**, including overlays made entirely of
triangles. The geometry API `TriangleMesh::triangulate_quads` instead follows
the public SDK: it retains overlays and appends new face indices after each
original n-gon face list.

Non-mesh selections are skipped. Processed meshes retain their identities,
attributes and groups, move to the object table end and record one Undo step,
even when all their faces were already triangles. Preselected meshes stay
selected after acceptance; command-first picks are released. Undo restores the
source geometry and preselection, while Redo releases processed selection.
All geometry is staged before document mutation; a failed conversion preserves
geometry, order, selection and history.

The [qualification record](../triangulate-qualification.json) compares 17
independent public SDK inputs and 19 native command workflows, including
triangle-only n-gons, mixed mesh/SubD selections, multiple meshes, postselection,
cancellation and Undo/Redo. All 17 accepted command replays and 19 application
getters match the recorded raw geometry, channels and document state.

Physical postselection, restricted objects, grips, degenerate-input repair,
coordinates near numeric limits, other units and comparative performance remain
unqualified. Native input is bounded by the shared mesh resource limits.
