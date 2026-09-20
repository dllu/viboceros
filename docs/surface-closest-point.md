# Surface closest points

[Numerical robustness](numerical-robustness.md) · [EvaluateUVPt](commands/evaluate-uv-point.md)

`NurbsSurface::closest_parameters` is shared by surface analysis and geometry
operations. Exactly affine bilinear patches use direct constrained projection.
Other surfaces use a bounded search that samples the UV domain, refines up to
sixteen promising starts, and independently checks the four natural boundary
curves. The implementation is isolated in `nurbs_surface/closest_point.rs`.

## Affine bilinear fast path

The direct path accepts only degree-one surfaces with a `2 × 2` control net,
identical nonzero weights, and exactly parallelogram-shaped controls. The diagonal
identity is checked with `FiniteSum`, not a model tolerance or equality of rounded
sums: even a tiny warp or a diagonal mismatch hidden by floating-point addition
must fall back to the general search. Common negative weights are supported.

A normalized tangent frame gives the unconstrained plane projection. If it lies
inside the patch, it is considered alongside the closest point on each of the four
finite edges. Independently clamping the plane's U and V coordinates is **not**
correct for skew patches; each edge needs its own one-dimensional minimization.
There are at most five surface evaluations, instead of a sampled multi-start
search. Candidates are mapped to the original parameter domains, evaluated on
the original NURBS, and compared using the exact distance predicate below.

Nonuniform weights, refined control nets, non-affine patches, singular tangent
frames, and numerical failures retain the general path. This is an exact
representation check followed by a floating-point solve, not an exact arithmetic
closest-point construction. Tests include 918 independent convex optimality
checks, rotated and translated frames, skew edge minima, reversed axes, common
signed/extreme weights, distant queries, extreme domains, and nearly parallel
axes. Source geometry remains unchanged.

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

## Curvature-aware refinement

General surfaces retain the same UV seeds, sixteen refinement starts, and four
independent boundary searches. The refinement now first tries the full Hessian of
half the squared distance, including mixed partials. In independently
speed-normalized parameter coordinates its entries are
`Hᵢⱼ = unitᵢ·unitⱼ − residual·Sᵢⱼ/(speedᵢ speedⱼ)`. Nalgebra's Cholesky solve
accepts only a positive-definite free-variable Hessian. A boundary coordinate is
held fixed when the gradient points out of the domain; the other coordinate is
still minimized independently.

Indefinite or singular Hessians, non-normal metric products, and unrepresentable
curvature steps retain tangent-plane QR. Failed second-derivative evaluation also
retains usable first derivatives. Both directions use clamping and backtracking;
an unevaluable trial point is rejected without discarding the whole start. No
surface is recognized as, or substituted by, an analytic cylinder or paraboloid.

### Stationarity at the evaluation-roundoff limit

An exact comparison of *evaluated* points cannot remove error in those positions.
Near a smooth minimum, the true distance decrease is quadratic in parameter error,
while point-evaluation error can change the distance to first order. A convex
paraboloid regression exposed roughly `1e-7` model-point error despite decreasing
the distance between stored binary64 points.

After candidate selection, at most eight local Newton corrections improve the
projected gradient (including boundary KKT signs). They must remain within
`sqrt(64 ε) × scale` of the selected point and within `64 ε × scale` of its
distance, where scale includes control coordinates, the selected point, and query
distance. These are floating-point safeguards, not certified rational-evaluation
error bounds. Ordinary search and candidate comparison remain monotone; this
last correction may increase the rounded distance within that allowance. If
second derivatives overflow, one unit-domain copy can recover the correction,
then parameters are mapped back and evaluated on the original surface.

Tests cover the full mixed Hessian under independent parameter rescaling, skew
metrics, active constraints, rejected indefinite/singular Hessians, a two-minimum
saddle, and a local-polish guard. Seventy-two convex-paraboloid queries have
independently prescribed global minima, including edges and corners, and retain
the `1e-8` model-point bound. Another regression has finite first derivatives but
overflowing second derivatives on `[0,10⁻¹⁷⁰] × [0,10¹⁶⁰]` domains.

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

