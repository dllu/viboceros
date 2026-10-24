# MeshSplit

`MeshSplit` partitions meshes, surfaces and polysurfaces into open pieces without
capping their cut boundaries. Select targets, press Enter, select mesh, surface,
polysurface or curve cutters, then press Enter. Preselected targets skip the first
selection. Results receive fresh IDs and inherit each target's attributes, layer
and groups. They are unselected; surviving cutters remain selected. Undo restores
original geometry storage and any productive preselection. Computation finishes
before document edits, so failure preserves geometry, selection and Redo.

Build with `native-smlib`. The command uses the shared
[usd-brep/SMLib partition kernel](../mesh-split-kernel.md). Curves extrude along the
active CPlane's Z axis. `IP` uses the existing infinite-plane picker, including
three points, world/CPlane axes, through-point, planar objects and named CPlanes.
`PermitSplittingInputsWithEachOther` allows targets to be picked as cutters;
select the cutters after enabling it.

`Tolerance=Auto|Document|Zero` shares its preference with the mesh Booleans and
intersection commands. `CoplanarFaces=Keep|Drop` shares their coplanar mode;
explicit `Keep` restores polyline output for subsequent intersections.
`CreateNgons=Yes|No` and `DiscardUnsplitMeshes=Yes|No` default to `Yes` and persist
after cancellation. Discard keeps an unsplit target unchanged; disabling it
admits a fresh intermediate mesh even when only one piece results. The command
reconstructs source quads and logical n-gons from source-face ownership.
See [Rhino's reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshsplit.htm).

The [qualification](../mesh-split-qualification.json) checks 58 command executions
and 65 picker workflows at `1e-7`, including ordered objects, geometry, attributes,
groups, selection, shared preferences, n-gon boundary length and covered surface,
Undo and Redo. Forty-six command executions and 53 picker workflows match all
checks. Twelve cases retain differences in component order, contact face grouping,
mixed cutters, three-cutter surface truncation or native n-gon boundaries.
The infinite-plane and CPlane curve cases match surface geometry but differ in
logical boundaries: some captured native boundaries omit part of their covered
faces. Vertex-channel interpolation, curved meshing, extreme scales, near contacts
and comparative performance remain unqualified.

```text
MeshSplit CreateNgons=Yes DiscardUnsplitMeshes=Yes
MeshSplit FirstSet=target-id SecondSet=cutter-id
MeshSplit FirstSet=target-id InfinitePlane=WorldYZ
```
