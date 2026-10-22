# SubD vertex seams

[Mesh seams](mesh-seams.md) · [Crease editing](crease.md)

`WeldVertices` smooths selected interior SubD control vertices and their incident
creases. Boundary vertices stay unchanged, including boundary corners with an
incident interior crease. An already-smooth vertex retains its soft edge weights.
`UnweldVertex` makes selected vertices corners, including boundary vertices.
These operations preserve control topology and vertex indices.

Ctrl+Shift preselect SubD vertices, then run the command. Alternatively, start
the command, select vertices, and press Enter. Clicks and window selection use
the same component table. `SelNone` clears pending picks; Escape cancels.
`PointsOn` exposes control grips; `EditPtOn` exposes their limit-surface stations.
Moving surface grips solves the native limit masks for selected controls;
unselected controls stay fixed and neighboring surface points can move.

For scripts, select mesh or SubD parents and use `Vertices=0,2`, `Vertices=All`,
or one coordinate to pick the nearest vertex. Bare commands accept selected
SubD grips. `UnweldVertex ModifyNormals=No` preserves mesh normal directions;
this option does not change the SubD tag policy.

The complete batch is staged before admission and commits one Undo step while
preserving IDs and metadata. Welding clears vertex/grip selection before
recording history and moves changed SubDs before changed meshes. Unwelding
preserves selected grips and owner selection, moving changed objects to the end
in document order. True SubD no-ops preserve Redo.

The [qualification record](../vertex-seams-qualification.json) compares 46 saved
Rhino 8.32 workflows, including six mixed mesh/SubD batches. Another 80 application
replays exercise the 40 pure-SubD recipes before and after command start.
Control topology, tags, sampled limit stations/facets, metadata, order, grip
selection and Undo/Redo agree within `1e-6`. This does not certify arbitrary
topology, continuous surface error, native mouse gestures or performance.
Nine surface-grip movement captures provide six native matches. Two weighted-edge
cases and an unstable all-selected cube solve retain explicit native gaps;
Viboceros meets the requested selected stations and uses exact whole-object
affine transforms.

SubD `WeldEdge` and `UnweldEdge` remain unimplemented. Their 26 captures are
diagnostics: the weld automation made no pure-SubD edit, and unwelding can split
crease topology. `Crease` and `RemoveCrease` remain available for tag-only edits.