The [curvature fixture](../tools/rhino_oracle/fixtures/surface-closest-curvature.json)
and [fresh Rhino reference](surface-closest-curvature-rhino-reference.json) add
twenty-one queries on convex paraboloids and a warped bilinear saddle. The saved
record selects their first benchmark round without reserializing floating-point
values. Native paraboloid results are within `1.7e-11` of the independent analytic
minima; one Rhino upper-edge result is `8.71e-8` away. This new replay therefore
uses `1e-7`, while the original seventeen-query replay and native analytic tests
keep their stricter `1e-8` bounds. Saddle model points agree within `1.3e-12`.

```sh
cargo test -p viboceros-geometry closest_point_
cargo test -p viboceros-geometry affine_closest
cargo test -p viboceros-geometry closest_point::refine
cargo test -p viboceros-geometry exact_distance_order
cargo test -p viboceros-oracle closest_surface_fixture
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface-closest-point.json --absolute-epsilon 1e-8 --relative-epsilon 1e-8 --timeout 240
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface-closest-curvature.json --absolute-epsilon 1e-7 --relative-epsilon 1e-7 --timeout 240
```

## Performance measurement

### Affine path measurement

The [measurement record](surface-closest-point-performance.json) uses the same six
fixture operations, each repeated for three rounds of 200 iterations on aarch64
with release builds. Native baseline `1979507` and the optimized executable ran
sequentially after builds and tests finished. Times below are the median across
rounds, divided by queries per batch; all figures are microseconds per query.

| Fixture case | Native before | Native after | Rhino harness |
| --- | ---: | ---: | ---: |
| Plane, unit domains | 143.903 | 0.921 | 8.252 |
| Plane, wide U | 144.056 | 0.927 | 8.774 |
| Plane, anisotropic domains | 292.687 | 0.930 | 8.214 |
| Skew plane, anisotropic domains | 152.062 | 0.965 | 10.204 |
| Quarter cylinder, unit domains | 408.832 | 400.230 | 13.377 |
| Quarter cylinder, anisotropic domains | 475.432 | 484.134 | 12.906 |

The measured affine cases improve by 155–315× against the native baseline; the
general cylinder path is essentially unchanged. All eighteen operations match the
fresh Rhino 8.32 capture at absolute/relative epsilon `1e-8`. Maximum model-point
distance is `8.087e-9`, and maximum normalized-parameter difference is `2.453e-9`,
both on cylinder cases. Raw parameter differences must be interpreted relative to
the UV domain; a large native parameter error can correspond to a tiny geometric
error on a trillion-unit domain.

The probe times only batches of closest-parameter queries; construction and result
formatting are outside the timer. Rhino timings still include the Python/API
bridge and host overhead, so these are harness measurements rather than native
Rhino kernel speedups. The record includes raw round times to expose variability.
At this stage the cylinder search remained substantially slower than the Rhino
harness. The subsequent general refinement measurement follows.

### Curvature-aware refinement measurement

The [new measurement record](surface-closest-curvature-performance.json) combines
both fixtures, again using three rounds of 200 iterations. Baseline `d4ff70a`, the
new release executable, and the Rhino harness ran sequentially after all tests and
builds finished. Median microseconds per query:

| Fixture case | Native before | Native after | Rhino harness | Native speedup |
| --- | ---: | ---: | ---: | ---: |
| Quarter cylinder, unit domains | 402.048 | 237.009 | 19.875 | 1.70× |
| Quarter cylinder, anisotropic domains | 494.821 | 250.724 | 17.090 | 1.97× |
| Paraboloid, unit domains | 502.474 | 441.081 | 57.606 | 1.14× |
| Paraboloid, anisotropic domains | 484.992 | 439.653 | 61.561 | 1.10× |
| Warped bilinear saddle | 249.814 | 189.547 | 19.811 | 1.32× |

The record also retains all four unchanged affine-path controls: measured times
were 2–4% higher, at `0.94–0.99 µs` per query. Cylinder model points now agree with
Rhino within `1.3e-15`. Paraboloid error against the analytic minimum decreased
from `2.8e-9`/`1.4e-8` (unit/anisotropic) to below `1.7e-11` for both domains.
All fresh comparisons pass their separately documented fixture epsilons. This
improves the general solver but does not establish performance parity: curved
cases remain slower than the Rhino harness, and the dense seed search remains.

The multi-start search is not a certified global minimum solver for arbitrary
multi-modal rational surfaces. Coarse seed ranking uses rounded distances, singular
regions can exhaust refinement, and seed distances still need to be representable.
These changes address demonstrated numerical failures, not all closest-point or
trim-aware picking limitations.
