# Mesh topology and editing

[Architecture](architecture.md) · [Mesh commands](commands/meshes.md) · [Numerical robustness](numerical-robustness.md)

The kernel distinguishes stored raw vertices from exact-location topology
vertices. Coincident raw vertices can remain separate across unwelded seams.
Stored faces are triangles or quads; triangulated facets are a separate view.
[`mesh/union_find`](../crates/viboceros-geometry/src/mesh/union_find.rs) shares
iterative path compression across face grouping and raw-vertex merging. Face
unions use rank; raw-vertex unions explicitly retain the earliest or latest index.
An independent label-partition reference checks all 69,905 union sequences of
length zero through four on four indices, including repeated/self unions. Long
chains in both directions verify complete compression without recursion.
Rank-based unions are checked after every step of all four-operation sequences
on four indices: component equivalence, unchanged ranks for redundant unions,
strictly increasing parent ranks, and the `component_size >= 2^rank` bound.
Balanced trees through 16,384 indices attain that bound before compression.
Command selection, source retention, groups, and history belong to the command
and document crates, not these geometry modules.

## Connectivity and components

[`mesh/components`](../crates/viboceros-geometry/src/mesh/components.rs) owns
disjoint splitting and mesh explosion. Face-indexed grouping preserves first-face
order without per-face tree lookups. A reusable vertex-remap array avoids
initializing a source-sized array for each component.

`try_explode_pieces` and `try_disjoint_pieces` check component limits before
geometry remapping. Grouping stops at the first over-budget root, before allocating
that component's face list. Commands pass their remaining output budgets; the
unbounded APIs share the implementation. Topology and root-map storage still
scale with the source, independently of the output limit.

Explosion streams edge incidences. The unwelded-edge predicate needs no heap
allocation for up to two incident faces and uses sets for non-manifold cases.
Logical-boundary and face-angle filters stream incident-face pairs too. Break
angles are inclusive; face-angle interval bounds are strict.
Selected-edge welding streams endpoint pairs without collecting edge uses or
per-endpoint sets. Index-preserving unions retain the earliest raw vertex even
when incidence order differs or raw indices repeat; unrelated coincident fans
remain separate.

## Normals and area

[`mesh/normals`](../crates/viboceros-geometry/src/mesh/normals.rs) shares direction
calculation between `face_normal` (a triangulated-facet index) and
`polygon_face_normals` (one result per original polygon). Triangle edges or quad
diagonals are normalized before crossing, avoiding overflow or underflow caused
solely by squaring their scale. A nonzero normal is still required. Warped quads
use their diagonal cross product, not an unweighted average of unit facet normals.

Mesh area uses a compensated sum of triangulated facet areas. Its ordinary
cross-product path falls back to an exact binary accumulator when a full cross
product or its magnitude overflows. The fallback applies the half factor before
rounding components. Tests distinguish representable areas from genuine overflow
and retain the smallest positive binary64 area.

Extreme-scale tests are native numerical checks, not claims of Rhino parity at
those scales. The [normal oracle record](oracle.md) covers ordinary-scale mixed
faces and warped quads, including winding and Rhino's float normal storage.

## Edge collapse

[`mesh/edge_collapse`](../crates/viboceros-geometry/src/mesh/edge_collapse.rs) owns
endpoint merging, face reduction, compaction, and final validation. The common
point-midpoint routine preserves constant subnormal offsets and handles signed
rounding ties and large offsets. Surviving faces and compacted vertices retain
source ordering; independent seam components remain distinct.
Moved positions are computed while emitting retained vertices, without cloning
and updating the entire source vertex array first. Coincident raw peers outside
the selected edge still move to its midpoint but are not merged merely by position.
After surviving faces resolve their union roots, the parent table is reused for
retention marks and final vertex indices. This avoids separate used-vertex and
remap arrays. Faces are remapped in place, and parent/face/output-vertex buffers
reserve fallibly; topology construction retains its separate allocation policy.

Face reduction uses fixed-size index checks without per-quad collections.
Index-degenerate triangles disappear; a single collapsed quad side produces a
rotated triangle. Repeated diagonals and multiple collapsed sides are discarded
according to the reduction policy, followed by geometric validation.

## Edge splitting and storage

[`mesh/edge_split`](../crates/viboceros-geometry/src/mesh/edge_split.rs) owns
split-point evaluation, seam policy, generated-face ordering, and output sizing.
It checks `2 * incident_triangles + 3 * incident_quads` and fallibly reserves
candidate storage before generation. Endpoint-coincident candidates are removed
before final sizing. Final vertex counts must fit both the address space and
the full `u32` index range; vertex and face output buffers reserve fallibly.

Each `SplitTriangle` stores two raw indices, a typed split-point position, and
winding. This is smaller than three optional indices plus winding and uses no
reserved vertex-index sentinel. Canonical vertex insertion remains separate
from final face orientation.

Retained faces stream from the source for vertex marking and final remapping,
without an intermediate face vector. The remap table first holds 0/1 retention
marks, then output indices during source-order compaction, avoiding a separate
vertex flag array. The affected-face and remap buffers also reserve fallibly.
Topology construction has separate allocations: none of these checks is an
overall memory budget or a guarantee of recovery from every out-of-memory condition.

## Verification

- [Component tests](../crates/viboceros-geometry/src/mesh/components/tests.rs): every undirected graph through six faces in two union orders, with every component budget; endpoint-index assignments through four edge uses; indexed face-angle pair calculations.
- [Normal tests](../crates/viboceros-geometry/src/mesh/normals/tests.rs): analytic directions, cyclic/reversed winding, facet versus polygon indexing, and scales from `1e-200` through `1e200`.
- [Area tests](../crates/viboceros-geometry/src/mesh/area_tests.rs): intermediate versus final overflow and the smallest positive area.
- [Collapse tests](../crates/viboceros-geometry/src/mesh/edge_collapse/tests.rs): all 320 triangle/quad index patterns over four labels, plus source order, seams, validation, and midpoint cases.
- [Split tests](../crates/viboceros-geometry/src/mesh/edge_split/tests.rs): wide-integer sizing references, compact representation, endpoints, unaffected-face order, and a 450-case planar side/winding/seam matrix with independent signed-area determinants. Area preservation is not assumed for endpoint duplication or warped quads.
- [Rhino edge-edit replay](../crates/viboceros-oracle/src/mesh_edge_replay_tests.rs): exact acceptance, coordinates, indices, and ordering for recorded split and collapse cases. See [oracle details](oracle.md).

Run the focused kernel and recorded split checks with:

```sh
cargo test -p viboceros-geometry mesh::
cargo test -p viboceros-oracle mesh_edge_replay_tests
```
