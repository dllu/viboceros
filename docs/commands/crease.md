# Crease and RemoveCrease

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

`Crease` sharpens SubD control edges and corners or unwelds mesh edges.
`RemoveCrease` smooths SubD components or welds mesh edges. Open SubD boundaries
remain creased. Select SubD control grips with `PointsOn`, then run either command;
the grips remain selected when the command completes.

Use Ctrl+Shift to preselect mesh edges or SubD edges and vertices, then run the
command. With no eligible preselection, run the command first, pick components,
and press Enter. Clicks and window/crossing selection share the same component
indices. Ctrl removes picks; Ctrl+Shift toggles a clicked pick. `SelNone` clears
the pending selection, and Escape cancels without changing model history.
Overlapping components offer numbered choices. With `PointsOn`, SubD picks use
the visible control cage and preserve selected grips.

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
Mesh/SubD edges use their original topology, excluding display tessellation
diagonals and SubD refinement interiors.

The complete edit is prepared before admission. Invalid or stale inputs leave
geometry and history unchanged. Objects keep their IDs, names, layers, groups and
colors. Processed meshes move to the document end before processed SubDs. A true
SubD no-op preserves Redo; Rhino-compatible mesh processing creates an Undo step
even when the selected edges already have the requested state.

The [qualification record](../crease-qualification.json) covers 110 successful
Rhino 8.32 workflows: mixed edges/vertices, corners and boundaries, soft edges,
nonmanifold meshes, colors, n-gons, control grips and Undo/Redo. An additional
220 application replays cover preselection and postselection against those saved
results. Controls, vertex
limit stations and density-two surface facets agree within `1e-6`. Mesh comparison
allows raw vertex-index permutations and cyclic face origins; native empty
deleted n-gon slots are retained in captures and omitted from the comparison view.
SubD edge samples retain source identities through public OpenNURBS refinement;
their density-two stations and segments follow the captured native surface mesh.
The displayed polylines approximate smooth curves between evaluated stations.

Rhino 8.32 ignores `RemoveCrease` on an already-smooth vertex with soft weights;
select its soft edges to remove those weights. Later
[official help](https://docs.mcneel.com/rhino/8/help/en-us/commands/crease.htm)
describes different vertex behavior. Continuous surface error, physical native
mouse gestures, material/texture channels and comparative performance remain
unqualified. Eight failed SDK postselection attempts and three exporter failures
are retained as excluded diagnostics.
