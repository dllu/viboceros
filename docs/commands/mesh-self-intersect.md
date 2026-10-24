# MeshSelfIntersect

`MeshSelfIntersect` creates polylines where each selected mesh intersects itself.
Select one or more meshes, surfaces, or polysurfaces, or start the command and
pick objects before pressing Enter. Separate selected objects are processed
independently; intersections between them belong to [MeshIntersect](mesh-intersect.md).
Sources remain unchanged. Output uses the current layer and default attributes.
Sources and results remain selected after execution; Undo and Redo clear those
transient selections. An empty result creates no geometry history entry.

Build with `native-smlib`. The command reuses usd-brep/SMLib's spatial index and
independent triangle-pair imprints. Each unordered pair is queried once. Ordinary
noncoplanar shared edges and coplanar triangulation seams are omitted; genuine
crossings through shared vertices remain. Coplanar regions union actual pairwise
overlap areas, avoiding the false coverage produced by intersecting a mesh with
an identical copy. Surface inputs use the display mesher. Computation completes
before document edits, preserving geometry, selection, and Redo on failure.

`Tolerance=Auto|Document|Zero` shares preferences with `MeshIntersect` and the
four mesh Boolean commands. `CoplanarFaces=Drop|Mesh|Polylines` shares its choice
with `MeshIntersect`. Accepted answers
persist after cancellation. The captured `Drop` and `Mesh` modes suppress
coplanar sheet boundaries; `Polylines` retains them. Ordinary coplanar edge
contacts are ignored. See the [native reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshselfintersect.htm).

The [qualification](../mesh-self-intersect-qualification.json) retains all 39
native workflows and checks ordered output, source identity, attributes,
selection, curve lengths, sampled geometry, shared options, Undo, and Redo at
`1e-7`. Thirty-four of 36 command executions and 37 of 39 application workflows
match. Coincident closed-shell preprocessing and the coincident mesh Boolean
control retain compatibility differences. Curved meshing,
near contacts, extreme scales, native polyline segmentation and parameter domains,
and comparative performance remain unqualified.

```text
MeshSelfIntersect Tolerance=Auto CoplanarFaces=Polylines
MeshSelfIntersect CoplanarFaces=Drop
```
