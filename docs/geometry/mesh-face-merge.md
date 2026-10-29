# Component-selected mesh face merging

`TriangleMesh::merge_face_components` merges connected mesh face sets through
the public OpenNURBS `MergeFaceSets` operation. This is the mesh kernel foundation
for `MergeFaces`; the application command is not registered yet.

`MeshMergeComponent` accepts original raw vertex or face indices, or an edge
identified by two raw endpoint indices. The bridge resolves native topology;
callers do not pass Rust edge ordinals as OpenNURBS indices. N-gon member facets
identify their logical face. Singleton logical regions retain their original
boundaries instead of being decomposed and rebuilt.

New merged faces require one boundary loop. Hole-bearing regions cannot become
one new face, while pre-existing hole-bearing n-gons remain intact. A scoped
thread-local boundary policy selects the original upstream path for this
operation, retaining the existing hole-aware coplanar adapter for other callers.
Explicit face picks can weld coincident seam vertices; the stored channels follow
the native operation. Invalid indices and failed native results leave the source
unchanged. Changed outputs must also pass the existing `MESH_VALIDATION`
fixed-triangle geometry check; its predicate and tolerances are unchanged.

## Qualification

The [validation receipt](mesh-face-merge-validation.json) binds the captured
requests, observations and current kernel sources.

Forty-eight actual Rhino command invocations cover faces, edges, vertices,
nonplanar regions, disjoint pieces, seams, existing n-gons, holes, partial rings,
reversed winding and empty selection, with and without colors, UVs and normals.
There are 36 storage changes and 12 unchanged results. Replays compare complete
before/after mesh storage: all vertices, ordered faces, channel values, n-gon
boundaries and member facets. Continuous values use `1e-7`; discrete values are
exact. All 48 outputs also pass fixed-triangle geometry validation. The same
audit runs in a binary that also initializes SMLib.

Regression tests exercise index rejection and restoration of the coplanar hole
policy across threads. The native captures retain selection, result and history
data for later command work. Their legacy `getter_opened` field records whether
the input timer fired; it does not qualify transient getter opening.

```sh
cargo test -p viboceros-geometry --test mesh_face_merge
cargo test -p viboceros-geometry --lib mesh::merge_faces
cargo test -p viboceros-oracle --test mesh_face_merge --features native-smlib
python3 -m unittest tools.rhino_oracle.test_mesh_merge_faces
```

SubD merging, mixed-component/disconnected-fan qualification, the `Results`
option, command selection/history policy and application picking remain to be
implemented before admitting the full command.
