# Border duplication

[Command reference](commands/editing.md) · [Oracle](oracle.md)

`DupBorder` and `DupFaceBorder` now share border assembly in
`viboceros-command/src/border`, with topology traversal isolated in
`viboceros-geometry/src/brep/borders.rs`.

Curved chains become one native polycurve, not independent edges in a new group.
Each NURBS segment retains its controls and weights, with an affine change to
consecutive child parameter intervals. No endpoint averaging or fitting is
performed. Positive-weight curves passing the kernel's zero-tolerance line
predicate can simplify to polylines, including degree-elevated lines. Surface
polylines use chord-length parameters; mesh polylines count edges. Single-edge
closed borders retain their NURBS representation.

B-rep edge direction comes from the incident trim and face orientation, not
the stored edge curve alone. Single-face objects and explicit face selections
use trim order, independently of edge-table order; whole polysurfaces use the
naked edges in table order. Extraction excludes mated/seam edges using one
incidence pass. Topological chaining uses O(T + E log E) time and O(E)
auxiliary storage for T trims and E edges. Exact-location
welded mesh boundaries follow the first canonical boundary edge from smaller
to larger index independently of face winding, starting at its larger endpoint.

### Closed seams and disconnected output order

The public [OpenNURBS joining implementation](../third_party/opennurbs/opennurbs_curve.cpp)
distinguishes line/polyline joining from general curve joining. In an exact
two-valent linear boundary, endpoint joins are visited by the pair's larger
then smaller input index. The last connection closes the loop and leaves its
start at that vertex. A fixed rotation relative to the earliest edge is wrong
when the edge table is reordered. Curved loops instead start at their earliest
input segment.

Disconnected line chains are emitted in surviving fragment-slot order, not
smallest-edge order. A new pair creates a slot; attaching a single edge retains
it; merging two oriented fragments retains the upstream slot. The Rust
implementation tracks these slots with a size-balanced disjoint-set forest
and path compression, without copying growing chains. A separate literal
pairwise-concatenation reference tests 512 permutations, and a 12,000-edge
interleaved-cycle regression exercises scaling and isolation. The pinned
OpenNURBS source is public reference material, not proprietary Rhino code.

Both commands stage outputs before document mutation and retain sources.
`DupBorder OutputLayer=Input` copies attributes and existing groups; its
`Current` option creates fresh attributes. `DupFaceBorder` always creates fresh
attributes, changing only the chosen layer. Output selection is direct, so
shared groups cannot reselect the retained source or other group members.
Typed `DupBorder Faces=...` delegates per-face extraction with DupBorder's
attribute policy. Undo/redo and late-failure atomicity have independent tests.

## Rhino evidence

The 50 cases in [borders.json](../tools/rhino_oracle/fixtures/borders.json) and
36 additional [seam cases](../tools/rhino_oracle/fixtures/border_seams.json) were
run through the actual Rhino 8.32.26160.13001 commands in owned private Xvfb
sessions. [Original observations](../tools/rhino_oracle/observations/borders.json)
and [seam observations](../tools/rhino_oracle/observations/border_seams.json)
retain creation order, curve type, native and child domains, NURBS controls,
weights, stored knots, 33 native-parameter samples, layers, colors, names,
groups, selection, and source retention.

The matrix includes rational and unclamped patches, degree-elevated planes,
closed-U surfaces, singular cones, cylinder walls, trimmed annuli and reversed
faces, explicit face order, both output layers and selection routes, unwelded
meshes, mesh holes, disconnected panels, and adjacent joined walls. Additional
cases permute edge tables, translate boxes, and cover concave L-shaped borders,
annuli, warped quad patches, reversed faces, and reordered curved single faces.

All 86 cases match every recorded field at 1e-10 absolute/relative tolerance.
The eight polysurface start-point discrepancies recorded in the first
checkpoint are resolved, without normalizing seams, directions, or output order.
Independent tests verify all source naked edges occur exactly once and every
recorded polysurface sample lies on a source naked edge. See the
[comparison report](borders-comparison.json) for per-case results.

At the 2026-09-20 checkpoint, the maximum error among full matches is
4.89e-15. The report records both fixture and observation hashes and the
release executable SHA-256. Validation passed: 2,658 Rust tests (18 existing
ignored tests), 206 Python tests, Clippy with warnings denied, and rustdoc with
warnings denied.

B-rep comparisons use the same Viboceros-written 3DM source in both engines.
Primitive factories assign different face numbers and orientations, so separate
box/extrusion factories would not be a valid command comparison. Export is
roundtrip-checked before use, temporary artifacts are exclusively created,
and existing paths are never overwritten. Standalone Rhino runs of B-rep
fixtures require such an artifact; `compare` prepares it automatically.

OpenNURBS omits the two unused exterior knots of a full NURBS knot vector.
Both recorders expand those absent entries by repeating their adjacent knot;
every stored knot is compared. There is no geometric, seam, direction, output
ordering, or control-point normalization. Timings are zero: these command
checks make no performance claim against Rhino.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/borders.json
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/border_seams.json
cargo test --release -p viboceros-oracle border::tests
```

## Remaining limits

Branching B-rep boundary graphs that cannot form one chain are rejected rather
than silently dropping edges; tolerance-gap healing is intentionally not
implemented. Output assembly uses the native 65,536-segment polycurve limit
and a one-million-object command limit. Native output-layer choices are
explicit/default Current rather than
Rhino's remembered options. Hatches, SubD borders, and mesh/SubD face-subset
selection are not implemented. Mixed curved/linear disconnected polysurfaces
and orientation-conflicting boundary graphs need broader oracle coverage.
These tests do not establish general NURBS or command compatibility beyond
the recorded cases.
