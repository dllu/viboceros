# SubD InsertPoint

InsertPoint edits the original SubD control cage, retaining face materials,
colors and UVW storage through previews and document Undo/Redo. Local Undo removes
the last accepted pick. A pick on a disconnected island is rejected and leaves
the previous candidate intact; a later pick can continue the accepted path.

An edge split keeps each incident face's stored UVW prefix and appends an unset
corner. Repeated splits preserve earlier unset corners. Untextured faces remain
untextured. The native adapter initializes every added slot explicitly, including
reused capacity, so archived results cannot contain uninitialized coordinates.
Face splitting follows public OpenNURBS behavior for retained and new faces.

## Evidence

Three immutable, acknowledged mouse-input captures contain 54 workflows and 332
Rhino-saved 3DM models. They cover single/interior edges, same-edge points in both
directions, crossings, three-point paths, local Undo, Undo-all, Cancel, materials,
missing UVW arrays, coherent/constant UVWs, a pentagon, grids and prior MergeFaces
edits. The I/O and application audits replay the edits independently from input
archives and inspect the final and historical states.

Rhino assigns new component IDs in an order that can vary. Repeating the same
16-case request changed new vertex IDs in `grid-reverse-three` and
`merged-grid-reverse`, with identical control geometry. The insertion audit
therefore resolves new vertices, edges and faces by unique geometry/topology.
Existing component IDs, oriented loops, tags and sharpness remain checked. This
rule applies to these insertion captures; other SubD qualification keeps its
existing identity checks. Numerical geometry uses `1e-7`; material channels,
colors, texture modes and stored UVWs are exact. Oriented sampled limit facets
are also compared.

The [receipt](subd-insert-validation.json) records source hashes, immutable inputs
and checks. The existing warped-mesh insertion gaps remain outside this SubD
qualification. Arbitrary nonmanifold, repeated crossing and perspective paths
still need broader evidence.

```sh
cargo test -p viboceros-geometry --test subd_edge_insert
cargo test -p viboceros-io --test subd_face_merge
cargo test --bin viboceros custom_uvw_insertions
python3 -m unittest tools.rhino_oracle.test_subd_insert_custom
```
