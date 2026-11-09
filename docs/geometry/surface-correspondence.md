# Complete surface correspondence bounds

`NurbsSurface::parameter_surface_deviation_bound(other, limit)` bounds the
distance between two complete untrimmed NURBS surfaces under independently
normalized U/V domains. A returned bound covers the entire surface interior
and all one-sided knot limits. `None` means outside the requested bound or
unproved within the supported limits. Reverse or swap a source explicitly to
change the parameter correspondence.

[Planar image bounds](surface-planar-images.md) provide a separate geometric
certificate through exact auxiliary quadrilaterals when UV speeds differ.
[Supplied U/V mappings](surface-mapped-images.md) certify curved images with
independent piecewise affine parameter speeds.

The implementation partitions both stored knot grids exactly, restricts their
homogeneous tensor patches with rational arithmetic, and forms the Bernstein
numerator and positive denominator of their difference. Convex hulls bound
Euclidean distance; exact half subdivision tightens loose hulls. No sampled or
rounded geometry replaces the original controls, knots or weights.

Identical stored tensors with nonzero common-sign weights return an exact zero
bound directly. This avoids unnecessary patch extraction on large unchanged
surfaces and archive round-trips. Mixed-sign tensors retain the regular checks.

Supports degrees through 16 and at most 4096 controls per source, independent
common-sign weight gauges, unclamped domains and discontinuous knots. The
shared surface-curve certificate limits bound work, rational bit size and
subdivision depth. Exhaustion returns an inconclusive result. The bound also
bounds symmetric Hausdorff distance, but does not certify trimming, topology,
normals or injectivity.

Regression cases include an interior bump hidden between every station of a
17×17 sample grid and four identical boundary curves, rational weights,
degree changes, different knot grids, discontinuities and subdivision. All
119 existing unrefined averaging cases additionally compare both complete
Native surfaces and their 3DM round-trips at `1e-9`, on portable and SMLib builds.

The [validation record](../surface-deviation-validation.json) retains the
complete replay scope, frozen sources and check logs. Reproduce it with
`python3 -m tools.rhino_oracle.report_surface_deviation`.
