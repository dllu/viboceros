# Selected-edge merging

[Edge cleanup](join-edge-cleanup.md) · [MergeAllEdges](commands/merge-edges.md)

`Brep::try_merge_edge(edge, angle_radians, tolerance)` recursively coalesces the
mergeable chain containing a source edge. It uses the same certified spatial/UV
concatenation, uncertainty propagation, valence-two checks and work budget as
`try_merge_all_edges`. Only that chain is visited; other split chains and
straight-edge representations remain untouched. Surfaces are unchanged. This
kernel API does not perform replacement-time face splitting.

Invalid indices, invalid angles and exhausted budgets are errors without source
mutation. An unmergeable seed returns an equal B-rep. Surviving source edges keep
their relative order, and the merged edge follows them. Vertex/edge indices can
change when unused entries are compacted. The common kernel's degree-16 and
exact-UV certification limits still apply.

## Public API evidence

[45 fixtures](../tools/rhino_oracle/fixtures/brep_merge_edge.json),
[raw observations](../tools/rhino_oracle/observations/brep_merge_edge.json),
[comparison](merge-edge-comparison.json) and [provenance](merge-edge-provenance.json)
cover nine shared, round-trip-checked 3DM sources in a private-Xvfb Rhino
8.32.26160.13001 session. Sources include forward/reversed boxes, polynomial and
rational profiles, unclamped and multispan surfaces, and linear/weighted seams
with both weight signs. Seeds at either end or inside a four-piece chain merge
three edges; an unrelated seed leaves the original geometry intact.

The probe calls the public
[`BrepEdgeList.MergeEdge`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.collections.brepedgelist/mergeedge)
and `Compact`, then records a fresh deep copy. This avoids stale managed trim
wrappers after compaction; no geometry is rebuilt, fitted or normalized. The
independent input is checked unchanged. The measured API return includes the
seed: it is `4` for three removed edges and `1` for a no-op. The oracle records
that value separately from the actual edge-count reduction.

**40/45 complete raw records match**, at absolute epsilon `1e-9` and relative
epsilon `1e-10`. The other five differ only in two unused outer surface knots
omitted by 3DM, already before merging. Regression tests assert those exact
differences and compare every remaining field, including all curve definitions,
UV intervals, component order, uncertainty and integrals. No sampled locus check
substitutes for the kernel's preservation certificates.

These are correctness probes, not timings. They do not establish arbitrary-input
Rhino parity, a performance ratio, or the interactive command's semantics.

## Command status

Interactive `MergeEdge` is not yet implemented. Its Rhino-only diagnostic uses
the separate `merge_edge_command` operation, so native requests cannot silently
fall back to `MergeAllEdges`. The initial six preselected-component probes all
reached Rhino's edge-pick prompt and canceled without edits. Six typed world-point
attempts also failed to select an edge. Both batches retain
[requests and raw responses](../tools/rhino_oracle/diagnostics/merge_edge/),
including command histories; they are not successful geometry-command comparisons.
Preselection or a coordinate macro is not an accepted substitute for a command-time
edge pick in these probes. Actual component picking, repeated edits, selection
and history need their own observations and UI integration. Diagnostic requests
require separately exported shared artifacts; native replay is intentionally
unsupported until the command exists.

## Verification

Native tests exercise all seed positions, preservation of unrelated split
chains, open/solid topology, closed circles and seams, invalid inputs, work
limits and no-op identity. Oracle tests replay the full retained API records.
Python tests check validation before host access, disposal on each failure path,
and unique owned artifacts even when callers reuse the same Python dictionaries.

```sh
cargo test --release -p viboceros-geometry selected_tests
cargo test --release -p viboceros-oracle merge_edge
python3 -m unittest tools.rhino_oracle.test_merge_edge_probe
```

Verification checkpoint: 2,941 release-mode Rust tests, seven offscreen GPU
tests, 243 Python tests, formatting, and Clippy/Rustdoc with warnings denied.
