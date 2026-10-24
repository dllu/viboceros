# Empty mesh objects

Rhino can retain a selectable mesh with zero vertices and zero faces after
`MeshBooleanSplit`. It is invalid geometry and has no valid bounding box, but it
still has an object identity, attributes, and group memberships. An enclosed
target produces an empty outside object and a nonempty inside object. Later
productive cutters retain empty regions alongside the earlier Split stages.

Viboceros stores this as `Geometry::EmptyMesh`. Geometry kernel `TriangleMesh`
values still require vertices and faces. Document and display bounds are optional:
empty objects contribute no points to extents, clipping, snapping, or screen
picking. `SelMesh`, `SelBadObjects`, direct selection, transforms, deletion, and
history can operate on the stored object. Geometry calculations that require a
nonempty mesh reject it explicitly.

The [native qualification](empty-mesh-qualification.json) replays two independent
commands and two application workflows, checking each object's order, ownership,
selection, attributes, and geometry after execution, Undo, and Redo. Public native
queries establish invalidity, missing bounds, selection, File3dm admission, and
save/reload persistence. Rust IO tests read those native result files and preserve
all supported object fields through another 3DM round trip. Empty and mixed nested
block definitions also round-trip locally without manufacturing an origin bound;
their native command behavior and validity remain unqualified.

```sh
CARGO_INCREMENTAL=0 cargo test -p viboceros-document --test empty_mesh
CARGO_INCREMENTAL=0 cargo test -p viboceros-io --test empty_mesh
python3 -m tools.rhino_oracle.report_mesh_boolean_empty /tmp/mesh-boolean-commands.log /tmp/mesh-boolean-getters.log
```
