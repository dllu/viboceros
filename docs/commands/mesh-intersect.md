# MeshIntersect

`MeshIntersect` creates intersection polylines from meshes, surfaces, and
polysurfaces while retaining its source objects. Select at least two objects,
or start the command and pick them before pressing Enter. Results use the current
layer with default attributes. The sources and results remain selected after
execution; Undo and Redo clear these transient selections.

Enable `native-smlib`. The command reuses usd-brep/SMLib spatial queries and polygon
imprints on owned working meshes, uses iOverlay for coplanar coverage and holes, and
assembles segments through the existing curve-joining module. Surface inputs use
the display mesher. Geometry completes before document edits; failed operations
preserve source storage, selection, and Redo.

`Tolerance=Auto|Document|Zero` shares preferences with all four mesh Boolean
commands. `CoplanarFaces=Drop|Mesh|Polylines` persists across cancellation. The
licensed Rhino 8.32 command accepts singular `Mesh` and rejects `Meshes` and
`Tolerance=Literal`, despite the spellings in the newer
[online reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshintersect.htm).
The captured `Mesh` mode suppresses open-sheet overlap boundaries. Whole coincident
facet seams remain intersection lines; partial overlaps inside shared area are removed.

The [qualification](../mesh-intersect-qualification.json) replays 57 commands
and 60 application workflows against independently saved native scenes. Fifty-four
commands and 57 getters match at `1e-7`, including source identity, attributes,
selection, shared options, Undo, and Redo. Three rejected-option probes remain
separately recorded. Three workflows retain differences in coincident triangle-edge
traversal. Curved meshing, extreme scales, custom
tolerance coefficients, native point segmentation/parameter domains, and
comparative performance remain unqualified.

```text
MeshIntersect Tolerance=Auto CoplanarFaces=Polylines
MeshIntersect Tolerance=Document CoplanarFaces=Drop
```
