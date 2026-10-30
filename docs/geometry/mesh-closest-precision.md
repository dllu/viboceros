# Mesh closest-point precision

[Cap comparisons](cap-mesh-native.md) · [Geometry guides](README.md)

Nearest-point queries on long thin triangles previously subtracted almost equal
dot products to classify triangle regions. A million-unit edge could move an
interior query almost half a unit. All intermediate numbers were finite.

Triangle queries now use cross products for region determinants and compute
short-edge projections directly. Existing outward-rounded intervals certify
each region decision. An interpolation is accepted only when its bounds put each
true foot coordinate within 32 adjacent spacings of the returned coordinate.
Unknown decisions,
loose bounds, overflow and underflow use the existing exact rational query.
The document's modeling tolerance does not alter this arithmetic policy.

TwoSum and bounded FMA residuals preserve exact scalar operations and cancellation
between products. Residual checks below their representable range cannot certify
an operation as exact. The nearest-point implementation now lives together in
the `mesh/closest` modules.

The index rejects leaf triangles whose boxes cannot beat its best candidate.
At zero distance, it skips candidates that cannot win the lowest-index tie;
node metadata preserves earlier candidates across the hierarchy.

Analytic tests cover rotated orthogonal frames, winding, every vertex order,
logical-face queries and indexed queries. Exact-rational holdouts cover all
closed Voronoi regions, seven scale exponents and four aspect ratios; scalar
enclosures are checked independently across the binary64 range.

All 22 archived Cap sources must now satisfy the unchanged `1e-6` sampling bound
before the command and after Undo. Source arrays still round-trip exactly.
Native Cap's separate region, count, winding and normal differences remain
explicit in the replay gates.

Manual timings measure unoptimized direct triangle calls, including accuracy;
they are not a comparison with Rhino or a release-performance qualification.
The [validation record](mesh-closest-precision-validation.json) retains baseline
errors and timing costs alongside the current source and replay measurements.
The earlier [Cap record](cap-mesh-native-validation.json) keeps its measured
source hashes and failures at revision `729d86cd9`.
