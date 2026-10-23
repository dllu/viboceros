# SubD edge seams

[Mesh seams](mesh-seams.md) · [Vertex seams](vertex-seams.md) · [Creases](crease.md)

`UnweldEdge` cuts selected smooth or creased SubD edges where the surrounding
face fans separate. Open paths, closed loops and branches can split topology.
A single cut inside a closed fan makes no change. Boundary vertices that split
become corners; other new vertices receive automatic boundary tags.

`WeldEdge` joins exact coincident boundary edges within one SubD. Selecting one
side suffices; its edge and endpoint identities survive. Welding smooths the
joined edges and retags their endpoints, including existing corners. Separate
SubDs and edges with a positive gap remain separate. Use `Crease` or
`RemoveCrease` for tag edits without splitting or joining topology.

Ctrl+Shift preselect mesh or SubD edges and run the command, or start the command,
select edges and press Enter. Selected SubD edges remain selected after the
edit, with retired edge indices mapped to their survivors. `SelNone` clears
pending picks and Escape cancels. Scripts accept selected parents with
`Edges=0,2`, `Edges=All`, or a coordinate for the nearest displayed edge.
`UnweldEdge ModifyNormals=No` affects mesh normals; either setting gives the
same SubD topology.

Mixed mesh/SubD batches stage all results before one Undo step and preserve
object IDs and metadata. Changed objects move to the end: SubDs first for
welding, meshes first for unwelding. True SubD no-ops preserve Redo.

The [qualification record](../edge-seams-qualification.json) covers 94 SubD
source comparisons from 86 saved Rhino 8.32 scenes, 66 successful command
replays and 132 frontend selection replays. Control topology, tags, sampled
limit points/facets, metadata, object order and Undo/Redo agree within `1e-6`.
Some cuts assign different indices to new coincident control vertices and new
edges; subsequent component-index scripts can therefore differ. Original control
and edge indices survive cuts, and weld survivor selection is checked separately.

Wire or nonmanifold edges, ambiguous parallel source edges and more than two
coincident boundary sides are rejected. Arbitrary topology, continuous surface
error, native mouse gestures and comparative performance remain unqualified.
Seven failed selection probes remain archived as diagnostics; the corrected
tolerance probes are included in the qualification.
