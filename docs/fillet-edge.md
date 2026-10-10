# Native circular edge fillets

The optional SMLib adapter now fillets original Rust edge identities on a
freshly imported temporary solid. Native import retains the exact original
edge-to-pointer map; mutable operations discard that map before changing
topology. Geometry and document ownership remain in Rust.

The high-level upstream default cross-section tolerance produced a detectable
volume discrepancy on a radius-one box edge. The bridge uses the public
fillet executive with explicit approximation/cross-section tolerances and enables
global merging after parameter setup. Result transfer retains exact native
NURBS definitions. Near-boundary UV coordinates only propose pole connectors;
the whole connector image must continuously certify at the shared spatial vertex.

The oversized all-edge box fixture exposed a second issue: native success and
manifold topology alone accepted a geometrically interfering result that Rhino
rejects. The adapter now checks nonadjacent face intersections and permits
point contacts only at shared topological vertices. Native error status is also
checked. The result is admitted to the document only after Rust B-rep validation.

## Exercise

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test -p viboceros-smlib \
  --features native --test fillet_edges --release
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run -p viboceros-smlib \
  --features native --example fillet_edges --release -- \
  /tmp/viboceros-fillet-edges-owned
cargo run --release --features native-smlib
```

The example writes editable 3DM/STEP fixtures for a single physical box edge,
three edges meeting at one corner and all twelve box edges. Tests exercise every
original box edge, source preservation, duplicate indices, invalid input,
oversized-radius rejection, closed topology and analytic rounded-box boundaries.
Volume tolerance is derived from modelling distance tolerance and boundary area;
it is separate from the 1e-8 boundary-coordinate check.

The command and viewport tests cover staging, radius changes, cancellation,
component picking, metadata preservation, stale geometry and undo/redo. The
licensed Rhino reference uses constant-radius `RollingBall`, no setback, model
tolerance 1e-9 and angular tolerance 1e-10. API output and command interaction
are separate qualification scopes; full native UI parity remains unproven.

General fillets, radius-law editing, open inputs, scale extremes, adjacency-aware
self-intersection certification and larger output arrangements remain further
work. See [command use and current limits](commands/fillet-edge.md).

## Interchange and captured limits

All three editable 3DM exports read as valid closed B-reps in Rhino. Imported
spatial edge and lifted trim samples agree with the independent radius-one
boundaries within 1e-8. The native SDK's spatial curves also pass; its lifted
trims for the single-edge and three-edge corner fixtures deviate by about
4.55e-7 and are explicitly recorded as unqualified at that epsilon. Native
volume-query discrepancies remain separate diagnostics.

OpenNURBS derives approximate isocurve flags from parameter curves. Archive
output recomputes those flags with the native library; Rust import restores
exactly compatible side/interior classifications without changing geometry.
The rounded-box regression includes a complete editable 3DM round trip.
The retained STEP files are editable surface models; this capture does not
qualify them as independent STEP material solids.

[Comparison](fillet-edge-comparison.json), [SDK reference](fillet-edge-native-reference.json),
[3DM import capture](fillet-edge-interchange-reference.json),
[exact files and diagnostic logs](fillet-edge-diagnostics/) and
[source/check provenance](fillet-edge-provenance.json) retain the evidence.
