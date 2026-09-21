# Short overlaps and endpoint protection in surface Join

[Kernel](brep-edge-joining.md) · [Command](commands/join.md) ·
[Complete replay report](join-short-overlaps-comparison.json)

This page records the original short-overlap audit. The later
[partial-boundary certificate](join-trim-certificates.md) resolves its eighteen
uncertainty-only differences, bringing this fixture to 96 full matches. The
original report remains historical evidence.

Automatic Join now distinguishes a short crossing overlap from a complete short
edge. It also prevents boundary adjustment from clustering the distinct ends of
an existing edge. The four orientation failures from the
[projective-correspondence audit](join-projective-correspondence.md) now fully
match Rhino, including the seven-edge topology and rebuilt spatial curves.

## Rules and safeguards

For two straight segments with interleaved parameter-independent endpoints,
each segment contributes one original tip inside the overlap. If the exact
Euclidean tip distance is within the Join radius, discovery leaves this as an
endpoint contact rather than proposing two short cut edges. An entire edge
contained in the other interval remains eligible, however short it is. This is
not a minimum edge length or projected-overlap cutoff. All proposed cuts still
require complete retained-curve certificates before subdivision.

Endpoint contacts are canonicalized, deduplicated, then ordered by exact
independent pair distance, with source vertex indices breaking ties. Union-find
maintains the edge-adjacency graph between live groups. A union is forbidden
whenever it would place both distinct ends of any existing edge in one group.
Exact corresponding tips therefore precede nearby opposite ends of a short edge.
This includes original, inserted and already-mated edges. It does not establish
Rhino's general ordered corner-clustering policy or guarantee that different
groups can never have coincident means. Existing movement limits, uncertainty
certificates and final B-rep validation remain mandatory.

The distance ordering uses exact sums of binary64 products, not rounded squared
norms. Independent rational-reference tests cover 512 random finite cases,
subnormals, overflow-scale distances and cancellation. Sorting and small-to-large
adjacency updates charge the shared Join work budget. Sources remain unchanged,
including on budget exhaustion. Candidate **edge mating** remains mutual-unique,
not greedy nearest-edge pairing.

Construction also now requires seam endpoints to agree with its independently
built corner topology. Approximate surface closure can otherwise misclassify a
short open strip far from the world origin and produce incompatible seam vertex
references. Both parameter directions are tested. Join tests preserve complete
edges down to `1e-6`, including geometry translated to `1e9`, without moving their
endpoints or changing surfaces and UV curves. The surface closure classifier's
coordinate-scale allowance itself is unchanged.

## Identical inputs and actual commands

The [102 portable requests](../tools/rhino_oracle/fixtures/join_short_overlaps.json)
and [102 raw observations](../tools/rhino_oracle/observations/join_short_overlaps.json)
come from Rhino 8.32.26160.13001, in two sequential owned private-Xvfb sessions.
Each source was exported to an owned 3DM, roundtrip-checked and checked again
after Rhino document insertion. The report hashes all 102 source artifacts and
both shared requests/raw batches. No proprietary implementation was inspected.

The main 84 records cover seven lengths from `0.0001` to `0.01`, both selection
modes, opposed flat and projectively related curved sheets, crossing intervals,
contained and prefix intervals, and complete short shared edges. Eighteen more
records cover offsets, rotations and reversed parameter directions. Document
absolute tolerance is `0.001`; source construction uses fixture tolerance `1e-9`.
The native harness previously used the document override to reconstruct source
B-reps, collapsing short features before Join. It now keeps those two tolerances
separate, matching the shared-artifact protocol. A regression verifies unchanged
short input geometry at different document Join tolerances.

With that harness correction applied to both sides of the native comparison,
the main batch improves from **28 to 70 full matches**, resolving all **14 native
execution failures**. The intermediate baseline executable hash was not captured;
its response hash and exact baseline source description are retained. The older
uncorrected-harness baseline is not presented as an identical-input comparison.

All 102 discovery records remain in the report at `1e-10` absolute / `1e-12`
relative comparison epsilon: **78 full matches, no native errors, 24 differences**.

- Eighteen differ only in edge/vertex uncertainty. ULP-scale split rounding can
  trigger native's conservative model-tolerance floor, approximately `0.001`,
  where Rhino records zero. Complete partial-trim image certificates are still
  needed; safe bounds are not zeroed to match the reference.
- Four offset cases join with eight vertices, nine edges and one mate, but
  native averages tips that Rhino preserves. One also subdivides a different
  incident edge and uses a different second-face sense. Spatial curves,
  uncertainty and some trim/incidence data differ.
- One offset preselection case leaves two unjoined sheets in both engines;
  Rhino subdivides and adjusts one boundary, leaving five edges where native
  retains four. This is a discovery/rebuilding difference, not a joined edge.
- One reversed, rotated short-overlap case differs only in the raw vertex table
  permutation and corresponding incidence/tolerance ownership. A diagnostic test
  verifies the remap, but the raw comparison remains a mismatch.

Across thirteen surface-command fixtures, **633 of 804** cases fully match,
**32** retain native execution errors, and **139** have other differences. Of the
previous 702 rows, only the four resolved orientation errors change; all other
698 flags, residuals and errors are identical. The new record retains every
old discrepancy. Harness timings do not establish kernel speed parity.

```sh
cargo test --release -p viboceros-geometry short_edges
cargo test --release -p viboceros-oracle short_overlap_discovery
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_short_overlaps.json --observations tools/rhino_oracle/observations/join_short_overlaps.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

Replay intentionally exits 1 for the 24 retained differences. General ordered
clustering, partial-trim uncertainty and offset cut ownership remain open work.

Verification on the tree including the viewport-cache merge passed 2,862 Rust
tests (22 opt-in tests ignored by the regular run), seven separately run offscreen
GPU tests, and 232 Python tests. Formatting, Clippy and Rustdoc passed with warnings
denied where applicable. README stays concise at 43 lines.
