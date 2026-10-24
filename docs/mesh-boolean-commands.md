# Mesh Boolean commands

Enable `native-smlib` for `MeshBooleanUnion`, `MeshBooleanIntersection`,
`MeshBooleanDifference`, and `MeshBooleanSplit`. They accept meshes, surfaces,
and polysurfaces. Surface inputs use the existing display mesher; curved-surface compatibility is pending.

Union takes at least two selected objects. Intersection, Difference, and Split
use two selection sets. Press Enter after each set. Scripts can supply
`FirstSet=id,id SecondSet=id,id` to the two-set commands.

`Tolerance=Auto|Document|Zero` is shared across the four commands and survives
cancellation. `Document` uses model tolerance; `Zero` uses ten OpenNURBS
zero-tolerance units. `Auto` scales model tolerance by the captured public mesh-intersection
coefficient, `0.0001`. Custom coefficient settings remain pending. Numeric values
and `Literal` are rejected by native Rhino and by this implementation. The Zero policy follows McNeel's
[public command clarification](https://discourse.mcneel.com/t/meshbooleanoptions/201908/6).

Difference offers `DeleteInput=Yes|No` in the cutter getter. It controls cutter
retention; targets are replaced in either mode. The accepted setting persists
after cancellation. Union and Intersection split disconnected bodies into
separate objects. Difference returns one mesh per input target; separated pieces
of one target share that mesh. Split keeps its cutters and returns outside/inside partitions.
[Multi-set and Split evidence](mesh-boolean-sets.md) covers target boundaries,
output order, cutter retention, and remaining partition differences.

Results preserve the first source's identity, name, layer, object color, and group
memberships. Undo restores the original shared geometry. All geometry is staged
before document edits, and invalid inputs preserve selection and Redo.

The original three commands have a [baseline qualification](mesh-boolean-command-qualification.json) replays
79 command invocations and 85 application getter workflows against saved Rhino
scenes, including cancellation, option memory, source order, disconnected bodies,
through holes, cutter retention, and Undo/Redo. It compares actual per-object
meshes as well as metadata; stored output normals, UVs, and vertex colors are
outside this comparison. Rejected exploratory macros remain separately listed.
Coincident, face-contact, overlapping first-set and thin-overlap differences
remain explicit gaps. Area and volume checks catch duplicate or overlapping
facets even when sampled boundaries agree.
See the [backend evidence](mesh-boolean-geometry.md) for triangulation and kernel
ownership details. Surface samples do not prove continuous surface equality.

```sh
cargo test -p viboceros-command --features native-smlib --test mesh_boolean_command_native -- --nocapture --test-threads=1 > /tmp/mesh-boolean-commands.log 2>&1
cargo test -p viboceros --features native-smlib mesh_boolean -- --nocapture --test-threads=1 > /tmp/mesh-boolean-getters.log 2>&1
python3 -m tools.rhino_oracle.report_mesh_boolean_commands /tmp/mesh-boolean-commands.log /tmp/mesh-boolean-getters.log
```
