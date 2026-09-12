# Known non-manifold angle-unweld mismatch

[Oracle overview](oracle.md) · [Mesh topology](mesh-topology.md)

Status: **unresolved**. This concerns angle-based `Mesh.Unweld`, not selected-edge
`Mesh.UnweldEdge`. The native implementation joins incident faces that already
share a raw endpoint and have a sufficiently small normal angle. Live Rhino
8.32.26160.13001 measurements show that this all-pairs connectivity model does not
reproduce non-manifold angle unwelding.

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

For the 36-case matrix, label the original triangles `0`, `1` (the smooth pair),
and `2` (the perpendicular face). At endpoint A, Rhino preserves sharing between
`0` and `1` for face orders `021`, `201`, and `210`, but separates them for `012`,
`102`, and `120`. Endpoint B is fully separated in every measured case. Existing
partial sharing and `ModifyNormals` do not change those results in this matrix.
This is an observation about these inputs, not a general replacement algorithm.
Native replay differs in all 36 matrix cases and two of the three threshold
cases; the zero-angle partial-sharing case matches exactly.

The recorded vertex counts, coordinates, face indices, and ordering are exact.
The mismatch includes different vertex sharing, not just renumbering or numerical
roundoff. Increasing a comparison epsilon cannot resolve it. Stored normals are
not part of these records.

## Running the diagnostic

The two native parity tests are explicitly ignored while this gap remains:

```sh
cargo test -p viboceros-oracle nonmanifold_angle_unweld -- --ignored
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/mesh_unweld_nonmanifold.json --timeout 240
```

The tests are expected to fail with the current implementation. Do not replace
their expected results with native output or interpret the normal suite's green
status as proof of non-manifold angle-unweld compatibility. The next investigation
should compare public Rhino topology/radial traversal outputs and sequential
vertex rebuilding against these measurements before changing connectivity rules.
