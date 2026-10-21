# MeshPolyline

`MeshPolyline` creates a triangle mesh from each selected closed polyline.
Preselected inputs run immediately. Otherwise, start the command, select closed
polylines and press Enter. Escape cancels selection without consuming Redo.

```text
MeshPolyline
MeshPolyline Sources=<id>,<id>
```

`Sources` is a scripting extension that names the inputs explicitly. It rejects
duplicate, missing, hidden, locked or ineligible sources before document edits.
Closed degree-one NURBS polylines are also eligible. Open curves remain
ineligible even when their endpoints lie within the document tolerance.

The original curves remain unchanged. Each result copies its source's name,
object color and layer; the current layer does not change. Source and result
objects finish unselected. Multiple outputs share one Undo operation, and Undo
restores the unselected sources. Nested curves produce independent meshes.

Construction reuses constrained polygon triangulation and retains the original
spatial coordinates. Convex four-corner regions use the shared shortest spatial
diagonal comparison. A flood fill classifies the exterior without testing every
triangle against every boundary segment. The combined input is limited to
100,000 vertices. Empty construction creates no history entry and preserves Redo.

The [qualification report](../closed-polyline-qualification.json) retains 31 Rhino
captures covering concave boundaries, reversed winding, changed seams, collinear
edge points, tilted and nonplanar curves, degree-one NURBS, nested and multiple
inputs, attributes and pre/postselection workflows. Thirty SDK and UI cases
match bidirectional samples and raw vertex/face counts at `1e-6`.

The captured self-crossing input produces a partial triangle in Rhino; local
construction returns no mesh. This gap remains explicit. Three native cancelled
cases have SDK and UI document-state replays; direct CLI cancellation parity
remains unqualified. General projection-ambiguous spatial polygons, native face
ordering and comparative speed need further qualification.

References: [McNeel MeshPolyline command](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshpolyline.htm)
and [public Mesh.CreateFromClosedPolyline API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.mesh/createfromclosedpolyline).
