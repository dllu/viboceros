# Surface closest points

[Numerical robustness](numerical-robustness.md) · [EvaluateUVPt](commands/evaluate-uv-point.md)

`NurbsSurface::closest_parameters` is shared by surface analysis and geometry
operations. Its bounded search samples the UV domain, refines up to sixteen
promising starts, and independently checks the four natural boundary curves.
The implementation is isolated in `nurbs_surface/closest_point.rs`.

## Parameter-scale independence

The tangent solve normalizes its two Jacobian columns before constructing an
orthogonal frame with compensated cross products. Parameter speeds are not model
feature sizes: increasing a UV domain makes its derivative smaller without
changing the geometry. Model tolerance therefore controls the tangential
model-space residual, not whether a nonzero derivative is accepted.

A regression demonstrated the old failure on a `4 × 2` plane: stretching U from
`[0,1]` to `[0,10¹²]` changed the projection of `(1.37,0.63,3)` to approximately
`(1.37931,0.62069,0)`, an error of `0.013` model units. Independent U/V scaling now
preserves the expected projection. Step conversion divides by a scaled derivative
norm without first forming a potentially overflowing norm.

If all refinements fail, or a derivative/step becomes non-finite, non-unit domains
are retried on a unit-domain copy. Returned parameters are mapped back and checked
on the original surface. Ordinary successful queries do not copy the control net.
Native tests include `[0,10⁻³⁰⁸]`, `[0,10³⁰⁸]`, and `[-MAX,MAX]` parameter domains.

## Rounded distance ties

For a query far from a surface, candidate distances can round to the same binary64
value even when their tangential errors differ substantially. A plane regression
at Z=`10¹⁵` previously retained a sampled point instead of the projected point.

`Point3::compare_distances` orders squared distances exactly for the stored finite
coordinates. It expands `|a-t|² - |b-t|²` into twelve products, cancels the common
`t²` terms algebraically, and accumulates positive and negative terms separately
with the shared fixed-limb binary accumulator. Comparing the integer accumulators
needs neither a square root nor a rounded result. Subnormal differences and
overflowing distances remain distinguishable. The surface solver uses this for
final candidate selection and line-search ties. This is an exact predicate on
evaluated points, not an exact solution of the rational optimization problem.

## Evidence and limits

The [fixture](../tools/rhino_oracle/fixtures/surface-closest-point.json) and
[Rhino 8.32 reference](surface-closest-point-rhino-reference.json) cover seventeen
queries on planar, skew planar, and rational quarter-cylinder surfaces, with unit,
wide, and strongly anisotropic domains. Cases include interior projections,
natural-edge minima, and corners. Saved-record replay compares model points,
distances, and normalized parameters to `1e-8`; native parameter allowances scale
with domain width. Extreme-domain and distant-query regressions are independently
analytic tests, not additional Rhino measurements.

```sh
cargo test -p viboceros-geometry closest_point_
cargo test -p viboceros-geometry exact_distance_order
cargo test -p viboceros-oracle closest_surface_fixture
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface-closest-point.json --absolute-epsilon 1e-8 --relative-epsilon 1e-8 --timeout 240
```

The probe times only batches of closest-parameter queries; construction and result
formatting are outside the timer. Rhino timings still include the Python/API
bridge, so the ratios are harness measurements rather than native kernel speedups.
The short three-iteration run does not establish performance parity: several
native batches, particularly the cylinder searches, are slower than the licensed
Rhino harness. Reducing the many-seed search cost remains follow-up work.

The multi-start search is not a certified global minimum solver for arbitrary
multi-modal rational surfaces. Coarse seed ranking uses rounded distances, singular
regions can exhaust refinement, and seed distances still need to be representable.
These changes address demonstrated numerical failures, not all closest-point or
trim-aware picking limitations.
