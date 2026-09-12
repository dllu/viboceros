# Non-manifold angle-unweld compatibility

[Oracle overview](oracle.md) · [Mesh topology](mesh-topology.md)

Status: **recorded mismatch corrected**. This concerns angle-based `Mesh.Unweld`,
not selected-edge `Mesh.UnweldEdge`. The earlier native implementation joined all
incident face pairs sharing a raw endpoint with a sufficiently small normal angle.
Live Rhino 8.32.26160.13001 measurements exposed excess sharing and incorrect
ordering at non-manifold junctions. All 69 non-manifold records below now replay
exactly, including raw vertex and face ordering.

## Reproducible measurements

- [36-case request](../tools/rhino_oracle/fixtures/mesh_unweld_nonmanifold.json) and
  [Rhino response](../tools/rhino_oracle/observations/mesh_unweld_nonmanifold.json):
  three triangles share one exact-location edge. Two have the same normal; the
  third is perpendicular. Cases vary all six face orders, fully shared versus
  partially shared endpoints, and both `ModifyNormals` values at a 45° threshold.
- [Three threshold cases](../tools/rhino_oracle/fixtures/mesh_unweld_nonmanifold_thresholds.json)
  and [Rhino response](../tools/rhino_oracle/observations/mesh_unweld_nonmanifold_thresholds.json):
  a fully shared edge with an unused source vertex, reversed face order with a
  partially shared endpoint, and zero-angle separation. These were captured in
  the same batch as the passing ordinary angle-unweld cases, then split into
  separate records without changing their values or per-case timings.
- [Six vertex-order cases](../tools/rhino_oracle/fixtures/mesh_unweld_vertex_order.json)
  and [Rhino response](../tools/rhino_oracle/observations/mesh_unweld_vertex_order.json)
  swap the two endpoint vertices without changing ordered face geometry. Sharing
  remains unchanged, arguing against sequential vertex processing as the cause.
- [24 four-face cases](../tools/rhino_oracle/fixtures/mesh_unweld_four_faces.json)
  and [Rhino response](../tools/rhino_oracle/observations/mesh_unweld_four_faces.json)
  vary every face permutation around an edge with two pairs of equal normals.
  They distinguish radial adjacency from joining all smooth pairs, and expose
  the ordering contribution of isolated radial edge groups.

For the 36-case matrix, label the original triangles `0`, `1` (the smooth pair),
and `2` (the perpendicular face). At endpoint A, Rhino preserves sharing between
`0` and `1` for face orders `021`, `201`, and `210`, but separates them for `012`,
`102`, and `120`. Endpoint B is fully separated in every measured case. Existing
partial sharing and `ModifyNormals` do not change those results in this matrix.
These measurements constrain the traversal policy; they do not prove parity for
every non-manifold mesh. Earlier native replay differed in all 36 matrix cases
and two of the three threshold cases. Those failures are now resolved without
changing their expected outputs.

The recorded vertex counts, coordinates, face indices, and ordering are exact.
The earlier mismatch included different vertex sharing, not just renumbering or
numerical roundoff. It was not resolved by increasing an epsilon. Stored normals are
not part of these records.

## Native traversal policy

Ordinary edges retain smooth face connectivity. At a non-manifold vertex, faces
are also considered in radial traversal order with an incoming edge for each
first occurrence. A qualifying incoming edge prevents joining its adjacent walk
faces. Subsequent adjacent faces can share only if both their normal-angle test
and existing raw endpoint indices agree. Non-adjacent smooth faces are not joined
merely because they use the same non-manifold edge.

Singleton radial groups contribute their faces at the group's position rather
than deferring them to source-face order. This matters both for subsequent smooth
grouping and for exact output index ordering. The four-face records rejected an
intermediate approach that still joined all newly encountered smooth face pairs.

## Running the diagnostic

### Radial sorting checked separately

The [12-case topology request](../tools/rhino_oracle/fixtures/mesh_radial_topology.json)
and [Rhino response](../tools/rhino_oracle/observations/mesh_radial_topology.json)
record raw-vertex membership, edge-face incidence, and edge order before/after
the public [`TopologyVertices.SortEdges`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.collections.meshtopologyvertexlist/sortedges)
call. McNeel documents non-manifold edges as boundaries for this sort.
The probe duplicates and disposes its mesh; it does not edit document objects.
It is Rhino-only and is not a native `Operation` variant.

The native `mesh::radial::tests` regression matches the flattened sorted edge
lists at every vertex in all twelve cases, including all face orders and partial
sharing. Python checks validate record incidence and membership against the
source faces. This isolated the earlier mismatch to the grouping/rebuilding
stage rather than the radial edge sorter.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/mesh_radial_topology.json --timeout 240
cargo test -p viboceros-geometry mesh::radial::
```

### Unweld parity regressions

All of these parity tests are enabled in the normal suite:

```sh
cargo test -p viboceros-oracle mesh_edit_replay_tests
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/mesh_unweld_nonmanifold.json --timeout 240
```

The tests compare the captured Rhino results exactly, without relabeling raw
indices. They establish the recorded cases, not universal compatibility across
arbitrary high-valence, mixed-face, or multiply non-manifold configurations.
