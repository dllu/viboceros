# Shared-edge B-rep assembly

[Architecture](architecture.md) · [Join command status](commands/join.md)

`Brep::try_join_edge_pairs` assembles explicitly paired, complete naked edges.
`join_breps` adds automatic boundary search, straight partial-overlap splitting,
and connected-output extraction. The surface/polysurface `Join` and `JoinCopy`
adapters use this automatic path; [command policy and remaining Rhino
differences](commands/join.md#surfaces-and-polysurfaces) are documented separately.

Combine sources with `Brep::try_combine`, split partial boundaries with
`try_split_edges_at_parameters`, then supply tuples of
`(retained_edge, removed_edge, opposite_curve_directions)`. Indices refer to the
combined, already-split input. Each edge must be naked and occur in only one
pair. No source is mutated, including on failure.

## Whole-curve acceptance

The common-basis certificate requires equal degree and control count, exactly
affine-equivalent full knot vectors, and exactly proportional, sign-coherent
weights. Reversed directions, shifted/scaled domains, and a common negative
weight scale are supported. Each corresponding Euclidean control-point
distance must be no greater than the explicit absolute join distance.

These conditions give both curves the same nonnegative rational basis functions
`R_i(t)`, whose sum is one. Their difference is `sum R_i(t) (P_i - Q_i)`, so the
largest control-point distance bounds the entire curve, not just samples or
endpoints. A second exact certificate covers different degrees, knot refinements
and positive rational weights through degree 16. It aligns the union of the
normalized knot spans, extracts homogeneous Bernstein controls with exact
polar-form evaluation, and forms the rational difference
`(Na Wb - Nb Wa) / (Wa Wb)`. Its positive denominator makes the projected control
hull a whole-span bound. Midpoint subdivision can resolve inconclusive hulls;
an out-of-range interior control is not itself evidence of an excessive gap.
Unclamped domains and either common weight sign are supported. All algebra uses
the exact input binary64 values, without rounded knot insertion or sampling.

When affine correspondence fails, endpoint derivatives can propose up to two
positive projective maps `F(t) = c*t/(1-t+c*t)`. Each map is an increasing
bijection of the complete domains. Knots are aligned using its exact inverse,
and each extracted span is exactly composed with its local projective map before
the same Bernstein difference proof. Stationary, equal-degree endpoints can
instead suggest a map through adjacent weight ratios. A proposal never bypasses
the proof. This supports changed rational parameter speeds without changing the
retained curves or surfaces; see the [projective audit](join-projective-correspondence.md).

These are sufficient, not necessary, tests. A hull still inconclusive after
16 subdivision levels remains unmatched; exhausting the shared work budget is
an atomic error. General non-projective parameter correspondence and curved partial
overlaps remain unsupported. The [curved-certificate audit](join-curved-certificates.md)
records both newly matched cases and remaining differences.

A separate straight-locus certificate accepts exactly collinear, monotonically
ordered control points with sign-coherent nonzero weights and clamped ends.
Full-order interior knots are conservatively rejected: collinear controls alone
do not exclude discontinuities or missing intervals. Continuity and monotonicity
prove the complete oriented segment locus, allowing different degrees, knots,
and rational speeds. Corresponding endpoint distances then bound the entire
segment. Exact determinant tests reject even subnormal off-line controls.

Knot-ratio and weight-proportion predicates compare exact binary64 product sums.
Distance acceptance likewise compares exact squared distances with the squared
threshold, without rounding coordinate differences first. Fixed 66-limb
accumulators cover finite binary64 coordinates, products, and carries. A normal
`hypot` supplies only a candidate upper bound, subsequently checked exactly.
Neither distant translations, source component tolerances, nor document-relative
tolerance widen the join distance. Zero distance is permitted.

## Topology and uncertainty

The retained spatial edge, every underlying surface, and every UV trim remain
unchanged, including their domains, controls, and weights. Surviving edge and
vertex records retain source order. Vertex unions retain the lowest source
index, and every member must be within the join distance of that representative;
chains cannot silently move a remote endpoint beyond the distance.

Edge and vertex tolerances include the original uncertainty and an outward-rounded
displacement bound. For nonzero displacement, original uncertainty includes the
model-tolerance floor under which the source was validated. This conservative
propagation is distinct from Rhino's measured/rebuilt component tolerances.

A face-adjacency parity graph reconciles orientation while holding the first
face of each component fixed. Existing seam uses participate in the graph.
Contradictory cycles, edges with more than two uses, repeated pairs, and attempts
to reuse an already-mated edge fail. Paired trims become Mated or, for two uses
in the same loop, Seam. Singular trims retain their parameter curves and update
only their vertex references. The complete result passes normal B-rep validation.

The primitive does not infer outward normals, classify cavities, reject
zero-volume double sheets, deduplicate unrelated coincident vertices, or split
disconnected topology into separate objects. An empty pair list preserves the
source exactly after validation. Pair processing is limited to 100,000 pairs
and four million input controls, checked before curve matching. Matching has a
16-million-unit work budget, shared with automatic discovery when used there;
assembly does not reset that budget. Topology work
is linear apart from disjoint-set operations; final geometric validation has
the existing B-rep validator's cost and sampled trim/edge correspondence limits.

## Automatic discovery and components

`join_breps(&[&Brep], distance, tolerance)` returns connected B-reps with sorted
source indices and joined-edge counts. A source can contribute to several
outputs. Only boundaries between original input objects are considered; unmatched
boundaries within one input are not implicitly sewn. Existing closed inputs
remain unchanged, including inward orientation.

A balanced, widest-axis AABB tree searches naked-edge control hulls without
tolerance-scaled coordinate grids. The narrow-phase certificate only accepts
sign-coherent rational bases, for which these hulls are conservative. Exact
straight partial overlaps are planned in a dominant coordinate. Bounded binary
search over the finite parameter lattice finds cuts; actual trimmed curve
representations must pass the whole-curve certificate before cuts are proposed.
For interleaved overlaps, original tips within the exact Euclidean Join radius
are treated as a vertex contact without cutting tiny crossing pieces. Complete
short edges, including contained intervals, remain eligible; see the
[short-overlap audit](join-short-overlaps.md).
Subdivision keeps each source's new edges in its local table, but original
vertices from all sources precede inserted vertices. Endpoint unions therefore
retain genuine source endpoints ahead of rounded cut evaluations. A complete
boundary's spatial curve precedes a cut one; if both were cut, the later source's
piece survives. Surfaces and UV loci are unchanged.

Mutual unique candidates join first. Three or more competing boundaries remain
unjoined unless other edges establish a component in which a pair is uniquely
resolvable. That deferred closure retains the later edge. Every certified
candidate participates, including farther candidates within the join distance;
nearest-first greedy pairing is not used. Existing input-internal mated edges
remain intact. Each piece is paired once, and orientation conflicts fail
atomically. Overlapping proposals can still introduce subdivisions whose
candidates remain unjoined. See the [boundary-matching audit](join-boundary-matching.md).

`join_breps_with_report` also returns sorted original-source candidate pairs.
The command uses these contacts to admit incremental picks while reconsidering
the original accepted boundaries, without repeatedly sewing a temporary result.
Contact does not itself imply a final topological join.

Automatic assembly also [adjusts nearby spatial boundaries](join-gap-rebuilding.md),
independently of this topological decision. Endpoint clusters use incident-edge
weighted means; affected clamped curves preserve their chord profiles, weights,
and knots. All incident edges participate, including existing mated edges.
Contacts are processed in exact point-pair distance order. A maintained
edge-adjacency graph forbids any cluster from containing both distinct ends of
an existing edge. This protects tiny features; it does not resolve the remaining
general order-dependent clustering differences.
The explicit `try_join_edge_pairs` primitive remains geometry-preserving.
Natural clamped rows/columns and exactly evaluated tensor-product isocurves supply
lifted-boundary certificates for updated uncertainty. Interior isocurves and
unclamped fixed directions retain their exact homogeneous controls, without
rounding a derived spatial curve. Oriented partial intervals normalize knots
exactly; endpoint certificates cover both sides of full-order discontinuities.
See the [partial-boundary](join-trim-certificates.md) and
[general-isocurve audits](join-isocurve-certificates.md). Rational Bernstein subdivision can tighten curved-gap
bounds across different rational bases without sampled acceptance. Unsupported certificates retain
conservative propagation, not suppressed tolerances. Clusters or curve changes
beyond the join distance fail atomically; nonclamped incident curves retain the
unadjusted policy for the entire assembly. The work budget covers this refinement.

Connected extraction visits face/edge references and compacts each component's
tables without rescanning the entire assembly per output. Newly joined closed
shells use [exact spatial orientation](solid-orientation.md), independent of
whether their volume fits in `f64`. Unsupported witnesses retain a numerical
signed-volume fallback for that single connected component; it is not a validity
certificate. Untouched components keep their original sense. This is not cavity
classification or a Boolean operation. The [scale audit](join-orientation.md)
retains overflow/underflow fixes and explicit large-coordinate Rhino differences.
Limits are 10,000 sources, 200,000 naked edges, one million candidate pairs, and
16 million charged work units, plus the subdivision/explicit-assembly limits.
These bound search work, not every validation or high-degree geometry cost.
Orientation has its own 262,144-unit exact-work budget per queried output;
that budget does not bound rational-integer bit complexity or fallback integration.
Tests cover 1,000 disconnected sheets, all 64 box-face orientation masks,
one-to-many straight overlaps, independent rational speeds (including partial
cuts on negative and large shifted parameter domains), immutable sources,
and randomized tree searches against an all-pairs reference at varied scales.

## Oracle boundary and regression evidence

Successfully joined components also run [certified redundant-edge cleanup](join-edge-cleanup.md).
`Brep::try_merge_all_edges` updates all incident trim rings at smooth valence-two
vertices, retaining branches and singular vertices. Whole-curve certificates
bound accumulated spatial changes and require exact UV preservation. The
explicit pair-assembly primitive and untouched components do not perform cleanup.

The `brep_join` probe uses public
[RhinoCommon JoinBreps](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/joinbreps)
on owned geometry in a private Xvfb session, without inserting document objects.
Native inputs are exported individually to 3DM and roundtrip-checked before
Rhino reads the exact same files. The native side uses fixture-supplied edge
pairs; Rhino discovers its own matches. This is evidence for the recorded
connected assemblies, not interactive Join. Separate actual-command records
exercise automatic matching and document behavior; see the [surface command
audit](commands/join.md#surfaces-and-polysurfaces).

Records retain raw spatial edge definitions and order, vertices, component
tolerances, oriented incidence, face senses, full underlying surfaces, every
trim-curve definition, face areas, and signed volume. Face incidence records
use the same stable sorting as the Cap probe; no edges, vertices, surfaces, trim
definitions, or differing output policies are normalized away. The probe checks
that Rhino does not mutate its inputs and disposes all owned models and geometry
on success and failure. Geometry extraction and calls are untimed; these cases
do not establish a kernel performance ratio.

The [matching fixture](../tools/rhino_oracle/fixtures/brep_join_edges.json) and
[raw observations](../tools/rhino_oracle/observations/brep_join_edges.json) cover
28 open and closed assemblies, reordered sources, mixed input orientations,
opposite edge directions, pre-split partial overlaps, rational boundaries,
different parameter scales and common weight scales. All recorded fields match
at absolute epsilon `1e-10` and relative epsilon `1e-12`; maximum numeric residual
is below `2.04e-12`. The original 22 matching box-sheet records have zero residual.
The separate [policy fixture](../tools/rhino_oracle/fixtures/brep_join_policies.json)
and [raw observations](../tools/rhino_oracle/observations/brep_join_policies.json)
retain eight low-level policy differences: Rhino makes newly closed boxes
outward, chooses a canonical orientation for the recorded coincident double
sheets, and can move gap-joined vertices and spatial edges toward midpoints.
The native assembly primitive keeps its first face sense and retained geometry.
At the recorded `0.001` gap/tolerance boundary, Rhino returns two unjoined sheets
but moves one endpoint in both outputs; the native explicit pair joins within
its exact distance predicate. Neither side mutates its input objects.

A [large-parameter-origin fixture](../tools/rhino_oracle/fixtures/brep_join_parameter_origin.json)
and [raw observation](../tools/rhino_oracle/observations/brep_join_parameter_origin.json)
retain a separate integration difference. The profile is
`C(t) = (t+2t², 2t(1-t), 0)/(1-t+t²)` on `[0,1]`, extruded by height 3.
Independent 70-digit integration gives area
`10.22183381267366196861971010606321278774687575928505900910084421931395`.
The native area differs by less than `2e-14`; Rhino's area is approximately
`3.81e-9` low when the equivalent surface U domain starts at `1e9`.
All other recorded fields still pass the ordinary comparison tolerance.

Independent tests cover all 64 orientation masks of a six-face box, pair order,
retaining a later edge slot, rational cylinders and cones, closed boundaries,
same-face seam restoration, singular vertices, nonuniform UV parameter speed,
pre-split partial overlaps, gap uncertainty, transitive-cluster rejection, and
failure atomicity. Exact-predicate tests compare against independent arbitrary-
precision rational arithmetic, including subnormal and overflowing squared
distances, translated coordinates, and nearly equal knots or weights.

```sh
cargo test --release -p viboceros-geometry brep::join_edges
cargo test --release -p viboceros-oracle brep_join
python3 -m unittest tools.rhino_oracle.test_brep_join_probe
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/brep_join_edges.json --timeout 240 --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

Comparing `brep_join_policies.json` or `brep_join_parameter_origin.json` at the
ordinary epsilon is expected to report the documented differences. Replay tests
assert those raw differences explicitly.
