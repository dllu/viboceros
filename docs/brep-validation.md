# B-rep boundary validation

[Architecture](architecture.md) · [Surface morphing](surface-morphing.md) · [Oracle](oracle.md)

`Brep::try_new` validates shared vertices and edges, face-local trim loops,
orientations, isoparametric flags, and geometric boundary correspondence.
Topology checks live in `brep/validate`; the geometric correspondence checks are
separate in `brep/validate/boundary`.

Matching endpoints alone are insufficient. A bowed spatial edge can share the
correct vertices while leaving its face, and a bowed UV trim can share the right
endpoints while leaving its spatial edge. Validation now samples each nonempty
knot span on uniform and cosine-spaced grids and checks both directions:

- The trim evaluated on its surface must lie on the shared 3D edge.
- The shared 3D edge must lie on that evaluated trim. This catches extra edge
  excursions even when every trim point lies somewhere on the edge.
- A singular trim must remain at its collapsed model-space vertex throughout
  the sampled interior, not just at its ends.

Equal normalized parameters, adjusted for reversed edge use, are only a fast
candidate for correspondence. If their points differ, validation searches for
geometric coincidence. Edge-to-trim search operates on the composed curve
`S(u(t), v(t))`, using its analytic chain-rule tangent and bounded multistart,
backtracked refinement. It does not require equal edge and p-curve domains,
degrees, knots, or parameter speeds. Tests include a quadratic-speed trim paired
with a linear edge, including reversed edge use.

Full-order interior knots also receive exact left/right limit checks. Positional
jumps are rejected without relying on neighboring floating-point samples;
coincident limits remain valid even when the control structure is discontinuous.
The p-curve checks reuse the stable rational evaluator through a temporary
`(u,v,0)` representation, then evaluate the surface to obtain model-space points.

Spatial edge correspondence uses the larger of the document's absolute tolerance
and the edge's recorded model-space tolerance. Singular correspondence uses the
vertex's model-space tolerance. UV trim tolerances do not enlarge these spatial
allowances. Edge jump distances are Euclidean, not coordinate-box comparisons.
The checks never increase a component tolerance to conceal a measured mismatch.

Edge uses are indexed once by shared-edge index, replacing repeated scans of the
entire trim table during topology validation.

## Incidence queries

`brep/incidence` implements `is_manifold`, `is_closed`, and `is_solid` with one
trim traversal and an edge-count table. Each call costs O(trims + edges), rather
than rescanning all trims for each edge. Two bytes per edge hold oriented counts
saturated at three; exact counts above two are unnecessary for these predicates.
Singular trims without an edge are ignored, and face reversal participates in
the orientation test. Solidity retains the existing rule of exactly one use in
each direction, not an additional geometric self-intersection proof.

`edge_use_count` still returns an exact count for a requested edge, using an
allocation-free traversal. It returns `None` for an invalid edge index.
For whole-model inspection, `edge_use_counts` returns exact counts in edge-index
order using O(trims + edges) time and one `usize` per edge. `DupBorder` and the
B-rep meshing oracle use this bulk query to find naked edges without repeated
trim scans; their edge ordering and boundary geometry are unchanged.
Regression tests compare all queries with explicit use enumeration across open,
closed, non-manifold, reversed, singular-trim, and high-valence cases. This is an
algorithmic complexity improvement; no Rhino performance ratio is claimed.

## Evidence and limits

Three regressions first demonstrated that the old constructor accepted bowed
edges, bowed trims, and extra edge excursions. They now fail construction as
intended. Further tests cover singular trims, knot jumps, coincident full-order
limits, independent parameterization, reversed edge use, and tolerance units.

These are finite geometric checks, not a continuous Hausdorff-distance proof or
a complete validity classifier for arbitrary rational B-reps. Unsampled features
may escape them, and ill-conditioned correspondence searches can reject a valid
model. They do not prove absence of face self-intersection or interior singularities.
Existing loop-orientation and incidence checks remain necessary.
[B-rep morph fitting](brep-morphing.md) now uses these assembly checks: accurate
individual surface fits alone do not establish shared-boundary consistency.

The initial boundary-validation checkpoint passed 1,310 Rust tests, 19 Python
tests, and strict Clippy. All 117 native oracle fixtures (1,193 operations)
executed; 369 recorded
curve comparisons retain their documented tolerances. Fresh Rhino 8.32 checks
pass for six trimmed mass-property cases at absolute `1e-8`, relative `1e-10`
(largest difference `1.22e-9`), and two non-affine trimmed-surface splits at
absolute `1e-9`, relative `1e-10` (largest difference `4.67e-10`). These compare
valid geometry and command results, not equivalence to Rhino's validity predicate.
