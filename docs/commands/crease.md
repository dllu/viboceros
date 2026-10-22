# Crease and RemoveCrease

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

`Crease` sharpens SubD control edges and corners or unwelds mesh edges.
`RemoveCrease` smooths SubD components or welds mesh edges. Open SubD boundaries
remain creased. Select SubD control grips with `PointsOn`, then run either command;
the grips remain selected when the command completes.

For scripts, select the parent object and specify zero-based source components:

```text
Crease Edges=0,3
Crease Vertices=0,1
RemoveCrease Edges=All
RemoveCrease Edges=3 Vertices=0 Object=<selected-uuid>
```

SubD indices follow the stored control-net tables. Mesh indices identify exact
position topology edges, including unwelded seams. `Vertices=` applies to SubDs.
Both options accept `All`; a bare whole-object selection is insufficient.
Interactive mesh/SubD edge and SubD vertex component picking is pending.

The complete edit is prepared before admission. Invalid or stale inputs leave
geometry and history unchanged. Objects keep their IDs, names, layers, groups and
colors. Processed meshes move to the document end before processed SubDs. A true
SubD no-op preserves Redo; Rhino-compatible mesh processing creates an Undo step
even when the selected edges already have the requested state.

The [qualification record](../crease-qualification.json) covers 110 successful
Rhino 8.32 workflows: mixed edges/vertices, corners and boundaries, soft edges,
nonmanifold meshes, colors, n-gons, control grips and Undo/Redo. Controls, vertex
limit stations and density-two surface facets agree within `1e-6`. Mesh comparison
allows raw vertex-index permutations and cyclic face origins; native empty
deleted n-gon slots are retained in captures and omitted from the comparison view.

Rhino 8.32 ignores `RemoveCrease` on an already-smooth vertex with soft weights;
select its soft edges to remove those weights. Later
[official help](https://docs.mcneel.com/rhino/8/help/en-us/commands/crease.htm)
describes different vertex behavior. Continuous surface error, physical native
mouse gestures, material/texture channels and comparative performance remain
unqualified. Eight failed SDK postselection attempts and three exporter failures
are retained as excluded diagnostics.
