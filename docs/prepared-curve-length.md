# Prepared curve length queries

`viboceros_geometry::ArcLengthSampler` prepares integration, inversion and
optional lookup brackets for repeated queries on a native curve. It supports
lines, conics, polylines, NURBS and polycurves through the existing curve kernel.

Use `try_new(CurveRef, tolerance)` to borrow a curve or
`try_new_owned(Arc<Curve3>, tolerance)` to retain shared immutable geometry.
Owned samplers can move independently of the caller without a self-reference;
cloning the caller's Arc does not duplicate the curve. Prepare lookup brackets
before sharing a sampler through an Arc.

`distance_at_parameter` maps a source parameter to distance from the active
start. `parameter_at_distance`, `point_at_distance` and `sample_at_distance`
perform the reverse query. Samples include the native parameter, point and unit
tangent. Parameters keep the original domain even when integration uses a private
normalized representation. The original source is available through `source_curve`.

`prepare_budgeted_repeated_sampling(preferred_subdivisions)` accelerates repeated
queries with integration prefix brackets. It reduces the requested density to fit
the existing aggregate limit of 1,048,576 parameter/length pairs, or retains
uncached integration when even the smallest table cannot fit. Brackets still
refine their queries using the existing integration and inversion tolerances.
A zero requested density is invalid. Failed preparation retains the prior tables.

Distances must be finite and lie in `[0, total_length()]`; parameters must be
finite and lie in the active source domain. Total length must be positive and
finite. Exact source endpoints and the kernel's stable linear-span sampling are
retained. Corners use the existing right-sided tangent policy, with the terminal
sample following the last segment. This API does not wrap closed curves or
extend open ones beyond their domains.

The owned and borrowed paths share one implementation. Regression tests compare
cached non-unit rational NURBS queries and verify that the owned sampler retains
and releases its source. The existing length tests also exercise extreme
parameter ranges, tiny terminal spans, rational-line speeds, composites and
forward/inverse correspondence.

This shared ownership supports the ongoing [Flow implementation](flow-development.md).
