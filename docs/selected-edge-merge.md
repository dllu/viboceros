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

`try_merge_edge_with_scope` additionally accepts `BrepEdgeMergeScope::Start`,
`End`, `Both` or `Chain`. Limited scopes visit each chosen original endpoint
once, merging at most one neighbor there; they never continue through newly
exposed endpoints. Ends follow the seed's spatial curve, including when that
curve is reversed. `Chain` is the existing recursive operation. All scopes
share the same preservation certificates, work budget and atomic validation.

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

Interactive [MergeEdge](commands/merge-edge.md) now uses real screen-space
component picks, ambiguity/neighbor choices and one transactional replacement.
Its separate `merge_edge_command` oracle operation runs that command natively,
never falling back to `MergeAllEdges`. The initial six preselected-component probes all
reached Rhino's edge-pick prompt and canceled without edits. Six typed world-point
attempts also failed to select an edge. Both batches retain
[requests and raw responses](../tools/rhino_oracle/diagnostics/merge_edge/),
including command histories; they are not successful geometry-command comparisons.
Preselection or a coordinate macro is not an accepted substitute for a command-time
edge pick in these probes. Live Rhino requests require separately exported shared
artifacts; native replay builds their documented source definitions.

### Real mouse picks and choices

[15 requests](../tools/rhino_oracle/diagnostics/merge_edge/mouse-request.json),
[complete responses](../tools/rhino_oracle/diagnostics/merge_edge/mouse-response.json)
and [provenance](merge-edge-mouse-provenance.json) now record actual owned-window
clicks on interior split edges of a box, a quadratic surface and a weighted seam.
The host resolves only newly owned process windows and sends one click per unique
worker marker. Dedicated, single-iteration requests and validated case IDs prevent
marker injection. Projected coordinates must lie inside the owned viewport.

The ordinary command opens an additional choice menu after the click. The
scriptable `-MergeEdge` exposes `EdgeA`, `EdgeB`, `Both` and `All` at the command
line. On these three sources they respectively remove one start neighbor, one
end neighbor, two immediate neighbors, or all three redundant chain edges.
Cancel at that choice leaves complete snapshots unchanged and creates no history
entry. Each successful choice ends the command, preserves identity, attributes
and group memberships, leaves the object unselected, and restores the complete
before/after snapshots with one Undo/Redo pair.

The scoped kernel comparison uses the **explicit edge-table permutations** listed
in provenance, then compares every geometry field at `1e-9` absolute / `1e-10`
relative epsilon. This distinguishes geometric agreement from raw ordering:
the kernel retains surviving source entries, while the document command reorders
some entries. No production geometry is reordered to fit these observations.
The native command now replays these 15 cases plus
[21 additional observations](merge-edge-command-provenance.json). The latter
establish endpoint choices, no-op/Enter cancellation, bounded angular candidate
cutoffs, preselection cleanup and preservation of a kinky face. UI tests exercise
actual pointer capture, overlap choices, cancellation and stale picks. They do
not establish general Rhino picking/occlusion equivalence or exact table ordering.

A later endpoint/no-op diagnostic timed out on Rhino's ambiguous-selection menu
near a box corner. Its [request](../tools/rhino_oracle/diagnostics/merge_edge/ambiguous-request.json)
and [timeout](../tools/rhino_oracle/diagnostics/merge_edge/ambiguous-timeout.txt)
are retained separately; that entire batch is excluded from the 15 observations.
The driver does not guess which item in a selection menu to accept.

## Verification

Native tests exercise limited and recursive scopes, reversed spatial ends,
all seed positions, preservation of unrelated split
chains, open/solid topology, closed circles and seams, invalid inputs, work
limits and no-op identity. Oracle tests replay the full retained API records.
Python tests check validation before host access, disposal on each failure path,
and unique owned artifacts even when callers reuse the same Python dictionaries.

```sh
cargo test --release -p viboceros-geometry selected_tests
cargo test --release -p viboceros-oracle merge_edge
python3 -m unittest tools.rhino_oracle.test_merge_edge_probe tools.rhino_oracle.test_merge_edges_probe
```

Verification checkpoint: 2,957 release-mode Rust tests, seven offscreen GPU
tests, 247 Python tests, formatting, and Clippy/Rustdoc with warnings denied.
